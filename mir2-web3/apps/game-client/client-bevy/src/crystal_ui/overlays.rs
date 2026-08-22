//! Operable native player windows: bag, equipment, inspect, death, menu, chat, mail, bigmap, shop, storage.

use std::collections::VecDeque;

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, Interaction, JustifyContent, Node,
    PositionType, UiRect, Val,
};

use crate::game_shop::{
    game_shop_page_count, game_shop_page_entries, game_shop_page_for_index, GameShopModel,
    GameShopPaymentType, GameShopRequest, GAME_SHOP_QUANTITY_MAX, GAME_SHOP_QUANTITY_MIN,
};
use crate::inventory::{InventoryModel, ItemModel};
use crate::mail::{
    mail_claim_enabled, mail_delete_enabled, MailModel, MailOperationKind, MAX_MAIL_ATTACHMENTS,
};
use crate::map::MapModel;
use crate::native_shell::{
    NativeShellModel, NativeShellScreen, NativeUiIntent, NativeUiIntentQueue,
};
use crate::pending_operations::{
    AuthoritativeModelRevisions, InventoryOperationFeedback, NativeSessionBoundaryTracker,
    OverlayResetTracker, PendingLifecycleSet, PendingOperationKey, PendingOperations,
    SessionResetGameShopPreservation, SessionResetRevision,
};
use crate::quest_model::NpcDialogModel;
use crate::read_model::{UiReadModel, UiSurfaceSignals};
use crate::shop::{
    shop_buy_enabled, shop_quantity_clamped, shop_quantity_dec, shop_quantity_inc,
    shop_sell_enabled, ShopModel,
};
use crate::skill_model::SkillModel;
use crate::storage::{
    storage_deposit_enabled, storage_expand_enabled, storage_password_display,
    storage_remove_password_enabled, storage_set_password_enabled, storage_unlock_enabled,
    storage_withdraw_enabled, StorageModel,
};

use super::hud::CrystalHudAction;

/// Overlay mutation must run before any `Res<NativePlayerUiState>` readers in
/// the same Update. Unordered Res + ResMut on this resource panics Bevy B0001.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativePlayerUiSet {
    Mutate,
    Read,
}

const PANEL_BG: Color = Color::srgba(0.07, 0.05, 0.03, 0.94);
const TEXT: Color = Color::srgb(0.95, 0.92, 0.82);
const GOLD: Color = Color::srgb(0.94, 0.78, 0.28);
const BUTTON_BG: Color = Color::srgba(0.28, 0.18, 0.08, 0.95);
const BUTTON_DISABLED: Color = Color::srgba(0.30, 0.24, 0.16, 0.45);
const MAX_QUEUED: usize = 24;
const BAG_SLOTS: u32 = 46;

pub const BIGMAP_ZOOM_MIN: f32 = 0.5;
pub const BIGMAP_ZOOM_MAX: f32 = 3.0;
pub const BIGMAP_ZOOM_STEP: f32 = 0.25;
pub const BIGMAP_WIDTH: f32 = 568.0;
pub const BIGMAP_HEIGHT: f32 = 380.0;

// Re-export shop/storage constants for external consumers that import via overlays.
pub use crate::shop::{SHOP_QUANTITY_MAX, SHOP_QUANTITY_MIN, SHOP_QUANTITY_STEP};
pub use crate::storage::{STORAGE_BASE_SIZE, STORAGE_EXPANDED_SIZE, STORAGE_EXPAND_COST};

// ---------------------------------------------------------------------------
// Item inspect
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInspect {
    pub container: u8,
    pub slot: u32,
    pub key: String,
    pub name: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryOperationDraft {
    Move { source_slot: u32, unique_id: u64 },
    Merge { source_slot: u32, unique_id: u64 },
}

pub fn bigmap_zoom_clamped(zoom: f32) -> f32 {
    zoom.clamp(BIGMAP_ZOOM_MIN, BIGMAP_ZOOM_MAX)
}

pub fn bigmap_zoom_in(zoom: f32) -> f32 {
    bigmap_zoom_clamped(zoom + BIGMAP_ZOOM_STEP)
}

pub fn bigmap_zoom_out(zoom: f32) -> f32 {
    bigmap_zoom_clamped(zoom - BIGMAP_ZOOM_STEP)
}

pub fn bigmap_asset_path(map_name: Option<&str>) -> Option<String> {
    let map_name = map_name?.trim();
    if map_name.eq_ignore_ascii_case("BichonProvince")
        || map_name.eq_ignore_ascii_case("Bichon Province")
    {
        return Some("original-ui/MMap/101.png".to_owned());
    }
    None
}

pub fn bigmap_player_position(map: &MapModel, zoom: f32) -> (f32, f32) {
    // For overlay panel we place player dot relative to BigMap viewport center.
    // The viewport is BIGMAP_WIDTH x BIGMAP_HEIGHT centered at (0,0) in panel space.
    // Player logical position is scaled by zoom.
    let zoom = bigmap_zoom_clamped(zoom);
    // Normalized position within viewport: center is (BIGMAP_WIDTH/2, BIGMAP_HEIGHT/2)
    // We add small offset based on center_x/y to prove correctness.
    let x = BIGMAP_WIDTH * 0.5
        + (map.center_x as f32 * zoom * 0.5).clamp(-BIGMAP_WIDTH * 0.4, BIGMAP_WIDTH * 0.4);
    let y = BIGMAP_HEIGHT * 0.5
        + (map.center_y as f32 * zoom * 0.5).clamp(-BIGMAP_HEIGHT * 0.4, BIGMAP_HEIGHT * 0.4);
    (x, y)
}

#[derive(Debug, Clone, Resource, PartialEq)]
pub struct NativePlayerUiState {
    /// The single authoritative core UI state used by the native adapter.
    /// `quest_ui` reads and writes this same field; it does not own another
    /// `UiState` resource.
    pub core: mir2_ui_core::state::UiState,
    pub bigmap_zoom: f32,
    pub chat_draft: String,
    pub inspect: Option<ItemInspect>,
    pub shop_quantity: u16,
    pub shop_repair_mode: bool,
    /// Repair selection is a `(container, slot)` pair. The emitted command
    /// still carries the authoritative unique id, so bag slot 3 cannot be
    /// mistaken for equipment slot 3.
    pub shop_repair_container: u8,
    pub shop_repair_slot: Option<u32>,
    /// Bounded cash-shop page; keeps the native panel usable while every
    /// server catalog row remains reachable.
    pub game_shop_page: usize,
    pub split_count: u16,
    pub inventory_operation: Option<InventoryOperationDraft>,
    pub selected_group_member: Option<u8>,
    pub selected_guild_member: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MailComposeFocus {
    #[default]
    Recipient,
    Message,
    Gold,
}

#[derive(Debug, Clone, Default, Resource, PartialEq, Eq)]
pub struct MailComposeUi {
    pub focus: MailComposeFocus,
    pub last_notice: Option<String>,
}

impl Eq for NativePlayerUiState {}

impl Default for NativePlayerUiState {
    fn default() -> Self {
        Self {
            core: mir2_ui_core::state::UiState {
                screen: mir2_ui_core::state::UiScreen::InGame,
                panel: mir2_ui_core::state::UiPanel::None,
                minimap_visible: true,
                chat_focused: false,
                ..Default::default()
            },
            bigmap_zoom: 1.0,
            chat_draft: String::new(),
            inspect: None,
            shop_quantity: 1,
            shop_repair_mode: false,
            shop_repair_container: 0,
            shop_repair_slot: None,
            game_shop_page: 0,
            split_count: 1,
            inventory_operation: None,
            selected_group_member: None,
            selected_guild_member: None,
        }
    }
}

impl NativePlayerUiState {
    fn apply(&mut self, action: mir2_ui_core::action::UiAction) {
        let is_panel = matches!(
            action,
            mir2_ui_core::action::UiAction::OpenInventory
                | mir2_ui_core::action::UiAction::OpenCharacter
                | mir2_ui_core::action::UiAction::OpenSkill
                | mir2_ui_core::action::UiAction::OpenQuestLog
                | mir2_ui_core::action::UiAction::OpenOptions
                | mir2_ui_core::action::UiAction::OpenMenu
                | mir2_ui_core::action::UiAction::OpenGameShop
                | mir2_ui_core::action::UiAction::OpenNpcShop
                | mir2_ui_core::action::UiAction::OpenMail
                | mir2_ui_core::action::UiAction::OpenBigMap
                | mir2_ui_core::action::UiAction::OpenStorage
                | mir2_ui_core::action::UiAction::OpenGroup
                | mir2_ui_core::action::UiAction::OpenGuild
                | mir2_ui_core::action::UiAction::OpenTrade
                | mir2_ui_core::action::UiAction::ClosePanel
                | mir2_ui_core::action::UiAction::CloseAllPanels
                | mir2_ui_core::action::UiAction::ToggleMinimap
                | mir2_ui_core::action::UiAction::OpenChatSettings
        );
        if is_panel && self.core.screen != mir2_ui_core::state::UiScreen::InGame {
            self.core.screen = mir2_ui_core::state::UiScreen::InGame;
        }
        self.core = mir2_ui_core::reducer::reduce(&self.core, action).state;
    }

    pub fn inventory_open(&self) -> bool {
        self.core.inventory_open()
    }
    pub fn equipment_open(&self) -> bool {
        self.core.equipment_open()
    }
    pub fn menu_open(&self) -> bool {
        self.core.menu_open()
    }
    pub fn skill_open(&self) -> bool {
        self.core.skill_open()
    }
    pub fn quest_open(&self) -> bool {
        self.core.quest_open()
    }
    pub fn options_open(&self) -> bool {
        self.core.options_open()
    }
    pub fn mail_open(&self) -> bool {
        self.core.mail_open()
    }
    pub fn bigmap_open(&self) -> bool {
        self.core.bigmap_open()
    }
    pub fn shop_open(&self) -> bool {
        self.core.shop_open()
    }
    pub fn npc_shop_open(&self) -> bool {
        self.core.npc_shop_open()
    }
    pub fn storage_open(&self) -> bool {
        self.core.storage_open()
    }
    pub fn group_open(&self) -> bool {
        self.core.is_group_open()
    }
    pub fn guild_open(&self) -> bool {
        self.core.is_guild_open()
    }
    pub fn trade_open(&self) -> bool {
        self.core.is_trade_open()
    }
    pub fn minimap_visible(&self) -> bool {
        self.core.minimap_visible()
    }
    pub fn chat_focused(&self) -> bool {
        self.core.chat_focused()
    }

    pub fn toggle_inventory(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenInventory);
    }
    pub fn toggle_equipment(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenCharacter);
    }
    pub fn toggle_menu(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenMenu);
    }
    pub fn toggle_skill(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenSkill);
    }
    pub fn toggle_quest(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenQuestLog);
    }
    pub fn toggle_options(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenOptions);
    }
    pub fn toggle_mail(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenMail);
    }
    pub fn toggle_bigmap(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenBigMap);
    }
    pub fn toggle_shop(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenGameShop);
    }
    pub fn toggle_npc_shop(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenNpcShop);
    }
    pub fn toggle_storage(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenStorage);
    }
    pub fn toggle_group(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenGroup);
    }
    pub fn toggle_guild(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenGuild);
    }
    pub fn toggle_trade(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenTrade);
    }
    pub fn toggle_minimap(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::ToggleMinimap);
    }
    pub fn set_chat_focused(&mut self, focused: bool) {
        self.apply(if focused {
            mir2_ui_core::action::UiAction::FocusChat
        } else {
            mir2_ui_core::action::UiAction::BlurChat
        });
    }
    pub fn blocks_gameplay_keys(&self) -> bool {
        self.core.blocks_gameplay_keys()
    }
    pub fn blocks_world_click(&self) -> bool {
        self.core.blocks_world_click() || self.inspect.is_some()
    }
    pub fn blocks_world_action(&self, dialog_open: bool, dead: bool) -> bool {
        self.blocks_world_click() || dialog_open || dead
    }
    pub fn captures_pointer(
        &self,
        dragging_window: bool,
        dragging_scrollbar: bool,
        button_pressed: bool,
    ) -> bool {
        dragging_window || dragging_scrollbar || button_pressed || self.blocks_world_click()
    }
    pub fn trimmed_chat_to_send(&self) -> Option<String> {
        let value = self.chat_draft.trim();
        (!value.is_empty()).then(|| value.to_owned())
    }
    pub fn is_chat_focused(&self) -> bool {
        self.core.chat_focused()
    }
    pub fn close_windows(&mut self) {
        self.core.panel = mir2_ui_core::state::UiPanel::None;
        self.core.options_draft = None;
        self.core.chat_settings_draft = None;
        self.inspect = None;
        self.inventory_operation = None;
        self.shop_repair_container = 0;
        self.shop_repair_slot = None;
        self.game_shop_page = 0;
        self.split_count = 1;
        self.selected_group_member = None;
        self.selected_guild_member = None;
    }
    pub fn reset_session(&mut self) {
        let options = self.core.options.clone();
        let chat_settings = self.core.chat_settings;
        let game_shop_had_pending = self.core.game_shop_pending.is_some();
        *self = Self::default();
        // Options are application-scoped rather than character/session-scoped.
        // Keep the loaded/applied values when the shell leaves InGame so a
        // logout/re-login does not silently revert the user's local settings.
        self.core.options = options;
        self.core.chat_settings = chat_settings;
        self.core.game_shop_unknown = game_shop_had_pending;
    }
    pub fn reset_session_preserving_exact_game_shop_receipt(
        &mut self,
        receipt: &crate::game_shop::GameShopReceipt,
    ) {
        let options = self.core.options.clone();
        let chat_settings = self.core.chat_settings;
        *self = Self::default();
        self.core.options = options;
        self.core.chat_settings = chat_settings;
        let _ = self.core.preserve_exact_game_shop_receipt_boundary(receipt);
    }
    pub fn zoom_in(&mut self) {
        self.bigmap_zoom = bigmap_zoom_in(self.bigmap_zoom);
    }
    pub fn zoom_out(&mut self) {
        self.bigmap_zoom = bigmap_zoom_out(self.bigmap_zoom);
    }
    pub fn shop_quantity_inc(&mut self) {
        self.shop_quantity = shop_quantity_inc(self.shop_quantity);
    }
    pub fn shop_quantity_dec(&mut self) {
        self.shop_quantity = shop_quantity_dec(self.shop_quantity);
    }
}

#[derive(Debug, Default, Resource)]
pub struct UiEffectQueue {
    effects: VecDeque<mir2_ui_core::effect::UiEffect>,
}

impl UiEffectQueue {
    pub fn push(&mut self, effect: mir2_ui_core::effect::UiEffect) {
        if self.effects.len() >= MAX_QUEUED {
            self.effects.pop_front();
        }
        self.effects.push_back(effect);
    }

    pub fn drain(&mut self) -> Vec<mir2_ui_core::effect::UiEffect> {
        self.effects.drain(..).collect()
    }

    pub fn drain_mail_commands(&mut self) -> Vec<mir2_ui_core::effect::GatewayCommand> {
        let mut commands = Vec::new();
        let mut retained = VecDeque::with_capacity(self.effects.len());
        while let Some(effect) = self.effects.pop_front() {
            match effect {
                mir2_ui_core::effect::UiEffect::GatewayCommand(
                    command @ mir2_ui_core::effect::GatewayCommand::SendMail { .. },
                ) => commands.push(command),
                other => retained.push_back(other),
            }
        }
        self.effects = retained;
        commands
    }

    pub(crate) fn drain_options_bounded(
        &mut self,
        max: usize,
    ) -> Vec<mir2_ui_core::effect::UiEffect> {
        self.drain_matching_bounded(max, |effect| {
            matches!(
                effect,
                mir2_ui_core::effect::UiEffect::ApplyAudioSettings { .. }
                    | mir2_ui_core::effect::UiEffect::ApplyWindowMode { .. }
                    | mir2_ui_core::effect::UiEffect::PersistOptions { .. }
            )
        })
    }

    pub(crate) fn drain_chat_settings_bounded(
        &mut self,
        max: usize,
    ) -> Vec<mir2_ui_core::effect::UiEffect> {
        self.drain_matching_bounded(max, |effect| {
            matches!(
                effect,
                mir2_ui_core::effect::UiEffect::ApplyChatSettings { .. }
                    | mir2_ui_core::effect::UiEffect::PersistChatSettings { .. }
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.effects.len()
    }

    fn drain_matching_bounded<F>(
        &mut self,
        max: usize,
        mut matches: F,
    ) -> Vec<mir2_ui_core::effect::UiEffect>
    where
        F: FnMut(&mir2_ui_core::effect::UiEffect) -> bool,
    {
        if max == 0 || self.effects.is_empty() {
            return Vec::new();
        }

        let mut drained = Vec::with_capacity(max.min(self.effects.len()));
        let mut retained = VecDeque::with_capacity(self.effects.len());
        while let Some(effect) = self.effects.pop_front() {
            if drained.len() < max && matches(&effect) {
                drained.push(effect);
            } else {
                retained.push_back(effect);
            }
        }
        self.effects = retained;
        drained
    }
}

/// Apply a shared action and retain every typed adapter effect for the host.
pub fn dispatch_ui_action(
    core: &mut mir2_ui_core::state::UiState,
    effects: &mut UiEffectQueue,
    action: mir2_ui_core::action::UiAction,
) -> mir2_ui_core::reducer::Transition {
    let is_panel = matches!(
        &action,
        mir2_ui_core::action::UiAction::OpenInventory
            | mir2_ui_core::action::UiAction::OpenCharacter
            | mir2_ui_core::action::UiAction::OpenSkill
            | mir2_ui_core::action::UiAction::OpenQuestLog
            | mir2_ui_core::action::UiAction::OpenOptions
            | mir2_ui_core::action::UiAction::OpenMenu
            | mir2_ui_core::action::UiAction::OpenGameShop
            | mir2_ui_core::action::UiAction::OpenNpcShop
            | mir2_ui_core::action::UiAction::OpenMail
            | mir2_ui_core::action::UiAction::OpenBigMap
            | mir2_ui_core::action::UiAction::OpenStorage
            | mir2_ui_core::action::UiAction::ClosePanel
            | mir2_ui_core::action::UiAction::CloseAllPanels
            | mir2_ui_core::action::UiAction::ToggleMinimap
            | mir2_ui_core::action::UiAction::OpenChatSettings
    );
    if is_panel && core.screen != mir2_ui_core::state::UiScreen::InGame {
        core.screen = mir2_ui_core::state::UiScreen::InGame;
    }
    let transition = mir2_ui_core::reducer::reduce(core, action);
    *core = transition.state.clone();
    for effect in transition.effects.iter().cloned() {
        effects.push(effect);
    }
    transition
}

// ---------------------------------------------------------------------------
// Z-order and modal priority (Goal 4.8)
// ---------------------------------------------------------------------------

pub const OVERLAY_HUD_Z: i32 = 950;
pub const OVERLAY_MINIMAP_Z: i32 = 905;
pub const OVERLAY_QUEST_Z: i32 = 900;
pub const OVERLAY_CHAT_Z: i32 = 975;
pub const OVERLAY_NPC_DIALOG_Z: i32 = 980;
pub const OVERLAY_DEATH_Z: i32 = 985;
pub const OVERLAY_MENU_Z: i32 = 990;
pub const OVERLAY_SHELL_Z: i32 = 1000;

/// Verify HUD < Chat < NPC < Death < Menu < Shell ordering.
pub fn is_overlay_z_order_correct() -> bool {
    // Note: quest/minimap are below HUD; main ordering under test is HUD/chat/dialog/death/menu/shell.
    OVERLAY_HUD_Z < OVERLAY_CHAT_Z
        && OVERLAY_CHAT_Z < OVERLAY_NPC_DIALOG_Z
        && OVERLAY_NPC_DIALOG_Z < OVERLAY_DEATH_Z
        && OVERLAY_DEATH_Z < OVERLAY_MENU_Z
        && OVERLAY_MENU_Z < OVERLAY_SHELL_Z
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OverlayModalPriority {
    Hud = 0,
    NpcDialog = 1,
    Chat = 2,
    Death = 3,
    SystemMenu = 4,
}

pub fn modal_priority_for_state(
    state: &NativePlayerUiState,
    dialog_open: bool,
    dead: bool,
) -> Option<OverlayModalPriority> {
    if state.menu_open() {
        return Some(OverlayModalPriority::SystemMenu);
    }
    if dead {
        return Some(OverlayModalPriority::Death);
    }
    if state.chat_focused() {
        return Some(OverlayModalPriority::Chat);
    }
    if dialog_open || state.quest_open() {
        return Some(OverlayModalPriority::NpcDialog);
    }
    if state.inventory_open()
        || state.equipment_open()
        || state.skill_open()
        || state.mail_open()
        || state.bigmap_open()
        || state.shop_open()
        || state.npc_shop_open()
        || state.storage_open()
        || state.group_open()
        || state.guild_open()
        || state.trade_open()
        || state.options_open()
    {
        // Generic window below NPC dialog but above HUD.
        return Some(OverlayModalPriority::NpcDialog);
    }
    None
}

/// Effective Z for a modal priority (used in rendering).
pub fn z_for_modal(priority: OverlayModalPriority) -> i32 {
    match priority {
        OverlayModalPriority::Hud => OVERLAY_HUD_Z,
        OverlayModalPriority::NpcDialog => OVERLAY_NPC_DIALOG_Z,
        OverlayModalPriority::Chat => OVERLAY_CHAT_Z,
        OverlayModalPriority::Death => OVERLAY_DEATH_Z,
        OverlayModalPriority::SystemMenu => OVERLAY_MENU_Z,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativePlayerUiIntent {
    UseItem {
        key: Option<String>,
        unique_id: Option<u64>,
        slot: Option<u8>,
        grid: Option<String>,
    },
    EquipItem {
        unique_id: u64,
        grid: String,
        to: i32,
    },
    RemoveItem {
        unique_id: u64,
        grid: String,
        to: i32,
    },
    DropItem {
        key: String,
        unique_id: u64,
        count: u16,
        hero_inventory: bool,
    },
    MoveItem {
        grid: String,
        unique_id: u64,
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
    Chat {
        message: String,
    },
    ReadMail {
        mail_id: u64,
    },
    ClaimMail {
        mail_id: u64,
    },
    DeleteMail {
        mail_id: u64,
    },
    SendMail {
        recipient: String,
        message: String,
        gold: u32,
        attachment_unique_ids: Vec<u64>,
    },
    // Shop
    /// Cash GameShop purchase. The host maps this only to `gameShopBuy`.
    GameShopBuy {
        request_id: String,
        g_index: i32,
        quantity: u8,
        price_type: i32,
    },
    BuyItem {
        item_index: u64,
        count: u16,
    },
    SellItem {
        unique_id: u64,
        count: u16,
    },
    RepairItem {
        unique_id: u64,
    },
    SRepairItem {
        unique_id: u64,
    },
    // Storage
    StoreItem {
        unique_id: u64,
        from: i32,
        to: i32,
    },
    TakeBackItem {
        unique_id: u64,
        from: i32,
        to: i32,
    },
    UnlockStorage {
        password: String,
    },
    SetStoragePassword {
        current: String,
        new_password: String,
    },
    RemoveStoragePassword {
        current: String,
    },
    ExpandStorage,
    GroupSwitch {
        allow_group: bool,
    },
    GroupAddMember {
        name: String,
    },
    GroupRemoveMember {
        name: String,
    },
    GroupInvite {
        accept_invite: bool,
    },
    GuildRequestInfo {
        info_type: u8,
    },
    GuildEditMember {
        change_type: u8,
        rank_index: u8,
        name: String,
        rank_name: String,
    },
    GuildEditNotice {
        notice: Vec<String>,
    },
    GuildInvite {
        accept_invite: bool,
    },
    TradeRequest,
    TradeReply {
        accept_invite: bool,
    },
    TradeGold {
        amount: u32,
    },
    TradeDepositItem {
        from: i32,
        to: i32,
    },
    TradeRetrieveItem {
        from: i32,
        to: i32,
    },
    TradeConfirm {
        locked: bool,
    },
    TradeCancel,
}

impl NativePlayerUiIntent {
    fn pending_key(&self) -> Option<PendingOperationKey> {
        match self {
            Self::ReadMail { mail_id } => Some(PendingOperationKey::ReadMail(*mail_id)),
            Self::ClaimMail { mail_id } => Some(PendingOperationKey::ClaimMail(*mail_id)),
            Self::DeleteMail { mail_id } => Some(PendingOperationKey::DeleteMail(*mail_id)),
            Self::SendMail {
                recipient,
                message,
                gold,
                attachment_unique_ids,
            } => Some(PendingOperationKey::SendMail {
                recipient: recipient.clone(),
                message: message.clone(),
                gold: *gold,
                attachment_unique_ids: attachment_unique_ids.clone(),
            }),
            Self::BuyItem { item_index, count } => Some(PendingOperationKey::Buy {
                item_index: *item_index,
                count: *count,
            }),
            Self::GameShopBuy { request_id, .. } => {
                Some(PendingOperationKey::GameShop(request_id.clone()))
            }
            Self::SellItem { unique_id, count } => Some(PendingOperationKey::Sell {
                unique_id: *unique_id,
                count: *count,
            }),
            Self::RepairItem { unique_id } => Some(PendingOperationKey::Repair(*unique_id)),
            Self::SRepairItem { unique_id } => Some(PendingOperationKey::SpecialRepair(*unique_id)),
            Self::DropItem {
                unique_id,
                count,
                hero_inventory,
                ..
            } => Some(PendingOperationKey::Drop {
                unique_id: *unique_id,
                count: *count,
                hero_inventory: *hero_inventory,
            }),
            Self::MoveItem {
                grid,
                unique_id,
                from,
                to,
            } => Some(PendingOperationKey::Move {
                grid: grid.clone(),
                unique_id: *unique_id,
                from: *from,
                to: *to,
            }),
            Self::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            } => Some(PendingOperationKey::Merge {
                grid_from: grid_from.clone(),
                grid_to: grid_to.clone(),
                id_from: *id_from,
                id_to: *id_to,
            }),
            Self::SplitItem {
                unique_id,
                grid,
                count,
            } => Some(PendingOperationKey::Split {
                grid: grid.clone(),
                unique_id: *unique_id,
                count: *count,
            }),
            Self::StoreItem {
                unique_id,
                from,
                to,
            } => Some(PendingOperationKey::StorageDeposit {
                unique_id: *unique_id,
                from: *from,
                to: *to,
            }),
            Self::TakeBackItem {
                unique_id,
                from,
                to,
            } => Some(PendingOperationKey::StorageWithdraw {
                unique_id: *unique_id,
                from: *from,
                to: *to,
            }),
            Self::UnlockStorage { .. } => Some(PendingOperationKey::StorageUnlock),
            Self::SetStoragePassword { .. } => Some(PendingOperationKey::StorageSetPassword),
            Self::RemoveStoragePassword { .. } => Some(PendingOperationKey::StorageRemovePassword),
            Self::ExpandStorage => Some(PendingOperationKey::StorageExpand),
            Self::GroupSwitch { .. }
            | Self::GroupAddMember { .. }
            | Self::GroupRemoveMember { .. }
            | Self::GroupInvite { .. }
            | Self::GuildRequestInfo { .. }
            | Self::GuildEditMember { .. }
            | Self::GuildEditNotice { .. }
            | Self::GuildInvite { .. }
            | Self::TradeRequest
            | Self::TradeReply { .. }
            | Self::TradeGold { .. }
            | Self::TradeDepositItem { .. }
            | Self::TradeRetrieveItem { .. }
            | Self::TradeConfirm { .. }
            | Self::TradeCancel => None,
            Self::UseItem { .. }
            | Self::EquipItem { .. }
            | Self::RemoveItem { .. }
            | Self::Chat { .. } => None,
        }
    }
}

#[derive(Debug, Default, Resource)]
pub struct NativePlayerUiIntentQueue {
    intents: VecDeque<NativePlayerUiIntent>,
}

impl NativePlayerUiIntentQueue {
    pub fn push_intent(&mut self, intent: NativePlayerUiIntent) -> bool {
        if self.intents.len() >= MAX_QUEUED {
            if matches!(intent, NativePlayerUiIntent::GameShopBuy { .. }) {
                return false;
            }
            let Some(index) = self
                .intents
                .iter()
                .position(|queued| !matches!(queued, NativePlayerUiIntent::GameShopBuy { .. }))
            else {
                return false;
            };
            self.intents.remove(index);
        }
        self.intents.push_back(intent);
        true
    }

    pub fn push_social_pending(
        &mut self,
        social: &mut crate::social::SocialModel,
        intent: NativePlayerUiIntent,
    ) -> bool {
        // Social operations are paired with a pending readback. A full queue
        // must reject the new operation before that pending state is created;
        // otherwise the generic queue's eviction policy could silently drop
        // an older intent while leaving a new social operation stuck forever.
        // The generic queue preserves transactional GameShop entries while
        // retaining bounded oldest-nontransaction eviction for normal work.
        if self.intents.len() >= MAX_QUEUED {
            return false;
        }
        if self.intents.iter().any(|queued| queued == &intent) {
            return false;
        }
        let operation = match &intent {
            NativePlayerUiIntent::GroupSwitch { allow_group } => {
                Some(crate::social::SocialPendingOperation::GroupSwitch {
                    allow_group: *allow_group,
                })
            }
            NativePlayerUiIntent::GroupAddMember { name } => {
                Some(crate::social::SocialPendingOperation::GroupAdd { name: name.clone() })
            }
            NativePlayerUiIntent::GroupRemoveMember { name } => {
                Some(crate::social::SocialPendingOperation::GroupRemove { name: name.clone() })
            }
            NativePlayerUiIntent::GroupInvite {
                accept_invite: true,
            } => Some(crate::social::SocialPendingOperation::GroupInviteAccept),
            NativePlayerUiIntent::GuildRequestInfo { .. } => {
                Some(crate::social::SocialPendingOperation::GuildInfo)
            }
            NativePlayerUiIntent::GuildEditMember { name, .. } => {
                Some(crate::social::SocialPendingOperation::GuildMember { name: name.clone() })
            }
            NativePlayerUiIntent::GuildEditNotice { .. } => {
                Some(crate::social::SocialPendingOperation::GuildNotice)
            }
            NativePlayerUiIntent::GuildInvite {
                accept_invite: true,
            } => Some(crate::social::SocialPendingOperation::GuildInviteAccept),
            NativePlayerUiIntent::TradeRequest => {
                Some(crate::social::SocialPendingOperation::TradeRequest)
            }
            NativePlayerUiIntent::TradeReply { .. } => {
                Some(crate::social::SocialPendingOperation::TradeReply)
            }
            NativePlayerUiIntent::TradeGold { amount } => {
                Some(crate::social::SocialPendingOperation::TradeGold { amount: *amount })
            }
            NativePlayerUiIntent::TradeDepositItem { from, to } => {
                Some(crate::social::SocialPendingOperation::TradeDeposit {
                    from: *from,
                    to: *to,
                })
            }
            NativePlayerUiIntent::TradeRetrieveItem { from, to } => {
                Some(crate::social::SocialPendingOperation::TradeRetrieve {
                    from: *from,
                    to: *to,
                })
            }
            NativePlayerUiIntent::TradeConfirm { locked } => {
                Some(crate::social::SocialPendingOperation::TradeConfirm { locked: *locked })
            }
            NativePlayerUiIntent::TradeCancel => {
                Some(crate::social::SocialPendingOperation::TradeCancel)
            }
            _ => None,
        };
        if let Some(operation) = operation {
            if !social.begin_pending(operation) {
                return false;
            }
        }
        self.push_intent(intent)
    }

    /// Queue a client-only transient intent without adding a server pending
    /// key. This remains useful for non-authoritative local intents; server
    /// operations should use [`Self::push_pending_intent`].
    pub fn push_transient_unique(&mut self, intent: NativePlayerUiIntent) -> bool {
        if self.intents.iter().any(|queued| queued == &intent) {
            return false;
        }
        self.push_intent(intent)
    }

    pub fn drain_intents(&mut self) -> Vec<NativePlayerUiIntent> {
        self.intents.drain(..).collect()
    }

    /// Queue a server-authoritative operation only when the same logical key
    /// is not already awaiting readback.
    pub fn push_pending_intent(
        &mut self,
        pending: &mut PendingOperations,
        intent: NativePlayerUiIntent,
    ) -> bool {
        // Mail ACKs have no request id. Do not let a second claim/send with a
        // different id or draft pass through merely because its exact key is
        // different; the gateway can correlate only one of each class.
        if matches!(
            &intent,
            NativePlayerUiIntent::ClaimMail { .. } | NativePlayerUiIntent::SendMail { .. }
        ) && pending.has_pending_mail_operation()
        {
            return false;
        }
        if self.intents.len() >= MAX_QUEUED {
            return false;
        }
        let Some(key) = intent.pending_key() else {
            return self.push_intent(intent);
        };
        if !pending.try_begin(key.clone()) {
            return false;
        }
        if !self.push_intent(intent) {
            pending.release(&key);
            return false;
        }
        true
    }

    /// Atomically reserve and enqueue the one in-flight native GameShop
    /// purchase. No correlation state is left behind when capacity or any
    /// reservation step fails.
    pub fn enqueue_game_shop_purchase(
        &mut self,
        core: &mut mir2_ui_core::state::UiState,
        game_shop: &mut GameShopModel,
        pending: &mut PendingOperations,
        g_index: i32,
        quantity: u8,
        price_type: i32,
    ) -> Option<GameShopRequest> {
        if self.intents.len() >= MAX_QUEUED
            || self
                .intents
                .iter()
                .any(|intent| matches!(intent, NativePlayerUiIntent::GameShopBuy { .. }))
        {
            return None;
        }

        let request = core.begin_game_shop_purchase(g_index, quantity, price_type)?;
        if !game_shop.reserve_purchase(request.clone()) {
            core.cancel_game_shop_purchase(&request.request_id);
            return None;
        }
        let key = PendingOperationKey::GameShop(request.request_id.clone());
        if !pending.try_begin(key.clone()) {
            game_shop.cancel_purchase_reservation(&request.request_id);
            core.cancel_game_shop_purchase(&request.request_id);
            return None;
        }
        if !self.push_intent(NativePlayerUiIntent::GameShopBuy {
            request_id: request.request_id.clone(),
            g_index: request.g_index,
            quantity: request.quantity,
            price_type: request.price_type,
        }) {
            pending.release(&key);
            game_shop.cancel_purchase_reservation(&request.request_id);
            core.cancel_game_shop_purchase(&request.request_id);
            return None;
        }
        Some(request)
    }

    pub fn clear(&mut self) {
        self.intents.clear();
    }
}

#[derive(Component)]
struct OverlayRoot;

#[derive(Component)]
struct OverlayInventory;

#[derive(Component)]
struct OverlayEquipment;

#[derive(Component)]
struct OverlayMenu;

#[derive(Component)]
struct OverlaySkill;

#[derive(Component)]
struct OverlayInspect;

#[derive(Component)]
struct OverlayDeath;

#[derive(Component)]
struct OverlayChatDraft;

#[derive(Component)]
struct OverlayMail;

#[derive(Component)]
struct OverlayBigMap;

#[derive(Component)]
struct OverlayShop;

#[derive(Component)]
struct OverlayGameShop;

#[derive(Component)]
struct OverlayStorage;

#[derive(Component)]
struct OverlayOptions;

#[derive(Component)]
struct OverlaySocial;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayButton {
    CloseWindows,
    CloseMail,
    CloseBigMap,
    CloseShop,
    CloseGameShop,
    CloseStorage,
    CloseOptions,
    OptionsMusicToggle,
    OptionsMusicVolumeDown,
    OptionsMusicVolumeUp,
    OptionsSoundToggle,
    OptionsSoundVolumeDown,
    OptionsSoundVolumeUp,
    OptionsWindowed,
    OptionsFullscreen,
    OptionsApply,
    OptionsCancel,
    OptionsDefaults,
    ToggleInventory,
    ToggleEquipment,
    ToggleShop,
    ToggleNpcShop,
    ToggleStorage,
    ToggleGroup,
    ToggleGuild,
    ToggleTrade,
    CloseSocial,
    GroupInviteAccept,
    GroupInviteDecline,
    GroupLeave,
    GroupAddSelected,
    GroupRemoveSelected,
    SelectGroupMember(u8),
    GuildRequestInfo,
    GuildInviteAccept,
    GuildInviteDecline,
    GuildPublishNotice,
    SelectGuildMember(u8),
    GuildKickSelected,
    TradeRequest,
    TradeAccept,
    TradeDecline,
    TradeGoldOffer,
    TradeDepositItem(u8),
    TradeConfirm,
    TradeCancel,
    Logout,
    UseInspected,
    EquipInspected,
    UnequipInspected,
    DropInspected,
    SplitInspected,
    SplitCountDec,
    SplitCountInc,
    ArmMoveInspected,
    ArmMergeInspected,
    CancelInventoryOperation,
    InspectBag(u32),
    InspectEquip(u32),
    SelectMail(u64),
    ClaimMail(u64),
    DeleteMail(u64),
    OpenMailCompose,
    MailRecipientFocus,
    MailMessageFocus,
    MailGoldInc,
    MailGoldDec,
    AddMailAttachment(u64),
    RemoveMailAttachment(u64),
    SubmitMail,
    CancelMailCompose,
    BigMapZoomIn,
    BigMapZoomOut,
    // NPC shop
    SelectShopGood(u64),
    ShopBuy,
    ShopSell,
    ShopRepair,
    ShopSRepair,
    ShopQuantityInc,
    ShopQuantityDec,
    ShopConfirm,
    ShopCancel,
    SelectBagForSell(u32),
    SelectBagForRepair(u32),
    SelectEquipForRepair(u32),
    // Cash shop
    SelectGameShopGood(i32),
    GameShopPaymentCredit,
    GameShopPaymentGold,
    GameShopBuy,
    GameShopQuantityInc,
    GameShopQuantityDec,
    GameShopPagePrev,
    GameShopPageNext,
    // Storage
    SelectBagForStore(u32),
    SelectStorage(u32),
    StorageDeposit,
    StorageWithdraw,
    StorageUnlock,
    StorageSetPassword,
    StorageRemovePassword,
    StorageExpand,
}

pub struct Mir2CrystalOverlayPlugin;

impl Plugin for Mir2CrystalOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<UiEffectQueue>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<InventoryOperationFeedback>()
            .init_resource::<AuthoritativeModelRevisions>()
            .init_resource::<SessionResetRevision>()
            .init_resource::<SessionResetGameShopPreservation>()
            .init_resource::<NativeSessionBoundaryTracker>()
            .init_resource::<OverlayResetTracker>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MailComposeUi>()
            .init_resource::<MapModel>()
            .init_resource::<UiReadModel>()
            .init_resource::<UiSurfaceSignals>()
            .init_resource::<NativeShellModel>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<ShopModel>()
            .init_resource::<GameShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<SkillModel>()
            .init_resource::<crate::social::SocialModel>()
            .init_resource::<crate::options_effects::OptionsRuntime>()
            .init_resource::<crate::audio::NativeAudioRuntime>()
            .add_systems(Startup, spawn_overlay_root)
            .add_systems(
                Startup,
                (
                    crate::options_effects::load_persisted_options,
                    crate::audio::initialize_native_audio,
                )
                    .chain(),
            )
            .configure_sets(
                Update,
                NativePlayerUiSet::Mutate.before(NativePlayerUiSet::Read),
            )
            .configure_sets(
                Update,
                PendingLifecycleSet::UiReset.before(NativePlayerUiSet::Mutate),
            )
            .add_systems(
                Update,
                crate::pending_operations::apply_overlay_session_reset
                    .in_set(PendingLifecycleSet::UiReset),
            )
            .add_systems(
                Update,
                (
                    consume_mail_operation_feedback,
                    consume_hud_buttons,
                    process_overlay_keyboard,
                    process_overlay_buttons,
                    crate::pending_operations::observe_native_session_boundary,
                    reconcile_native_game_shop_ui_state,
                    crate::options_effects::consume_options_effects,
                    crate::audio::sync_native_audio,
                )
                    .chain()
                    .in_set(NativePlayerUiSet::Mutate),
            )
            .add_systems(Update, render_overlays.in_set(NativePlayerUiSet::Read));
    }
}

/// Keep the native overlay's shared UiState correlation shadow aligned with
/// the runtime-owned GameShopModel. The runtime is feature-agnostic and owns
/// the receipt gate; this adapter is the only native-ui bridge that may mirror
/// or release UiState's pending request.
pub fn reconcile_native_game_shop_ui_state(
    mut state: ResMut<NativePlayerUiState>,
    game_shop: Res<GameShopModel>,
) {
    if let Some(pending) = game_shop.pending_purchase.as_ref() {
        if state.core.game_shop_pending.is_none() {
            state.core.game_shop_pending = Some(pending.clone());
            state.core.game_shop_next_request_id = game_shop.next_request_id;
            state.core.game_shop_unknown = game_shop.purchase_unknown;
        }
        return;
    }

    if let Some(receipt) = game_shop.last_receipt.as_ref() {
        if state
            .core
            .game_shop_pending
            .as_ref()
            .is_some_and(|request| receipt.matches_request(request))
        {
            let _ = state.core.apply_game_shop_receipt(receipt.clone());
        }
    } else if game_shop.purchase_unknown {
        state.core.mark_game_shop_unknown();
    } else if state.core.game_shop_pending.is_some() {
        state.core.clear_game_shop_session();
    }
    state.core.game_shop_next_request_id = game_shop.next_request_id;
}

pub fn inspect_label(item: &ItemModel) -> String {
    format!(
        "{}  x{}  [{}] slot {}",
        if item.name.is_empty() {
            item.key.as_str()
        } else {
            item.name.as_str()
        },
        item.quantity,
        container_name(item.container),
        item.slot
    )
}

pub fn container_name(container: u8) -> &'static str {
    match container {
        1 => "belt",
        2 => "equipment",
        4 => "storage",
        _ => "inventory",
    }
}

pub fn item_unique_id(item: &ItemModel) -> Option<u64> {
    item.unique_id
}

pub fn equip_destination_for_name(name: &str) -> i32 {
    let lowered = name.to_ascii_lowercase();
    if lowered.contains("pendant") || lowered.contains("necklace") {
        4
    } else if lowered.contains("ring") {
        7
    } else if lowered.contains("bracelet") {
        5
    } else if lowered.contains("helmet") {
        2
    } else if lowered.contains("boot") {
        11
    } else if lowered.contains("belt") {
        10
    } else if lowered.contains("armour") || lowered.contains("armor") {
        1
    } else {
        0
    }
}

pub fn equipment_slot_name(slot: u32) -> &'static str {
    match slot {
        0 => "Weapon",
        1 => "Armour",
        2 => "Helmet",
        3 => "Torch",
        4 => "Necklace",
        5 => "Bracelet L",
        6 => "Bracelet R",
        7 => "Ring L",
        8 => "Ring R",
        9 => "Amulet",
        10 => "Belt",
        11 => "Boots",
        12 => "Stone",
        13 => "Mount",
        _ => "Slot",
    }
}

pub fn belt_use_intent(slot: u8) -> NativePlayerUiIntent {
    NativePlayerUiIntent::UseItem {
        key: None,
        unique_id: None,
        slot: Some(slot),
        grid: Some("belt".to_owned()),
    }
}

fn spawn_overlay_root(mut commands: Commands) {
    commands
        .spawn((
            OverlayRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(1024.0),
                height: Val::Px(768.0),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(985),
            BackgroundColor(Color::NONE),
        ))
        .with_children(|root| {
            root.spawn((
                OverlayInventory,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    top: Val::Px(170.0),
                    width: Val::Px(360.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayEquipment,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(150.0),
                    top: Val::Px(170.0),
                    width: Val::Px(280.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayMenu,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(412.0),
                    top: Val::Px(260.0),
                    width: Val::Px(200.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                GlobalZIndex(OVERLAY_MENU_Z),
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlaySkill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    top: Val::Px(80.0),
                    width: Val::Px(280.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayInspect,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(390.0),
                    top: Val::Px(170.0),
                    width: Val::Px(240.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayDeath,
                GlobalZIndex(OVERLAY_DEATH_Z),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(312.0),
                    top: Val::Px(240.0),
                    width: Val::Px(400.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(12.0)),
                    row_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.18, 0.03, 0.03, 0.88)),
            ));
            root.spawn((
                OverlayChatDraft,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(230.0),
                    top: Val::Px(648.0),
                    width: Val::Px(420.0),
                    height: Val::Px(20.0),
                    display: Display::None,
                    padding: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ));
            root.spawn((
                OverlayMail,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(212.0),
                    top: Val::Px(80.0),
                    width: Val::Px(600.0),
                    max_height: Val::Px(520.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayBigMap,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(112.0),
                    top: Val::Px(80.0),
                    width: Val::Px(800.0),
                    height: Val::Px(500.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayShop,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(112.0),
                    top: Val::Px(80.0),
                    width: Val::Px(620.0),
                    max_height: Val::Px(520.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayGameShop,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(112.0),
                    top: Val::Px(80.0),
                    width: Val::Px(620.0),
                    max_height: Val::Px(520.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayStorage,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(150.0),
                    top: Val::Px(100.0),
                    width: Val::Px(640.0),
                    max_height: Val::Px(520.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlayOptions,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(212.0),
                    top: Val::Px(100.0),
                    width: Val::Px(600.0),
                    max_height: Val::Px(480.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
            root.spawn((
                OverlaySocial,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(172.0),
                    top: Val::Px(80.0),
                    width: Val::Px(680.0),
                    max_height: Val::Px(560.0),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(10.0)),
                    row_gap: Val::Px(5.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));
        });
}

fn consume_hud_buttons(
    mut state: ResMut<NativePlayerUiState>,
    buttons: Query<(&Interaction, &CrystalHudAction), Changed<Interaction>>,
    shell: Option<Res<NativeShellModel>>,
) {
    if !shell.is_some_and(|model| model.screen == NativeShellScreen::InGame) {
        return;
    }
    for (interaction, action) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            CrystalHudAction::Inventory => {
                state.toggle_inventory();
                if !state.inventory_open() {
                    state.inspect = None;
                }
            }
            CrystalHudAction::Character => {
                state.toggle_equipment();
            }
            CrystalHudAction::Menu => {
                state.toggle_menu();
            }
            CrystalHudAction::Skill => {
                state.toggle_skill();
            }
            CrystalHudAction::Quest => {
                state.toggle_quest();
            }
            CrystalHudAction::Option => {
                state.toggle_options();
            }
            CrystalHudAction::Mail => {
                state.toggle_mail();
            }
            CrystalHudAction::BigMap => {
                state.toggle_bigmap();
            }
            CrystalHudAction::MinimapToggle => {
                state.toggle_minimap();
            }
            CrystalHudAction::GameShop => {
                state.toggle_shop();
                if !state.shop_open() {
                    state.shop_quantity = 1;
                }
            }
        }
    }
}

fn consume_mail_operation_feedback(
    mut mail: ResMut<MailModel>,
    mut state: ResMut<NativePlayerUiState>,
    mut compose: ResMut<MailComposeUi>,
) {
    let Some(feedback) = mail.operation_feedback().cloned() else {
        return;
    };
    let text = match (feedback.kind, feedback.success) {
        (MailOperationKind::Send, true) => "Mail sent successfully".to_owned(),
        (MailOperationKind::Send, false) => "Mail was rejected; draft kept".to_owned(),
        (MailOperationKind::Collect, true) => "Mail attachments claimed".to_owned(),
        (MailOperationKind::Collect, false) => "Mail claim failed".to_owned(),
        (MailOperationKind::Delete, true) => "Mail deleted".to_owned(),
        (MailOperationKind::Delete, false) => "Mail delete failed".to_owned(),
        (MailOperationKind::Read, true) => "Mail opened".to_owned(),
        (MailOperationKind::Read, false) => "Mail read failed".to_owned(),
    };
    if matches!(feedback.kind, MailOperationKind::Send) && feedback.success {
        state.core.mail_compose = None;
    }
    compose.last_notice = Some(text);
    mail.mails.retain(|message| message.operation.is_none());
}

pub(crate) fn process_overlay_keyboard(
    mut state: ResMut<NativePlayerUiState>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    mut pending: ResMut<PendingOperations>,
    mut shell_intents: ResMut<NativeUiIntentQueue>,
    mut shell: ResMut<NativeShellModel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<KeyboardInput>,
    inventory: Res<InventoryModel>,
    mut compose_ui: ResMut<MailComposeUi>,
    mut game_shop: Option<ResMut<GameShopModel>>,
    mut storage: ResMut<StorageModel>,
    chat_state: Option<Res<crate::crystal_ui::chat::CrystalChatState>>,
    npc_dialog: Option<Res<NpcDialogModel>>,
    mut surface_signals: Option<ResMut<UiSurfaceSignals>>,
) {
    if shell.screen != NativeShellScreen::InGame {
        // Session ownership is reset exactly once through
        // SessionResetRevision/apply_overlay_session_reset. Keyboard handling
        // must never race the typed preserving boundary or clear GameShop
        // correlation after runtime receipt ingest.
        return;
    }

    if state.core.mail_compose_open() {
        if keys.just_pressed(KeyCode::Escape) {
            dispatch_ui_action(
                &mut state.core,
                &mut UiEffectQueue::default(),
                mir2_ui_core::action::UiAction::CancelMailCompose,
            );
            compose_ui.last_notice = None;
            return;
        }
        if keys.just_pressed(KeyCode::Tab) {
            compose_ui.focus = match compose_ui.focus {
                MailComposeFocus::Recipient => MailComposeFocus::Message,
                MailComposeFocus::Message => MailComposeFocus::Gold,
                MailComposeFocus::Gold => MailComposeFocus::Recipient,
            };
        }
        if keys.just_pressed(KeyCode::Backspace) {
            if let Some(draft) = state.core.mail_compose.as_ref() {
                match compose_ui.focus {
                    MailComposeFocus::Recipient => {
                        let mut value = draft.recipient.clone();
                        value.pop();
                        state.core.mail_compose.as_mut().unwrap().recipient = value;
                    }
                    MailComposeFocus::Message => {
                        let mut value = draft.message.clone();
                        value.pop();
                        state.core.mail_compose.as_mut().unwrap().message = value;
                    }
                    MailComposeFocus::Gold => {}
                }
            }
        }
        for event in typed.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            let Some(text) = &event.text else { continue };
            match compose_ui.focus {
                MailComposeFocus::Recipient => {
                    let mut value = state
                        .core
                        .mail_compose
                        .as_ref()
                        .map(|draft| draft.recipient.clone())
                        .unwrap_or_default();
                    value.extend(text.chars().filter(|ch| !ch.is_control()));
                    dispatch_ui_action(
                        &mut state.core,
                        &mut UiEffectQueue::default(),
                        mir2_ui_core::action::UiAction::SetMailRecipient { recipient: value },
                    );
                }
                MailComposeFocus::Message => {
                    let mut value = state
                        .core
                        .mail_compose
                        .as_ref()
                        .map(|draft| draft.message.clone())
                        .unwrap_or_default();
                    value.extend(text.chars().filter(|ch| !ch.is_control()));
                    dispatch_ui_action(
                        &mut state.core,
                        &mut UiEffectQueue::default(),
                        mir2_ui_core::action::UiAction::SetMailMessage { message: value },
                    );
                }
                MailComposeFocus::Gold => {}
            }
        }
        return;
    }

    if let Some(signals) = surface_signals.as_deref_mut() {
        if signals.npc_shop_open_requested {
            if !state.npc_shop_open() {
                state.toggle_npc_shop();
            }
            signals.npc_shop_open_requested = false;
        }
    }

    if state.chat_focused() {
        if keys.just_pressed(KeyCode::Escape) {
            state.set_chat_focused(false);
            state.chat_draft.clear();
            return;
        }
        if keys.just_pressed(KeyCode::Enter) {
            let raw = state.chat_draft.clone();
            let message_opt = if let Some(cs) = chat_state.as_deref() {
                crate::crystal_ui::chat::format_chat_for_filter(cs.filter, &raw)
            } else {
                let t = raw.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_owned())
                }
            };
            state.chat_draft.clear();
            state.set_chat_focused(false);
            if let Some(message) = message_opt {
                if !message.is_empty() {
                    intents.push_intent(NativePlayerUiIntent::Chat { message });
                }
            }
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            state.chat_draft.pop();
        }
        for event in typed.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if let Some(text) = &event.text {
                for ch in text.chars() {
                    if !ch.is_control() && state.chat_draft.chars().count() < 60 {
                        state.chat_draft.push(ch);
                    }
                }
            }
        }
        return;
    }

    // Storage password typing when storage is open and locked: capture text input for password draft
    if state.storage_open() && storage.has_password && !storage.unlocked {
        if keys.just_pressed(KeyCode::Backspace) {
            storage.password_draft.pop();
        }
        for event in typed.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if let Some(text) = &event.text {
                for ch in text.chars() {
                    if !ch.is_control() && storage.password_draft.chars().count() < 16 {
                        storage.password_draft.push(ch);
                    }
                }
            }
        }
        if keys.just_pressed(KeyCode::Enter) {
            if storage_unlock_enabled(&storage) {
                let pwd = storage.password_draft.clone();
                intents.push_pending_intent(
                    &mut pending,
                    NativePlayerUiIntent::UnlockStorage { password: pwd },
                );
            }
            return;
        }
        if keys.just_pressed(KeyCode::Escape) {
            // let Escape close windows below
        } else {
            // When locked, block other hotkeys
            return;
        }
    }

    if keys.just_pressed(KeyCode::Enter) {
        state.set_chat_focused(true);
        return;
    }
    // Quest/NPC surfaces own Escape. This system is ordered before the quest
    // input system so returning here preserves the pre-key modal state for the
    // dedicated handler and prevents the same key from opening Menu after it
    // closes Quest/Dialog later in this update.
    if keys.just_pressed(KeyCode::Escape)
        && (state.quest_open() || npc_dialog.as_deref().is_some_and(|dialog| dialog.is_open))
    {
        return;
    }
    if keys.just_pressed(KeyCode::KeyI) {
        let was_open = state.inventory_open();
        state.toggle_inventory();
        if !state.inventory_open() {
            state.inspect = None;
        } else if was_open {
            // already handled
        }
    }
    if keys.just_pressed(KeyCode::KeyC) {
        state.toggle_equipment();
    }
    if keys.just_pressed(KeyCode::KeyM) {
        state.toggle_mail();
    }
    if keys.just_pressed(KeyCode::KeyB) {
        state.toggle_bigmap();
    }
    if keys.just_pressed(KeyCode::KeyN) {
        state.toggle_minimap();
    }
    if keys.just_pressed(KeyCode::KeyO) {
        state.toggle_shop();
        if !state.shop_open() {
            state.shop_quantity = 1;
        }
    }
    if keys.just_pressed(KeyCode::KeyP) {
        state.toggle_storage();
        if state.storage_open() && storage.size == 0 {
            storage.size = STORAGE_BASE_SIZE;
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        if state.core.panel != mir2_ui_core::state::UiPanel::None || state.inspect.is_some() {
            // If shop/storage open, Escape acts as Cancel
            state.close_windows();
            state.shop_quantity = 1;
            storage.password_draft.clear();
            storage.new_password_draft.clear();
            storage.confirm_password_draft.clear();
        } else {
            state.toggle_menu();
        }
        return;
    }
    // BigMap zoom handling
    if state.bigmap_open() {
        if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
            state.zoom_in();
        }
        if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
            state.zoom_out();
        }
    }
    if keys.just_pressed(KeyCode::KeyU) {
        if let Some(intent) = inspected_use_intent(&state, &inventory) {
            intents.push_intent(intent);
        }
    }
    if keys.just_pressed(KeyCode::KeyG) {
        if let Some(intent) = inspected_equip_intent(&state, &inventory) {
            intents.push_intent(intent);
        }
    }
    if keys.just_pressed(KeyCode::KeyL) && state.menu_open() {
        let _ = shell.apply_ui_intent(NativeUiIntent::Logout);
        shell_intents.push(NativeUiIntent::Logout);
        state.close_windows();
    }
    // Cash GameShop quantity hotkeys use the cash model; NPC quantity stays
    // in the NPC-only state field.
    if state.shop_open() {
        let Some(game_shop) = game_shop.as_deref_mut() else {
            return;
        };
        if keys.just_pressed(KeyCode::BracketRight) || keys.just_pressed(KeyCode::Equal) {
            game_shop.quantity_inc();
        }
        if keys.just_pressed(KeyCode::BracketLeft) || keys.just_pressed(KeyCode::Minus) {
            game_shop.quantity_dec();
        }
    } else if state.npc_shop_open() {
        if keys.just_pressed(KeyCode::BracketRight) || keys.just_pressed(KeyCode::Equal) {
            state.shop_quantity_inc();
        }
        if keys.just_pressed(KeyCode::BracketLeft) || keys.just_pressed(KeyCode::Minus) {
            state.shop_quantity_dec();
        }
    }
}

fn process_overlay_buttons(
    mut state: ResMut<NativePlayerUiState>,
    mut effects: Option<ResMut<UiEffectQueue>>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    mut shell_intents: ResMut<NativeUiIntentQueue>,
    mut shell: ResMut<NativeShellModel>,
    inventory: Res<InventoryModel>,
    mut mail: ResMut<MailModel>,
    mut compose_ui: ResMut<MailComposeUi>,
    mut shop: ResMut<ShopModel>,
    game_shop: Option<ResMut<GameShopModel>>,
    mut storage: ResMut<StorageModel>,
    mut social: ResMut<crate::social::SocialModel>,
    ui: Option<Res<UiReadModel>>,
    combat_target: Option<Res<crate::quest_model::CombatTargetModel>>,
    mut pending: ResMut<PendingOperations>,
    buttons: Query<(&Interaction, &OverlayButton), Changed<Interaction>>,
) {
    if shell.screen != NativeShellScreen::InGame {
        return;
    }
    let mut fallback_effects = UiEffectQueue::default();
    let mut effects = effects.as_deref_mut().unwrap_or(&mut fallback_effects);
    let mut game_shop = game_shop;
    let (gold, credit, player_class) = ui
        .as_deref()
        .map(|model| {
            (
                model.player.gold,
                model.player.credit,
                model.player.class_name.clone().unwrap_or_default(),
            )
        })
        .unwrap_or((0, 0, String::new()));
    for (interaction, button) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            OverlayButton::CloseWindows => {
                state.close_windows();
                state.shop_quantity = 1;
            }
            OverlayButton::CloseMail => {
                if state.mail_open() {
                    state.core.panel = mir2_ui_core::state::UiPanel::None;
                }
            }
            OverlayButton::CloseBigMap => {
                if state.bigmap_open() {
                    state.core.panel = mir2_ui_core::state::UiPanel::None;
                }
            }
            OverlayButton::CloseShop => {
                if state.npc_shop_open() {
                    state.core.panel = mir2_ui_core::state::UiPanel::None;
                }
                state.shop_quantity = 1;
                state.shop_repair_container = 0;
                state.shop_repair_slot = None;
            }
            OverlayButton::CloseGameShop => {
                if state.shop_open() {
                    state.core.panel = mir2_ui_core::state::UiPanel::None;
                }
            }
            OverlayButton::CloseStorage => {
                if state.storage_open() {
                    state.core.panel = mir2_ui_core::state::UiPanel::None;
                }
                storage.password_draft.clear();
                storage.new_password_draft.clear();
                storage.confirm_password_draft.clear();
            }
            OverlayButton::CloseOptions => {
                if state.options_open() {
                    dispatch_ui_action(
                        &mut state.core,
                        &mut effects,
                        mir2_ui_core::action::UiAction::CancelOptions,
                    );
                }
            }
            OverlayButton::OptionsMusicToggle => {
                let value = !state
                    .core
                    .options_draft
                    .as_ref()
                    .unwrap_or(&state.core.options)
                    .music_enabled;
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetMusicEnabled { enabled: value },
                );
            }
            OverlayButton::OptionsMusicVolumeDown => {
                let value = state
                    .core
                    .options_draft
                    .as_ref()
                    .unwrap_or(&state.core.options)
                    .music_volume
                    .saturating_sub(10);
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetMusicVolume { volume: value },
                );
            }
            OverlayButton::OptionsMusicVolumeUp => {
                let value = state
                    .core
                    .options_draft
                    .as_ref()
                    .unwrap_or(&state.core.options)
                    .music_volume
                    .saturating_add(10)
                    .min(100);
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetMusicVolume { volume: value },
                );
            }
            OverlayButton::OptionsSoundToggle => {
                let value = !state
                    .core
                    .options_draft
                    .as_ref()
                    .unwrap_or(&state.core.options)
                    .sound_enabled;
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetSoundEnabled { enabled: value },
                );
            }
            OverlayButton::OptionsSoundVolumeDown => {
                let value = state
                    .core
                    .options_draft
                    .as_ref()
                    .unwrap_or(&state.core.options)
                    .sound_volume
                    .saturating_sub(10);
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetSoundVolume { volume: value },
                );
            }
            OverlayButton::OptionsSoundVolumeUp => {
                let value = state
                    .core
                    .options_draft
                    .as_ref()
                    .unwrap_or(&state.core.options)
                    .sound_volume
                    .saturating_add(10)
                    .min(100);
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetSoundVolume { volume: value },
                );
            }
            OverlayButton::OptionsWindowed => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetWindowMode {
                        mode: mir2_ui_core::state::UiWindowMode::Windowed,
                    },
                );
            }
            OverlayButton::OptionsFullscreen => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetWindowMode {
                        mode: mir2_ui_core::state::UiWindowMode::Fullscreen,
                    },
                );
            }
            OverlayButton::OptionsApply => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::ApplyOptions,
                );
            }
            OverlayButton::OptionsCancel => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::CancelOptions,
                );
            }
            OverlayButton::OptionsDefaults => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::ResetOptionsToDefaults,
                );
            }
            OverlayButton::ToggleInventory => {
                state.toggle_inventory();
                if !state.inventory_open() {
                    state.inspect = None;
                }
            }
            OverlayButton::ToggleEquipment => state.toggle_equipment(),
            OverlayButton::ToggleShop => {
                state.toggle_shop();
                if !state.shop_open() {
                    state.shop_quantity = 1;
                }
            }
            OverlayButton::ToggleNpcShop => {
                state.toggle_npc_shop();
            }
            OverlayButton::ToggleStorage => {
                state.toggle_storage();
                if state.storage_open() && storage.size == 0 {
                    storage.size = STORAGE_BASE_SIZE;
                }
            }
            OverlayButton::ToggleGroup => state.toggle_group(),
            OverlayButton::ToggleGuild => state.toggle_guild(),
            OverlayButton::ToggleTrade => state.toggle_trade(),
            OverlayButton::CloseSocial => {
                if state.group_open() || state.guild_open() || state.trade_open() {
                    state.core.panel = mir2_ui_core::state::UiPanel::None;
                }
            }
            OverlayButton::GroupInviteAccept => {
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GroupInvite {
                        accept_invite: true,
                    },
                );
            }
            OverlayButton::GroupInviteDecline => {
                intents.push_transient_unique(NativePlayerUiIntent::GroupInvite {
                    accept_invite: false,
                });
            }
            OverlayButton::GroupLeave => {
                if social.group.active {
                    intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::GroupSwitch { allow_group: false },
                    );
                }
            }
            OverlayButton::GroupAddSelected => {
                let Some(target) = combat_target
                    .as_deref()
                    .and_then(|model| model.target.as_ref())
                    .filter(|target| target.is_player)
                else {
                    continue;
                };
                let name = target.name.trim();
                if name.is_empty() || name.chars().count() > 32 {
                    continue;
                }
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GroupAddMember {
                        name: name.to_owned(),
                    },
                );
            }
            OverlayButton::SelectGroupMember(index) => {
                state.selected_group_member = Some(index);
            }
            OverlayButton::GroupRemoveSelected => {
                let Some(index) = state.selected_group_member else {
                    continue;
                };
                let Some(member) = social.group.members.get(usize::from(index)) else {
                    continue;
                };
                if member.leader || member.name.trim().is_empty() {
                    continue;
                }
                let name = member.name.clone();
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GroupRemoveMember { name },
                );
            }
            OverlayButton::GuildRequestInfo => {
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GuildRequestInfo { info_type: 0 },
                );
            }
            OverlayButton::GuildInviteAccept => {
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GuildInvite {
                        accept_invite: true,
                    },
                );
            }
            OverlayButton::GuildInviteDecline => {
                intents.push_transient_unique(NativePlayerUiIntent::GuildInvite {
                    accept_invite: false,
                });
            }
            OverlayButton::GuildPublishNotice => {
                if !social.guild.notice.is_empty() {
                    let notice = social.guild.notice.clone();
                    intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::GuildEditNotice { notice },
                    );
                }
            }
            OverlayButton::SelectGuildMember(index) => {
                state.selected_guild_member = Some(index);
            }
            OverlayButton::GuildKickSelected => {
                let Some(index) = state.selected_guild_member else {
                    continue;
                };
                let Some(member) = social.guild.members.get(usize::from(index)) else {
                    continue;
                };
                if member.name.trim().is_empty()
                    || !social
                        .guild
                        .permissions
                        .iter()
                        .any(|permission| permission.eq_ignore_ascii_case("kick"))
                {
                    continue;
                }
                let name = member.name.clone();
                let rank_index = member.rank_index.unwrap_or(0);
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GuildEditMember {
                        change_type: 1,
                        rank_index,
                        name,
                        rank_name: String::new(),
                    },
                );
            }
            OverlayButton::TradeRequest => {
                intents.push_social_pending(&mut social, NativePlayerUiIntent::TradeRequest);
            }
            OverlayButton::TradeAccept => {
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::TradeReply {
                        accept_invite: true,
                    },
                );
            }
            OverlayButton::TradeDecline => {
                intents.push_transient_unique(NativePlayerUiIntent::TradeReply {
                    accept_invite: false,
                });
            }
            OverlayButton::TradeGoldOffer => {
                if social.trade.state == "open" {
                    intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::TradeGold { amount: 100 },
                    );
                }
            }
            OverlayButton::TradeDepositItem(slot) => {
                if social.trade.state != "open" || slot >= 10 {
                    continue;
                }
                let Some(item) = inventory
                    .items
                    .iter()
                    .find(|item| item.container == 0 && item.slot == u32::from(slot))
                else {
                    continue;
                };
                if item_unique_id(item).is_none() {
                    continue;
                }
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::TradeDepositItem {
                        from: i32::from(slot),
                        to: 0,
                    },
                );
            }
            OverlayButton::TradeConfirm => {
                if social.trade.state == "open" && !social.trade.my_confirmed {
                    intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::TradeConfirm { locked: true },
                    );
                }
            }
            OverlayButton::TradeCancel => {
                intents.push_social_pending(&mut social, NativePlayerUiIntent::TradeCancel);
            }
            OverlayButton::Logout => {
                let _ = shell.apply_ui_intent(NativeUiIntent::Logout);
                shell_intents.push(NativeUiIntent::Logout);
                state.close_windows();
            }
            OverlayButton::UseInspected => {
                if let Some(intent) = inspected_use_intent(&state, &inventory) {
                    intents.push_intent(intent);
                }
            }
            OverlayButton::EquipInspected => {
                if let Some(intent) = inspected_equip_intent(&state, &inventory) {
                    intents.push_intent(intent);
                }
            }
            OverlayButton::UnequipInspected => {
                if let Some(intent) = inspected_remove_intent(&state, &inventory) {
                    intents.push_intent(intent);
                }
            }
            OverlayButton::DropInspected => {
                if let Some(item) = inspected_inventory_item(&state, &inventory) {
                    if let (Some(unique_id), Ok(count)) =
                        (item_unique_id(item), u16::try_from(item.quantity))
                    {
                        if count != 0 {
                            intents.push_pending_intent(
                                &mut pending,
                                NativePlayerUiIntent::DropItem {
                                    key: item.key.clone(),
                                    unique_id,
                                    count,
                                    hero_inventory: false,
                                },
                            );
                        }
                    }
                }
            }
            OverlayButton::SplitInspected => {
                if let Some(item) = inspected_inventory_item(&state, &inventory) {
                    if let Some(unique_id) = item_unique_id(item) {
                        let max = item.quantity.saturating_sub(1).min(u32::from(u16::MAX)) as u16;
                        let count = state.split_count.clamp(1, max.max(1));
                        if max > 0 {
                            intents.push_pending_intent(
                                &mut pending,
                                NativePlayerUiIntent::SplitItem {
                                    unique_id,
                                    grid: "inventory".to_owned(),
                                    count,
                                },
                            );
                        }
                    }
                }
            }
            OverlayButton::SplitCountDec => {
                state.split_count = state.split_count.saturating_sub(1).max(1);
            }
            OverlayButton::SplitCountInc => {
                if let Some(item) = inspected_inventory_item(&state, &inventory) {
                    let max = item.quantity.saturating_sub(1).min(u32::from(u16::MAX)) as u16;
                    state.split_count = state.split_count.saturating_add(1).min(max.max(1));
                }
            }
            OverlayButton::ArmMoveInspected => {
                if let Some(item) = inspected_inventory_item(&state, &inventory) {
                    if let Some(unique_id) = item_unique_id(item) {
                        state.inventory_operation = Some(InventoryOperationDraft::Move {
                            source_slot: item.slot,
                            unique_id,
                        });
                    }
                }
            }
            OverlayButton::ArmMergeInspected => {
                if let Some(item) = inspected_inventory_item(&state, &inventory) {
                    if let Some(unique_id) = item_unique_id(item) {
                        state.inventory_operation = Some(InventoryOperationDraft::Merge {
                            source_slot: item.slot,
                            unique_id,
                        });
                    }
                }
            }
            OverlayButton::CancelInventoryOperation => {
                state.inventory_operation = None;
            }
            OverlayButton::InspectBag(slot) => match state.inventory_operation.clone() {
                Some(InventoryOperationDraft::Move {
                    source_slot,
                    unique_id,
                }) if source_slot != slot => {
                    if intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::MoveItem {
                            grid: "inventory".to_owned(),
                            unique_id,
                            from: source_slot as i32,
                            to: slot as i32,
                        },
                    ) {
                        state.inventory_operation = None;
                        state.inspect = inventory
                            .items_in(0)
                            .into_iter()
                            .find(|item| item.slot == slot)
                            .map(inspect_from_item);
                    }
                }
                Some(InventoryOperationDraft::Merge {
                    source_slot,
                    unique_id: id_from,
                }) if source_slot != slot => {
                    let target = inventory
                        .items_in(0)
                        .into_iter()
                        .find(|item| item.slot == slot);
                    if let Some(id_to) = target.and_then(item_unique_id) {
                        if id_from != id_to
                            && intents.push_pending_intent(
                                &mut pending,
                                NativePlayerUiIntent::MergeItem {
                                    grid_from: "inventory".to_owned(),
                                    grid_to: "inventory".to_owned(),
                                    id_from,
                                    id_to,
                                },
                            )
                        {
                            state.inventory_operation = None;
                            state.inspect = target.map(inspect_from_item);
                        }
                    }
                }
                Some(_) => {}
                None => {
                    state.inspect = inventory
                        .items_in(0)
                        .into_iter()
                        .find(|item| item.slot == slot)
                        .map(inspect_from_item);
                    state.split_count = 1;
                }
            },
            OverlayButton::InspectEquip(slot) => {
                state.inspect = inventory
                    .items_in(2)
                    .into_iter()
                    .find(|item| item.slot == slot)
                    .map(inspect_from_item);
            }
            OverlayButton::SelectMail(id) => {
                mail.selected_id = Some(id);
                if mail
                    .mails
                    .iter()
                    .any(|message| message.id == id && !message.read)
                {
                    intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::ReadMail { mail_id: id },
                    );
                }
            }
            OverlayButton::ClaimMail(id) => {
                if let Some(msg) = mail.mails.iter().find(|m| m.id == id) {
                    if mail_claim_enabled(msg) {
                        intents.push_pending_intent(
                            &mut pending,
                            NativePlayerUiIntent::ClaimMail { mail_id: id },
                        );
                    }
                }
            }
            OverlayButton::DeleteMail(id) => {
                if let Some(msg) = mail.mails.iter().find(|m| m.id == id) {
                    if mail_delete_enabled(msg) {
                        intents.push_pending_intent(
                            &mut pending,
                            NativePlayerUiIntent::DeleteMail { mail_id: id },
                        );
                    }
                }
            }
            OverlayButton::OpenMailCompose => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::OpenMailCompose,
                );
                compose_ui.focus = MailComposeFocus::Recipient;
                compose_ui.last_notice = None;
            }
            OverlayButton::MailRecipientFocus => {
                compose_ui.focus = MailComposeFocus::Recipient;
            }
            OverlayButton::MailMessageFocus => {
                compose_ui.focus = MailComposeFocus::Message;
            }
            OverlayButton::MailGoldInc | OverlayButton::MailGoldDec => {
                if let Some(draft) = state.core.mail_compose.as_ref() {
                    let gold = if matches!(*button, OverlayButton::MailGoldInc) {
                        draft.gold.saturating_add(100)
                    } else {
                        draft.gold.saturating_sub(100)
                    };
                    dispatch_ui_action(
                        &mut state.core,
                        &mut effects,
                        mir2_ui_core::action::UiAction::SetMailGold { gold },
                    );
                }
            }
            OverlayButton::AddMailAttachment(unique_id) => {
                if mail_attachment_is_current(&inventory, unique_id) {
                    dispatch_ui_action(
                        &mut state.core,
                        &mut effects,
                        mir2_ui_core::action::UiAction::AddMailAttachment { unique_id },
                    );
                }
            }
            OverlayButton::RemoveMailAttachment(unique_id) => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::RemoveMailAttachment { unique_id },
                );
            }
            OverlayButton::SubmitMail => {
                let Some(draft) = state.core.mail_compose.clone() else {
                    continue;
                };
                let Some(ids) = valid_mail_attachment_ids(&inventory, &draft.attachment_unique_ids)
                else {
                    compose_ui.last_notice =
                        Some("Attachment is no longer in Bag1/Bag2".to_owned());
                    continue;
                };
                if draft.recipient.trim().is_empty() || draft.message.trim().is_empty() {
                    compose_ui.last_notice = Some("Recipient and message are required".to_owned());
                    continue;
                }
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SubmitMail,
                );
                for command in effects.drain_mail_commands() {
                    if let mir2_ui_core::effect::GatewayCommand::SendMail {
                        recipient,
                        message,
                        gold,
                        ..
                    } = command
                    {
                        let intent = NativePlayerUiIntent::SendMail {
                            recipient,
                            message,
                            gold,
                            attachment_unique_ids: ids.clone(),
                        };
                        if !intents.push_pending_intent(&mut pending, intent) {
                            compose_ui.last_notice = Some("Mail is already being sent".to_owned());
                        } else {
                            compose_ui.last_notice = Some("Sending mail…".to_owned());
                        }
                    }
                }
            }
            OverlayButton::CancelMailCompose => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::CancelMailCompose,
                );
                compose_ui.last_notice = None;
            }
            OverlayButton::BigMapZoomIn => state.zoom_in(),
            OverlayButton::BigMapZoomOut => state.zoom_out(),
            // Shop
            OverlayButton::SelectGameShopGood(id) => {
                if state.shop_open() {
                    let Some(game_shop) = game_shop.as_deref_mut() else {
                        continue;
                    };
                    game_shop.selected_game_shop_index = Some(id);
                    if let Some(page) = game_shop_page_for_index(game_shop, id) {
                        state.game_shop_page = page;
                    }
                }
            }
            OverlayButton::GameShopPaymentCredit => {
                if state.shop_open() {
                    let Some(game_shop) = game_shop.as_deref_mut() else {
                        continue;
                    };
                    game_shop.payment = GameShopPaymentType::Credit;
                }
            }
            OverlayButton::GameShopPaymentGold => {
                if state.shop_open() {
                    let Some(game_shop) = game_shop.as_deref_mut() else {
                        continue;
                    };
                    game_shop.payment = GameShopPaymentType::Gold;
                }
            }
            OverlayButton::GameShopQuantityInc => {
                if state.shop_open() {
                    let Some(game_shop) = game_shop.as_deref_mut() else {
                        continue;
                    };
                    game_shop.quantity_inc();
                }
            }
            OverlayButton::GameShopQuantityDec => {
                if state.shop_open() {
                    let Some(game_shop) = game_shop.as_deref_mut() else {
                        continue;
                    };
                    game_shop.quantity_dec();
                }
            }
            OverlayButton::GameShopPagePrev => {
                if state.shop_open() {
                    state.game_shop_page = state.game_shop_page.saturating_sub(1);
                }
            }
            OverlayButton::GameShopPageNext => {
                if state.shop_open() {
                    let page_count = game_shop
                        .as_deref()
                        .map(|model| game_shop_page_count(model.items.len()))
                        .unwrap_or(1);
                    state.game_shop_page =
                        (state.game_shop_page + 1).min(page_count.saturating_sub(1));
                }
            }
            OverlayButton::GameShopBuy => {
                if state.shop_open() {
                    let Some(game_shop) = game_shop.as_deref_mut() else {
                        continue;
                    };
                    let Some(entry) = game_shop.selected().cloned() else {
                        continue;
                    };
                    let quantity = game_shop.quantity;
                    let price_type = game_shop.payment.protocol_value();
                    if game_shop.buy_enabled(gold, credit, &player_class)
                        && entry.game_shop_index >= 0
                    {
                        let _ = intents.enqueue_game_shop_purchase(
                            &mut state.core,
                            game_shop,
                            &mut pending,
                            entry.game_shop_index,
                            quantity,
                            price_type,
                        );
                    }
                }
            }
            OverlayButton::SelectShopGood(id) => {
                if state.npc_shop_open() {
                    shop.selected_id = Some(id);
                }
            }
            OverlayButton::ShopBuy => {
                if state.npc_shop_open() && shop_buy_enabled(&shop, &inventory, state.shop_quantity)
                {
                    let id = shop.selected_id.unwrap();
                    intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::BuyItem {
                            item_index: id,
                            count: shop_quantity_clamped(state.shop_quantity),
                        },
                    );
                }
            }
            OverlayButton::ShopSell => {
                if !state.npc_shop_open() {
                    continue;
                }
                if let Some(slot) = shop.selected_bag_slot_for_sell {
                    if let Some(item) = inventory
                        .items
                        .iter()
                        .find(|i| i.container == 0 && i.slot == slot)
                    {
                        if let Some(uid) = item_unique_id(item) {
                            intents.push_pending_intent(
                                &mut pending,
                                NativePlayerUiIntent::SellItem {
                                    unique_id: uid,
                                    count: shop_quantity_clamped(state.shop_quantity),
                                },
                            );
                        }
                    }
                }
            }
            OverlayButton::ShopRepair => {
                if !state.npc_shop_open() {
                    continue;
                }
                if let Some(item) = selected_repair_item(&state, &inventory) {
                    if let Some(uid) = item_unique_id(item) {
                        if repair_selection_enabled(&state, &inventory) {
                            intents.push_pending_intent(
                                &mut pending,
                                NativePlayerUiIntent::RepairItem { unique_id: uid },
                            );
                        }
                    }
                }
            }
            OverlayButton::ShopSRepair => {
                if !state.npc_shop_open() {
                    continue;
                }
                if let Some(item) = selected_repair_item(&state, &inventory) {
                    if let Some(uid) = item_unique_id(item) {
                        if repair_selection_enabled(&state, &inventory) {
                            intents.push_pending_intent(
                                &mut pending,
                                NativePlayerUiIntent::SRepairItem { unique_id: uid },
                            );
                        }
                    }
                }
            }
            OverlayButton::ShopQuantityInc => {
                if state.npc_shop_open() {
                    state.shop_quantity_inc();
                }
            }
            OverlayButton::ShopQuantityDec => {
                if state.npc_shop_open() {
                    state.shop_quantity_dec();
                }
            }
            OverlayButton::ShopConfirm => {
                if !state.npc_shop_open() {
                    continue;
                }
                // Confirm is same as Buy when buy enabled, else Sell etc based on mode
                if state.shop_repair_mode {
                    if let Some(item) = selected_repair_item(&state, &inventory) {
                        if let Some(uid) = item_unique_id(item) {
                            if repair_selection_enabled(&state, &inventory) {
                                intents.push_pending_intent(
                                    &mut pending,
                                    NativePlayerUiIntent::RepairItem { unique_id: uid },
                                );
                            }
                        }
                    }
                } else if shop_buy_enabled(&shop, &inventory, state.shop_quantity) {
                    let id = shop.selected_id.unwrap();
                    intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::BuyItem {
                            item_index: id,
                            count: shop_quantity_clamped(state.shop_quantity),
                        },
                    );
                }
            }
            OverlayButton::ShopCancel => {
                if state.npc_shop_open() {
                    state.core.panel = mir2_ui_core::state::UiPanel::None;
                }
                state.shop_quantity = 1;
                shop.selected_id = None;
                shop.selected_bag_slot_for_sell = None;
                shop.selected_bag_slot_for_repair = None;
                state.shop_repair_container = 0;
                state.shop_repair_slot = None;
            }
            OverlayButton::SelectBagForSell(slot) => {
                if state.npc_shop_open() {
                    shop.selected_bag_slot_for_sell = Some(slot);
                }
            }
            OverlayButton::SelectBagForRepair(slot) => {
                if state.npc_shop_open() {
                    shop.selected_bag_slot_for_repair = Some(slot);
                    state.shop_repair_container = 0;
                    state.shop_repair_slot = Some(slot);
                }
            }
            OverlayButton::SelectEquipForRepair(slot) => {
                if state.npc_shop_open() {
                    state.shop_repair_container = 2;
                    state.shop_repair_slot = Some(slot);
                }
            }
            // Storage
            OverlayButton::SelectBagForStore(slot) => {
                storage.selected_bag_slot = Some(slot);
                storage.selected_storage_slot = None;
            }
            OverlayButton::SelectStorage(slot) => {
                storage.selected_storage_slot = Some(slot);
                storage.selected_bag_slot = None;
            }
            OverlayButton::StorageDeposit => {
                if storage_deposit_enabled(&storage, &inventory) {
                    let from = storage.selected_bag_slot.unwrap() as i32;
                    let Some(unique_id) = inventory
                        .items
                        .iter()
                        .find(|item| item.container == 0 && item.slot as i32 == from)
                        .and_then(item_unique_id)
                    else {
                        continue;
                    };
                    // find first free storage slot
                    let used: std::collections::HashSet<u32> = storage
                        .items
                        .iter()
                        .filter(|i| i.container == 4)
                        .map(|i| i.slot)
                        .collect();
                    let mut to = 0;
                    for s in 0..storage.size as u32 {
                        if !used.contains(&s) {
                            to = s as i32;
                            break;
                        }
                    }
                    intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::StoreItem {
                            unique_id,
                            from,
                            to,
                        },
                    );
                }
            }
            OverlayButton::StorageWithdraw => {
                if storage_withdraw_enabled(&storage, &inventory) {
                    let from = storage.selected_storage_slot.unwrap() as i32;
                    let Some(unique_id) = storage
                        .items
                        .iter()
                        .find(|item| item.container == 4 && item.slot as i32 == from)
                        .and_then(item_unique_id)
                    else {
                        continue;
                    };
                    let occupied: std::collections::HashSet<u32> = inventory
                        .items
                        .iter()
                        .filter(|i| i.container == 0)
                        .map(|i| i.slot)
                        .collect();
                    let mut to = 0;
                    for s in 0..BAG_SLOTS {
                        if !occupied.contains(&s) {
                            to = s as i32;
                            break;
                        }
                    }
                    intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::TakeBackItem {
                            unique_id,
                            from,
                            to,
                        },
                    );
                }
            }
            OverlayButton::StorageUnlock => {
                if storage_unlock_enabled(&storage) {
                    let pwd = storage.password_draft.clone();
                    intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::UnlockStorage { password: pwd },
                    );
                }
            }
            OverlayButton::StorageSetPassword => {
                if storage_set_password_enabled(&storage) {
                    let cur = storage.password_draft.clone();
                    let new = storage.new_password_draft.clone();
                    intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::SetStoragePassword {
                            current: cur,
                            new_password: new,
                        },
                    );
                }
            }
            OverlayButton::StorageRemovePassword => {
                if storage_remove_password_enabled(&storage) {
                    let cur = storage.password_draft.clone();
                    intents.push_pending_intent(
                        &mut pending,
                        NativePlayerUiIntent::RemoveStoragePassword { current: cur },
                    );
                }
            }
            OverlayButton::StorageExpand => {
                if storage_expand_enabled(&storage, inventory.gold) {
                    intents.push_pending_intent(&mut pending, NativePlayerUiIntent::ExpandStorage);
                }
            }
        }
    }
}

fn inspect_from_item(item: &ItemModel) -> ItemInspect {
    ItemInspect {
        container: item.container,
        slot: item.slot,
        key: item.key.clone(),
        name: item.name.clone(),
        quantity: item.quantity,
    }
}

fn inspected_inventory_item<'a>(
    state: &NativePlayerUiState,
    inventory: &'a InventoryModel,
) -> Option<&'a ItemModel> {
    let inspect = state.inspect.as_ref()?;
    (inspect.container == 0).then_some(())?;
    inventory
        .items
        .iter()
        .find(|item| item.container == 0 && item.slot == inspect.slot && item.key == inspect.key)
}

fn inspected_use_intent(
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
) -> Option<NativePlayerUiIntent> {
    let inspect = state.inspect.as_ref()?;
    let item = inventory.items.iter().find(|item| {
        item.container == inspect.container && item.slot == inspect.slot && item.key == inspect.key
    })?;
    Some(NativePlayerUiIntent::UseItem {
        key: Some(item.key.clone()),
        unique_id: item_unique_id(item),
        slot: u8::try_from(item.slot).ok(),
        grid: Some(container_name(item.container).to_owned()),
    })
}

fn inspected_equip_intent(
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
) -> Option<NativePlayerUiIntent> {
    let inspect = state.inspect.as_ref()?;
    if inspect.container == 2 {
        return inspected_remove_intent(state, inventory);
    }
    let item = inventory.items.iter().find(|item| {
        item.container == inspect.container && item.slot == inspect.slot && item.key == inspect.key
    })?;
    Some(NativePlayerUiIntent::EquipItem {
        unique_id: item_unique_id(item)?,
        grid: container_name(item.container).to_owned(),
        to: equip_destination_for_name(&item.name),
    })
}

fn inspected_remove_intent(
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
) -> Option<NativePlayerUiIntent> {
    let inspect = state.inspect.as_ref()?;
    let item = inventory
        .items
        .iter()
        .find(|item| item.container == 2 && item.slot == inspect.slot && item.key == inspect.key)?;
    Some(NativePlayerUiIntent::RemoveItem {
        unique_id: item_unique_id(item)?,
        grid: "equipment".to_owned(),
        to: -1,
    })
}

fn selected_repair_item<'a>(
    state: &NativePlayerUiState,
    inventory: &'a InventoryModel,
) -> Option<&'a ItemModel> {
    let slot = state.shop_repair_slot?;
    inventory
        .items
        .iter()
        .find(|item| item.container == state.shop_repair_container && item.slot == slot)
}

fn repair_selection_enabled(state: &NativePlayerUiState, inventory: &InventoryModel) -> bool {
    selected_repair_item(state, inventory)
        .is_some_and(|item| item.container == 0 || item.container == 2)
}

fn render_overlays(
    asset_server: Option<Res<AssetServer>>,
    shell: Option<Res<NativeShellModel>>,
    state: Res<NativePlayerUiState>,
    inventory: Res<InventoryModel>,
    inventory_feedback: Res<InventoryOperationFeedback>,
    mail: Res<MailModel>,
    map: Res<MapModel>,
    ui: Res<UiReadModel>,
    shop: Res<ShopModel>,
    game_shop: Res<GameShopModel>,
    storage: Res<StorageModel>,
    skills: Res<SkillModel>,
    social: Res<crate::social::SocialModel>,
    combat_target: Option<Res<crate::quest_model::CombatTargetModel>>,
    mut panels: ParamSet<(
        ParamSet<(
            Query<&mut Node, With<OverlayRoot>>,
            Query<(Entity, &mut Node), With<OverlayInventory>>,
            Query<(Entity, &mut Node), With<OverlayEquipment>>,
            Query<(Entity, &mut Node), With<OverlayMenu>>,
            Query<(Entity, &mut Node), With<OverlaySkill>>,
            Query<(Entity, &mut Node), With<OverlayInspect>>,
            Query<(Entity, &mut Node), With<OverlayDeath>>,
            Query<(Entity, &mut Node), With<OverlayChatDraft>>,
        )>,
        ParamSet<(
            Query<(Entity, &mut Node), With<OverlayMail>>,
            Query<(Entity, &mut Node), With<OverlayBigMap>>,
            Query<(Entity, &mut Node), With<OverlayShop>>,
            Query<(Entity, &mut Node), With<OverlayGameShop>>,
            Query<(Entity, &mut Node), With<OverlayStorage>>,
            Query<(Entity, &mut Node), With<OverlayOptions>>,
            Query<(Entity, &mut Node), With<OverlaySocial>>,
        )>,
    )>,
    mut commands: Commands,
) {
    let in_game = shell.is_some_and(|model| model.screen == NativeShellScreen::InGame);
    {
        let mut all = panels.p0();
        for mut node in all.p0().iter_mut() {
            node.display = if in_game {
                Display::Flex
            } else {
                Display::None
            };
        }
        if !in_game {
            return;
        }

        fill_panel(
            &mut commands,
            &mut all.p1(),
            state.inventory_open(),
            |parent| render_inventory(parent, &inventory, &state, &inventory_feedback),
        );
        fill_panel(
            &mut commands,
            &mut all.p2(),
            state.equipment_open(),
            |parent| render_equipment(parent, &inventory),
        );
        fill_panel(&mut commands, &mut all.p3(), state.menu_open(), render_menu);
        fill_panel(&mut commands, &mut all.p4(), state.skill_open(), |parent| {
            render_skills(parent, &skills)
        });
        fill_panel(
            &mut commands,
            &mut all.p5(),
            state.inspect.is_some(),
            |parent| render_inspect(parent, &state, &inventory),
        );
        let dead = ui.player.max_hp > 0 && ui.player.hp <= 0;
        fill_panel(&mut commands, &mut all.p6(), dead, render_death);
        fill_panel(
            &mut commands,
            &mut all.p7(),
            state.chat_focused(),
            |parent| render_chat_draft(parent, &state.chat_draft),
        );
    }
    {
        let mut secondary = panels.p1();
        fill_panel(
            &mut commands,
            &mut secondary.p0(),
            state.mail_open(),
            |parent| render_mail(parent, &mail, &inventory, &state),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p1(),
            state.bigmap_open(),
            |parent| render_bigmap(parent, asset_server.as_deref(), &map, &ui, &state),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p2(),
            state.npc_shop_open(),
            |parent| render_shop(parent, &shop, &inventory, &state),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p3(),
            state.shop_open(),
            |parent| render_game_shop(parent, &game_shop, &ui, &state),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p4(),
            state.storage_open(),
            |parent| render_storage(parent, &storage, &inventory, &state),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p5(),
            state.options_open(),
            |parent| render_options(parent, &state.core),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p6(),
            state.group_open() || state.guild_open() || state.trade_open(),
            |parent| {
                render_social(
                    parent,
                    &social,
                    &state,
                    &inventory,
                    combat_target.as_deref(),
                )
            },
        );
    }
}

fn fill_panel<C: Component>(
    commands: &mut Commands,
    query: &mut Query<(Entity, &mut Node), With<C>>,
    visible: bool,
    render: impl FnOnce(&mut ChildSpawnerCommands),
) {
    let Some((entity, mut node)) = query.iter_mut().next() else {
        return;
    };
    node.display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    commands.entity(entity).despawn_children();
    if visible {
        commands.entity(entity).with_children(render);
    }
}

fn render_inventory(
    parent: &mut ChildSpawnerCommands,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
    feedback: &InventoryOperationFeedback,
) {
    title(parent, "Bag");
    body(parent, &format!("{} Gold", inventory.gold));
    if let Some(draft) = &state.inventory_operation {
        let instruction = match draft {
            InventoryOperationDraft::Move { source_slot, .. } => {
                format!("Move source {source_slot}: select a different destination slot")
            }
            InventoryOperationDraft::Merge { source_slot, .. } => {
                format!("Merge source {source_slot}: select a different occupied slot")
            }
        };
        body(parent, &instruction);
    }
    if let Some(ack) = &feedback.last {
        body(
            parent,
            &format!(
                "{}: {} (inventory remains server-authoritative)",
                ack.label(),
                if ack.success() {
                    "accepted"
                } else {
                    "rejected"
                }
            ),
        );
    }
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_wrap: bevy::ui::FlexWrap::Wrap,
            column_gap: Val::Px(2.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|grid| {
            for slot in 0..BAG_SLOTS {
                let item = inventory
                    .items_in(0)
                    .into_iter()
                    .find(|item| item.slot == slot);
                let label = item
                    .map(|item| {
                        if item.quantity > 1 {
                            format!("{} x{}", short_name(&item.name, &item.key), item.quantity)
                        } else {
                            short_name(&item.name, &item.key)
                        }
                    })
                    .unwrap_or_default();
                let enabled = match &state.inventory_operation {
                    Some(InventoryOperationDraft::Move { source_slot, .. }) => *source_slot != slot,
                    Some(InventoryOperationDraft::Merge { source_slot, .. }) => {
                        *source_slot != slot && item.and_then(item_unique_id).is_some()
                    }
                    None => item.is_some(),
                };
                overlay_button(grid, &label, OverlayButton::InspectBag(slot), enabled);
            }
        });
    overlay_button(parent, "Close", OverlayButton::CloseWindows, true);
}

fn render_equipment(parent: &mut ChildSpawnerCommands, inventory: &InventoryModel) {
    title(parent, "Character / Equipment");
    for slot in 0..14 {
        let line = inventory
            .items_in(2)
            .into_iter()
            .find(|item| item.slot == slot)
            .map(|item| {
                format!(
                    "{}: {}",
                    equipment_slot_name(slot),
                    short_name(&item.name, &item.key)
                )
            })
            .unwrap_or_else(|| format!("{}: --", equipment_slot_name(slot)));
        overlay_button(parent, &line, OverlayButton::InspectEquip(slot), true);
    }
}

fn render_menu(parent: &mut ChildSpawnerCommands) {
    title(parent, "System");
    overlay_button(parent, "Bag (I)", OverlayButton::ToggleInventory, true);
    overlay_button(
        parent,
        "Character (C)",
        OverlayButton::ToggleEquipment,
        true,
    );
    overlay_button(parent, "Cash Shop (O)", OverlayButton::ToggleShop, true);
    overlay_button(parent, "NPC Shop", OverlayButton::ToggleNpcShop, true);
    overlay_button(parent, "Warehouse (P)", OverlayButton::ToggleStorage, true);
    overlay_button(parent, "Group", OverlayButton::ToggleGroup, true);
    overlay_button(parent, "Guild", OverlayButton::ToggleGuild, true);
    overlay_button(parent, "Trade", OverlayButton::ToggleTrade, true);
    overlay_button(parent, "Logout", OverlayButton::Logout, true);
    overlay_button(parent, "Close", OverlayButton::CloseWindows, true);
}

fn render_social(
    parent: &mut ChildSpawnerCommands,
    social: &crate::social::SocialModel,
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
    combat_target: Option<&crate::quest_model::CombatTargetModel>,
) {
    if state.group_open() {
        title(parent, "Group");
        let group = &social.group;
        body(
            parent,
            &format!(
                "Leader: {}  Members: {}/{}",
                group.leader_name.as_deref().unwrap_or("-"),
                group.members.len(),
                crate::social::MAX_GROUP_MEMBERS
            ),
        );
        if let Some(inviter) = group.pending_invite_from.as_deref() {
            body(parent, &format!("Incoming group invite from {inviter}"));
            overlay_button(parent, "Accept", OverlayButton::GroupInviteAccept, true);
            overlay_button(parent, "Decline", OverlayButton::GroupInviteDecline, true);
        }
        for (index, member) in group.members.iter().enumerate() {
            overlay_button(
                parent,
                &format!(
                    "{}{}  {}  Lv{}",
                    if member.leader { "[Leader] " } else { "" },
                    member.name,
                    if member.online { "Online" } else { "Offline" },
                    member.level.unwrap_or(0)
                ),
                OverlayButton::SelectGroupMember(index.min(255) as u8),
                true,
            );
        }
        overlay_button(
            parent,
            "Kick selected",
            OverlayButton::GroupRemoveSelected,
            state.selected_group_member.is_some(),
        );
        body(
            parent,
            &format!("Pending operations: {}", social.pending.len()),
        );
        if combat_target
            .and_then(|model| model.target.as_ref())
            .is_some_and(|target| target.is_player)
        {
            overlay_button(
                parent,
                "Invite current player",
                OverlayButton::GroupAddSelected,
                true,
            );
        }
        overlay_button(
            parent,
            "Leave group",
            OverlayButton::GroupLeave,
            group.active,
        );
    } else if state.guild_open() {
        title(parent, "Guild");
        let guild = &social.guild;
        body(
            parent,
            &format!(
                "{}  Lv{}  Gold {}",
                guild.name.as_deref().unwrap_or("No guild"),
                guild.level,
                guild.gold
            ),
        );
        body(
            parent,
            &format!(
                "Rank: {}  Members: {}/{}",
                guild.rank_name.as_deref().unwrap_or("-"),
                guild.members.len(),
                guild.max_members
            ),
        );
        if let Some(inviter) = guild.pending_invite_from.as_deref() {
            body(parent, &format!("Incoming guild invite from {inviter}"));
            overlay_button(parent, "Accept", OverlayButton::GuildInviteAccept, true);
            overlay_button(parent, "Decline", OverlayButton::GuildInviteDecline, true);
        }
        body(
            parent,
            &format!(
                "Notice: {}",
                if guild.notice.is_empty() {
                    "-".to_owned()
                } else {
                    guild.notice.join(" / ")
                }
            ),
        );
        for (index, member) in guild
            .members
            .iter()
            .take(crate::social::MAX_GUILD_MEMBERS)
            .enumerate()
        {
            overlay_button(
                parent,
                &format!(
                    "{}  {}",
                    member.name,
                    if member.online { "Online" } else { "Offline" }
                ),
                OverlayButton::SelectGuildMember(index.min(255) as u8),
                true,
            );
        }
        overlay_button(
            parent,
            "Kick selected",
            OverlayButton::GuildKickSelected,
            state.selected_guild_member.is_some()
                && guild
                    .permissions
                    .iter()
                    .any(|permission| permission.eq_ignore_ascii_case("kick")),
        );
        overlay_button(
            parent,
            "Publish current notice",
            OverlayButton::GuildPublishNotice,
            !guild.notice.is_empty(),
        );
        overlay_button(
            parent,
            "Refresh guild info",
            OverlayButton::GuildRequestInfo,
            true,
        );
    } else {
        title(parent, "Trade");
        let trade = &social.trade;
        body(
            parent,
            &format!(
                "State: {}  Partner: {}",
                if trade.state.is_empty() {
                    "idle"
                } else {
                    &trade.state
                },
                trade.partner.as_deref().unwrap_or("-")
            ),
        );
        body(
            parent,
            &format!(
                "Partner gold: {}  Items: {}  Confirmed: {}",
                trade.partner_gold,
                trade.partner_items.len(),
                trade.partner_confirmed
            ),
        );
        for item in &trade.partner_items {
            body(
                parent,
                &format!(
                    "  {} x{}",
                    item.name
                        .as_deref()
                        .or_else(|| item.item_index.map(|_| "Item #"))
                        .unwrap_or("Item"),
                    item.count
                ),
            );
        }
        if trade.state == "requested" {
            overlay_button(parent, "Accept trade", OverlayButton::TradeAccept, true);
            overlay_button(parent, "Decline trade", OverlayButton::TradeDecline, true);
        } else if trade.state == "open" {
            overlay_button(
                parent,
                "Offer 100 gold",
                OverlayButton::TradeGoldOffer,
                true,
            );
            for item in inventory
                .items
                .iter()
                .filter(|item| {
                    item.container == 0 && item.slot < 10 && item_unique_id(item).is_some()
                })
                .take(10)
            {
                overlay_button(
                    parent,
                    &format!(
                        "Offer {} x{}",
                        short_name(&item.name, &item.key),
                        item.quantity
                    ),
                    OverlayButton::TradeDepositItem(item.slot.min(9) as u8),
                    true,
                );
            }
            overlay_button(
                parent,
                "Confirm trade",
                OverlayButton::TradeConfirm,
                !trade.my_confirmed,
            );
            overlay_button(parent, "Cancel trade", OverlayButton::TradeCancel, true);
        } else {
            overlay_button(parent, "Request trade", OverlayButton::TradeRequest, true);
        }
    }
    body(parent, &format!("Pending: {}", social.pending.len()));
    overlay_button(parent, "Close", OverlayButton::CloseSocial, true);
}

fn render_skills(parent: &mut ChildSpawnerCommands, skills: &SkillModel) {
    title(parent, "Skills");
    if skills.skills.is_empty() {
        body(parent, "No skills learned yet.");
        body(
            parent,
            "Learn an active skill from the appropriate trainer.",
        );
        body(parent, "1-6  Use belt item");
    } else {
        for skill in &skills.skills {
            let key_label = skill
                .key
                .as_deref()
                .filter(|k| !k.is_empty())
                .unwrap_or("-");
            let line = format!(
                "{}  Lv{}  [{}]  CD:{}ms",
                skill.name, skill.level, key_label, skill.cooldown_ms
            );
            body(parent, &line);
        }
        if skills.skills.len() < 6 {
            body(parent, "1-6  Use belt item for remaining slots");
        }
    }
    overlay_button(parent, "Close", OverlayButton::CloseWindows, true);
}

fn render_inspect(
    parent: &mut ChildSpawnerCommands,
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
) {
    title(parent, "Item");
    if let Some(inspect) = state.inspect.as_ref() {
        body(
            parent,
            &format!(
                "{}  x{}  {} slot {}",
                if inspect.name.is_empty() {
                    inspect.key.as_str()
                } else {
                    inspect.name.as_str()
                },
                inspect.quantity,
                container_name(inspect.container),
                inspect.slot
            ),
        );
        overlay_button(parent, "Use (U)", OverlayButton::UseInspected, true);
        if inspect.container == 2 {
            overlay_button(parent, "Unequip (G)", OverlayButton::UnequipInspected, true);
        } else {
            overlay_button(parent, "Equip (G)", OverlayButton::EquipInspected, true);
        }
        if let Some(item) = inspected_inventory_item(state, inventory) {
            let valid_id = item_unique_id(item).is_some();
            let drop_enabled =
                valid_id && u16::try_from(item.quantity).is_ok() && item.quantity > 0;
            let split_max = item.quantity.saturating_sub(1).min(u32::from(u16::MAX)) as u16;
            parent
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    overlay_button(row, "Drop all", OverlayButton::DropInspected, drop_enabled);
                    overlay_button(
                        row,
                        "Split",
                        OverlayButton::SplitInspected,
                        valid_id && split_max > 0 && state.split_count <= split_max,
                    );
                });
            parent
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    overlay_button(
                        row,
                        "Split -",
                        OverlayButton::SplitCountDec,
                        split_max > 0 && state.split_count > 1,
                    );
                    body(row, &format!("{}", state.split_count));
                    overlay_button(
                        row,
                        "Split +",
                        OverlayButton::SplitCountInc,
                        split_max > 0 && state.split_count < split_max,
                    );
                });
            overlay_button(
                parent,
                "Move source",
                OverlayButton::ArmMoveInspected,
                valid_id,
            );
            overlay_button(
                parent,
                "Merge source",
                OverlayButton::ArmMergeInspected,
                valid_id && item.quantity > 0,
            );
            if state.inventory_operation.is_some() {
                overlay_button(
                    parent,
                    "Cancel move/merge",
                    OverlayButton::CancelInventoryOperation,
                    true,
                );
            }
        }
    }
}

fn render_death(parent: &mut ChildSpawnerCommands) {
    title(parent, "You are dead");
    body(parent, "Press V to revive in town");
}

fn render_chat_draft(parent: &mut ChildSpawnerCommands, draft: &str) {
    body(
        parent,
        &format!("Say: {}_", if draft.is_empty() { "" } else { draft }),
    );
}

fn mail_attachment_is_current(inventory: &InventoryModel, unique_id: u64) -> bool {
    unique_id != 0
        && inventory
            .items
            .iter()
            .any(|item| item.container == 0 && item.unique_id == Some(unique_id))
}

fn valid_mail_attachment_ids(inventory: &InventoryModel, ids: &[u64]) -> Option<Vec<u64>> {
    if ids.len() > MAX_MAIL_ATTACHMENTS
        || ids.iter().any(|id| *id == 0)
        || ids.windows(2).any(|window| window[0] == window[1])
    {
        return None;
    }
    let mut result = Vec::with_capacity(ids.len());
    for id in ids {
        if !mail_attachment_is_current(inventory, *id) || result.contains(id) {
            return None;
        }
        result.push(*id);
    }
    Some(result)
}

fn render_mail(
    parent: &mut ChildSpawnerCommands,
    mail: &MailModel,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
) {
    let visible = mail.visible_mails();
    let unread = visible.iter().filter(|mail| !mail.read).count();
    title(parent, &format!("Mail ({} / {})", unread, visible.len()));
    if let Some(compose) = state.core.mail_compose.as_ref() {
        render_mail_compose(parent, compose, inventory);
        overlay_button(parent, "Cancel", OverlayButton::CancelMailCompose, true);
        return;
    }
    if visible.is_empty() {
        body(parent, "No mail.");
        overlay_button(parent, "Write mail", OverlayButton::OpenMailCompose, true);
        overlay_button(parent, "Close", OverlayButton::CloseMail, true);
        return;
    }
    // List header
    body(
        parent,
        &format!("Unread: {}  Total: {}", unread, visible.len()),
    );
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|list| {
            for msg in visible {
                let is_selected = mail.selected_id == Some(msg.id);
                let claim_enabled = mail_claim_enabled(msg);
                let delete_enabled = mail_delete_enabled(msg);
                let read_label = if msg.read { "[Old]" } else { "[New]" };
                let claimed_label = if msg.claimed { " [Claimed]" } else { "" };
                let locked_label = if msg.locked { " [Locked]" } else { "" };
                let has_attach = if msg.has_attachment() {
                    " [Attachment]"
                } else {
                    ""
                };
                let line = format!(
                    "{} {}: {}{}{}{} {}",
                    read_label,
                    msg.sender,
                    short_name(&msg.subject, "Mail"),
                    claimed_label,
                    locked_label,
                    has_attach,
                    if msg.gold > 0 || !msg.items.is_empty() {
                        format!("({})", msg.attachment_summary())
                    } else {
                        String::new()
                    }
                );
                // Row container with select and action buttons
                list.spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(2.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    let label = if is_selected {
                        format!("▶ {}", line)
                    } else {
                        line
                    };
                    overlay_button(row, &label, OverlayButton::SelectMail(msg.id), true);
                    overlay_button(
                        row,
                        "Claim",
                        OverlayButton::ClaimMail(msg.id),
                        claim_enabled,
                    );
                    overlay_button(
                        row,
                        "Delete",
                        OverlayButton::DeleteMail(msg.id),
                        delete_enabled,
                    );
                });
            }
        });
    // Detail pane for selected
    if let Some(selected) = mail.selected() {
        body(parent, &format!("From: {}", selected.sender));
        body(parent, &format!("Subject: {}", selected.subject));
        if selected.body.trim().is_empty() {
            body(parent, "(No message body.)");
        } else {
            for line in selected.body.lines().take(4) {
                body(parent, line);
            }
        }
        if selected.has_attachment() {
            let status = if selected.claimed {
                "Claimed"
            } else if selected.locked {
                "Locked"
            } else {
                "Unclaimed"
            };
            body(
                parent,
                &format!("Attachment: {} [{}]", selected.attachment_summary(), status),
            );
        }
    } else {
        body(parent, "Select a message to read.");
    }
    overlay_button(parent, "Write mail", OverlayButton::OpenMailCompose, true);
    overlay_button(parent, "Close", OverlayButton::CloseMail, true);
}

fn render_mail_compose(
    parent: &mut ChildSpawnerCommands,
    draft: &mir2_ui_core::state::MailComposeDraft,
    inventory: &InventoryModel,
) {
    body(parent, "Write mail (Tab switches field; Esc cancels)");
    let message_label = if draft.message.is_empty() {
        "<type>".to_owned()
    } else {
        short_name(&draft.message, "<type>")
    };
    overlay_button(
        parent,
        &format!(
            "Recipient: {}",
            if draft.recipient.is_empty() {
                "<type>"
            } else {
                &draft.recipient
            }
        ),
        OverlayButton::MailRecipientFocus,
        true,
    );
    overlay_button(
        parent,
        &format!("Message: {message_label}"),
        OverlayButton::MailMessageFocus,
        true,
    );
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            body(row, &format!("Gold: {}", draft.gold));
            overlay_button(row, "-100", OverlayButton::MailGoldDec, draft.gold >= 100);
            overlay_button(row, "+100", OverlayButton::MailGoldInc, true);
        });
    body(
        parent,
        &format!("Attachments: {}/5", draft.attachment_unique_ids.len()),
    );
    for id in &draft.attachment_unique_ids {
        if let Some(item) = inventory
            .items
            .iter()
            .find(|item| item.container == 0 && item.unique_id == Some(*id))
        {
            parent
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    body(row, &format!("{} ×{}", item.name, item.quantity));
                    overlay_button(
                        row,
                        "Remove",
                        OverlayButton::RemoveMailAttachment(*id),
                        true,
                    );
                });
        }
    }
    for item in inventory.items_in(0) {
        let Some(id) = item.unique_id else { continue };
        if draft.attachment_unique_ids.contains(&id) {
            continue;
        }
        if draft.attachment_unique_ids.len() >= MAX_MAIL_ATTACHMENTS {
            break;
        }
        overlay_button(
            parent,
            &format!(
                "Attach {} ×{} (slot {})",
                item.name, item.quantity, item.slot
            ),
            OverlayButton::AddMailAttachment(id),
            true,
        );
    }
    overlay_button(
        parent,
        "Send",
        OverlayButton::SubmitMail,
        !draft.recipient.trim().is_empty() && !draft.message.trim().is_empty(),
    );
}

fn render_bigmap(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    map: &MapModel,
    ui: &UiReadModel,
    state: &NativePlayerUiState,
) {
    let map_name = ui.player.map_name.as_deref().unwrap_or("Unknown Map");
    title(parent, &format!("Big Map - {}", map_name));
    body(
        parent,
        &format!("Position: {}, {}", map.center_x, map.center_y),
    );
    body(parent, &format!("Zoom: {:.2}x", state.bigmap_zoom));
    if let (Some(asset), Some(asset_server)) = (bigmap_asset_path(Some(map_name)), asset_server) {
        // Render the same exported Crystal minimap image used by the native HUD.
        // The previous green placeholder only printed this path and therefore
        // could pass state tests while remaining visibly incomplete.
        parent
            .spawn(Node {
                width: Val::Px(BIGMAP_WIDTH),
                height: Val::Px(BIGMAP_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            })
            .with_children(|viewport| {
                viewport.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    ImageNode {
                        image: asset_server.load(asset),
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                ));
                let (px, py) = bigmap_player_position(map, state.bigmap_zoom);
                viewport.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(px - 4.0),
                        top: Val::Px(py - 4.0),
                        width: Val::Px(8.0),
                        height: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.2, 0.2)),
                ));
            });
    } else {
        body(parent, "No big map image for this region.");
        body(parent, &format!("Fallback world size: {}x{}", 700, 700));
        if !map.patches.is_empty() {
            body(
                parent,
                &format!("Terrain patches: {} (authoritative)", map.patches.len()),
            );
        }
    }
    // Zoom controls row
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            let can_zoom_in = state.bigmap_zoom < BIGMAP_ZOOM_MAX - 0.01;
            let can_zoom_out = state.bigmap_zoom > BIGMAP_ZOOM_MIN + 0.01;
            overlay_button(row, "Zoom In (+)", OverlayButton::BigMapZoomIn, can_zoom_in);
            overlay_button(
                row,
                "Zoom Out (-)",
                OverlayButton::BigMapZoomOut,
                can_zoom_out,
            );
        });
    overlay_button(parent, "Close", OverlayButton::CloseBigMap, true);
}

fn render_shop(
    parent: &mut ChildSpawnerCommands,
    shop: &ShopModel,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
) {
    title(parent, "Shop (NPC)");
    body(
        parent,
        &format!(
            "Gold: {}  Quantity: {}",
            inventory.gold, state.shop_quantity
        ),
    );
    if shop.goods.is_empty() {
        body(parent, "No goods. Talk to a shop NPC.");
    } else {
        parent
            .spawn(Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|list| {
                for good in &shop.goods {
                    let is_selected = shop.selected_id == Some(good.unique_id);
                    let label = format!(
                        "{}  Price:{}  Stock:{} {}{}",
                        short_name(&good.name, &good.unique_id.to_string()),
                        good.price,
                        good.stock_label(),
                        if is_selected { "◀" } else { "" },
                        if good.count > 1 {
                            format!(" x{}", good.count)
                        } else {
                            String::new()
                        }
                    );
                    overlay_button(
                        list,
                        &label,
                        OverlayButton::SelectShopGood(good.unique_id),
                        true,
                    );
                }
            });
    }
    // Quantity selector
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            let qty = shop_quantity_clamped(state.shop_quantity);
            let can_dec = qty > SHOP_QUANTITY_MIN;
            let can_inc = qty < SHOP_QUANTITY_MAX;
            overlay_button(row, "-", OverlayButton::ShopQuantityDec, can_dec);
            body(row, &format!("x{}", qty));
            overlay_button(row, "+", OverlayButton::ShopQuantityInc, can_inc);
        });
    // Bag items for sell/repair selection
    body(parent, "Select bag item independently for Sell or Repair:");
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_wrap: bevy::ui::FlexWrap::Wrap,
            column_gap: Val::Px(2.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|grid| {
            for slot in 0..8 {
                if let Some(item) = inventory.items_in(0).iter().find(|i| i.slot == slot) {
                    let label = format!("{} x{}", short_name(&item.name, &item.key), item.quantity);
                    let sell_label = if shop.selected_bag_slot_for_sell == Some(slot) {
                        format!("S▶{}", label)
                    } else {
                        format!("Sell {}", label)
                    };
                    let repair_label = if shop.selected_bag_slot_for_repair == Some(slot) {
                        format!("R▶{}", label)
                    } else {
                        format!("Repair {}", label)
                    };
                    overlay_button(
                        grid,
                        &sell_label,
                        OverlayButton::SelectBagForSell(slot),
                        true,
                    );
                    overlay_button(
                        grid,
                        &repair_label,
                        OverlayButton::SelectBagForRepair(slot),
                        true,
                    );
                } else {
                    overlay_button(grid, "--", OverlayButton::SelectBagForSell(slot), false);
                    overlay_button(grid, "--", OverlayButton::SelectBagForRepair(slot), false);
                }
            }
        });
    body(parent, "Equipment repair (container 2):");
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_wrap: bevy::ui::FlexWrap::Wrap,
            column_gap: Val::Px(2.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|grid| {
            for slot in 0..14 {
                let item = inventory
                    .items_in(2)
                    .into_iter()
                    .find(|item| item.slot == slot);
                let selected =
                    state.shop_repair_container == 2 && state.shop_repair_slot == Some(slot);
                let label = item
                    .map(|item| {
                        format!(
                            "{}{} {}",
                            if selected { "R▶" } else { "Repair " },
                            equipment_slot_name(slot),
                            short_name(&item.name, &item.key)
                        )
                    })
                    .unwrap_or_else(|| format!("{} --", equipment_slot_name(slot)));
                overlay_button(
                    grid,
                    &label,
                    OverlayButton::SelectEquipForRepair(slot),
                    item.is_some(),
                );
            }
        });
    // Action buttons with disabled states
    let buy_enabled = shop_buy_enabled(shop, inventory, state.shop_quantity);
    let sell_enabled = shop
        .selected_bag_slot_for_sell
        .map(|s| shop_sell_enabled(inventory, Some(s)))
        .unwrap_or(false);
    let repair_enabled = repair_selection_enabled(state, inventory);
    let s_repair_enabled = repair_enabled;

    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(row, "Buy", OverlayButton::ShopBuy, buy_enabled);
            overlay_button(row, "Sell", OverlayButton::ShopSell, sell_enabled);
            overlay_button(row, "Repair", OverlayButton::ShopRepair, repair_enabled);
            overlay_button(
                row,
                "S.Repair",
                OverlayButton::ShopSRepair,
                s_repair_enabled,
            );
        });
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            let confirm_enabled = if state.shop_repair_mode {
                repair_enabled
            } else {
                buy_enabled
            };
            overlay_button(row, "Confirm", OverlayButton::ShopConfirm, confirm_enabled);
            overlay_button(row, "Cancel", OverlayButton::ShopCancel, true);
        });
    overlay_button(parent, "Close", OverlayButton::CloseShop, true);
}

fn render_game_shop(
    parent: &mut ChildSpawnerCommands,
    game_shop: &GameShopModel,
    ui: &UiReadModel,
    state: &NativePlayerUiState,
) {
    title(parent, "Cash Shop");
    body(
        parent,
        &format!(
            "Gold: {}  Credit: {}  Quantity: {}",
            ui.player.gold, ui.player.credit, game_shop.quantity
        ),
    );
    body(
        parent,
        &format!(
            "Products: {}  Page {}/{}",
            game_shop.items.len(),
            state
                .game_shop_page
                .min(game_shop_page_count(game_shop.items.len()).saturating_sub(1))
                + 1,
            game_shop_page_count(game_shop.items.len())
        ),
    );
    if game_shop.pending_purchase.is_some() {
        body(
            parent,
            "Purchase pending; waiting for authoritative receipt.",
        );
    } else if game_shop.purchase_unknown {
        body(
            parent,
            "Purchase status unknown; refresh wallet, mail, and stock before retrying.",
        );
    }
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|list| {
            let class = ui.player.class_name.as_deref().unwrap_or("");
            let page = state
                .game_shop_page
                .min(game_shop_page_count(game_shop.items.len()).saturating_sub(1));
            for entry in game_shop_page_entries(game_shop, page) {
                let selected = game_shop.selected_game_shop_index == Some(entry.game_shop_index);
                let enabled = entry.visible_for_class(class);
                let label = format!(
                    "g#{} {}  G:{} C:{}  Stock:{}{}",
                    entry.game_shop_index,
                    short_name(&entry.item_name, &entry.item_index.to_string()),
                    entry.gold_price,
                    entry.credit_price,
                    entry.stock_label(),
                    if selected { " ◀" } else { "" },
                );
                overlay_button(
                    list,
                    &label,
                    OverlayButton::SelectGameShopGood(entry.game_shop_index),
                    enabled,
                );
            }
        });

    let page = state
        .game_shop_page
        .min(game_shop_page_count(game_shop.items.len()).saturating_sub(1));
    let page_count = game_shop_page_count(game_shop.items.len());
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(row, "Previous", OverlayButton::GameShopPagePrev, page > 0);
            body(row, &format!("Page {}/{}", page + 1, page_count));
            overlay_button(
                row,
                "Next",
                OverlayButton::GameShopPageNext,
                page + 1 < page_count,
            );
        });

    let selected = game_shop.selected();
    if let Some(entry) = selected {
        let payment = match game_shop.payment {
            GameShopPaymentType::Credit => "Credit",
            GameShopPaymentType::Gold => "Gold",
        };
        let price = entry
            .total_price(game_shop.payment, game_shop.quantity)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "disabled".to_owned());
        body(
            parent,
            &format!(
                "Selected: {}  Pay: {}  Total: {}  Stock: {}",
                entry.item_name,
                payment,
                price,
                entry.stock_label()
            ),
        );
    } else {
        body(parent, "Select a product");
    }

    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(
                row,
                "Credit",
                OverlayButton::GameShopPaymentCredit,
                game_shop.payment != GameShopPaymentType::Credit,
            );
            overlay_button(
                row,
                "Gold",
                OverlayButton::GameShopPaymentGold,
                game_shop.payment != GameShopPaymentType::Gold,
            );
            overlay_button(
                row,
                "-",
                OverlayButton::GameShopQuantityDec,
                game_shop.quantity > GAME_SHOP_QUANTITY_MIN,
            );
            body(row, &format!("x{}", game_shop.quantity));
            overlay_button(
                row,
                "+",
                OverlayButton::GameShopQuantityInc,
                game_shop.quantity < GAME_SHOP_QUANTITY_MAX,
            );
        });

    let class = ui.player.class_name.as_deref().unwrap_or("");
    let buy_enabled = game_shop.buy_enabled(ui.player.gold, ui.player.credit, class);
    overlay_button(parent, "Buy", OverlayButton::GameShopBuy, buy_enabled);
    if let Some(reason) = game_shop.buy_disabled_reason(ui.player.gold, ui.player.credit, class) {
        body(parent, &format!("Buy disabled: {reason}"));
    }
    overlay_button(parent, "Close", OverlayButton::CloseGameShop, true);
}

fn render_storage(
    parent: &mut ChildSpawnerCommands,
    storage: &StorageModel,
    inventory: &InventoryModel,
    _state: &NativePlayerUiState,
) {
    title(parent, "Warehouse (Storage)");
    let size = if storage.size == 0 {
        STORAGE_BASE_SIZE
    } else {
        storage.size
    };
    body(
        parent,
        &format!(
            "Bag Gold: {}  Storage: {}/{}  Expanded: {}",
            inventory.gold,
            storage.storage_occupied(),
            size,
            if storage.has_expanded { "Yes" } else { "No" }
        ),
    );
    if storage.has_expanded && storage.expiry != 0 {
        body(parent, &format!("Expanded expiry: {}", storage.expiry));
    }
    if storage.has_password {
        let status = if storage.unlocked {
            "Unlocked"
        } else {
            "Locked"
        };
        body(parent, &format!("Password: {} [{}]", "*".repeat(4), status));
        if !storage.unlocked {
            body(
                parent,
                &format!(
                    "Enter password: {}",
                    storage_password_display(&storage.password_draft)
                ),
            );
            let unlock_enabled = storage_unlock_enabled(storage);
            parent
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    overlay_button(row, "Unlock", OverlayButton::StorageUnlock, unlock_enabled);
                });
            body(parent, "Locked: deposit/withdraw disabled until unlocked.");
        }
    } else {
        body(parent, "No password set.");
    }
    // Bag section for deposit
    body(parent, "Bag (Deposit):");
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_wrap: bevy::ui::FlexWrap::Wrap,
            column_gap: Val::Px(2.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|grid| {
            for slot in 0..8 {
                let label_opt = inventory
                    .items_in(0)
                    .iter()
                    .find(|i| i.slot == slot)
                    .map(|item| {
                        let l = short_name(&item.name, &item.key);
                        if item.quantity > 1 {
                            format!("{} x{}", l, item.quantity)
                        } else {
                            l
                        }
                    });
                if let Some(label) = label_opt {
                    let is_selected = storage.selected_bag_slot == Some(slot);
                    let display = if is_selected {
                        format!("▶{}", label)
                    } else {
                        label
                    };
                    overlay_button(grid, &display, OverlayButton::SelectBagForStore(slot), true);
                } else {
                    overlay_button(grid, "--", OverlayButton::SelectBagForStore(slot), false);
                }
            }
        });
    // Storage grid
    body(
        parent,
        format!("Storage (Withdraw) {} slots:", size).as_str(),
    );
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_wrap: bevy::ui::FlexWrap::Wrap,
            column_gap: Val::Px(2.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|grid| {
            for slot in 0..size.min(16) as u32 {
                let label_opt = storage.item_in_storage(slot).map(|item| {
                    let l = short_name(&item.name, &item.key);
                    if item.quantity > 1 {
                        format!("{} x{}", l, item.quantity)
                    } else {
                        l
                    }
                });
                if let Some(label) = label_opt {
                    let is_selected = storage.selected_storage_slot == Some(slot);
                    let display = if is_selected {
                        format!("▶{}", label)
                    } else {
                        label
                    };
                    overlay_button(grid, &display, OverlayButton::SelectStorage(slot), true);
                } else {
                    overlay_button(grid, "--", OverlayButton::SelectStorage(slot), true);
                }
            }
            if size > 16 {
                body(grid, &format!("... +{} slots", size - 16));
            }
        });
    // Deposit / Withdraw buttons
    let deposit_enabled = storage_deposit_enabled(storage, inventory);
    let withdraw_enabled = storage_withdraw_enabled(storage, inventory);
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(
                row,
                "Deposit →",
                OverlayButton::StorageDeposit,
                deposit_enabled,
            );
            overlay_button(
                row,
                "← Withdraw",
                OverlayButton::StorageWithdraw,
                withdraw_enabled,
            );
        });
    // Password management
    body(parent, "Password Management:");
    body(
        parent,
        &format!(
            "New: {}  Confirm: {}",
            storage_password_display(&storage.new_password_draft),
            storage_password_display(&storage.confirm_password_draft)
        ),
    );
    let set_enabled = storage_set_password_enabled(storage);
    let remove_enabled = storage_remove_password_enabled(storage);
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(
                row,
                "Set Password",
                OverlayButton::StorageSetPassword,
                set_enabled,
            );
            overlay_button(
                row,
                "Remove Password",
                OverlayButton::StorageRemovePassword,
                remove_enabled,
            );
        });
    // Expansion
    let expand_enabled = storage_expand_enabled(storage, inventory.gold);
    body(
        parent,
        &format!("Expand Cost: {} Gold", STORAGE_EXPAND_COST),
    );
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(
                row,
                "Expand (10 days)",
                OverlayButton::StorageExpand,
                expand_enabled,
            );
        });
    overlay_button(parent, "Close", OverlayButton::CloseStorage, true);
}

fn render_options(parent: &mut ChildSpawnerCommands, core: &mir2_ui_core::state::UiState) {
    let options = core.options_draft.as_ref().unwrap_or(&core.options);
    title(parent, "Options");
    body(parent, "Changes are staged until Apply.");
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(
                row,
                if options.music_enabled {
                    "Music: On"
                } else {
                    "Music: Off"
                },
                OverlayButton::OptionsMusicToggle,
                true,
            );
            overlay_button(
                row,
                "Music -",
                OverlayButton::OptionsMusicVolumeDown,
                options.music_volume > 0,
            );
            body(row, &format!("{}%", options.music_volume));
            overlay_button(
                row,
                "Music +",
                OverlayButton::OptionsMusicVolumeUp,
                options.music_volume < 100,
            );
        });
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(
                row,
                if options.sound_enabled {
                    "Sound: On"
                } else {
                    "Sound: Off"
                },
                OverlayButton::OptionsSoundToggle,
                true,
            );
            overlay_button(
                row,
                "Sound -",
                OverlayButton::OptionsSoundVolumeDown,
                options.sound_volume > 0,
            );
            body(row, &format!("{}%", options.sound_volume));
            overlay_button(
                row,
                "Sound +",
                OverlayButton::OptionsSoundVolumeUp,
                options.sound_volume < 100,
            );
        });
    body(parent, &format!("Window mode: {:?}", options.window_mode));
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(
                row,
                "Windowed",
                OverlayButton::OptionsWindowed,
                options.window_mode != mir2_ui_core::state::UiWindowMode::Windowed,
            );
            overlay_button(
                row,
                "Fullscreen",
                OverlayButton::OptionsFullscreen,
                options.window_mode != mir2_ui_core::state::UiWindowMode::Fullscreen,
            );
        });
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            overlay_button(row, "Apply", OverlayButton::OptionsApply, true);
            overlay_button(row, "Cancel", OverlayButton::OptionsCancel, true);
            overlay_button(row, "Defaults", OverlayButton::OptionsDefaults, true);
        });
}

fn short_name(name: &str, key: &str) -> String {
    let source = if name.trim().is_empty() { key } else { name };
    let mut chars = source.chars();
    let taken: String = chars.by_ref().take(8).collect();
    if chars.next().is_some() {
        format!("{taken}…")
    } else {
        taken
    }
}

fn title(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_owned()),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(GOLD),
    ));
}

fn body(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_owned()),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(TEXT),
    ));
}

fn overlay_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: OverlayButton,
    enabled: bool,
) {
    let mut entity = parent.spawn((
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(18.0),
            padding: UiRect::all(Val::Px(3.0)),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(if enabled { BUTTON_BG } else { BUTTON_DISABLED }),
    ));
    if enabled {
        entity.insert((Button, action));
    }
    entity.with_children(|button| {
        button.spawn((
            Text::new(label.to_owned()),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(if enabled {
                TEXT
            } else {
                Color::srgba(0.8, 0.8, 0.8, 0.5)
            }),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::MailMessage;
    use crate::shop::ShopGood;

    #[test]
    fn options_adapter_dispatch_queues_typed_apply_effects() {
        let mut state = NativePlayerUiState::default();
        let mut effects = UiEffectQueue::default();
        dispatch_ui_action(
            &mut state.core,
            &mut effects,
            mir2_ui_core::action::UiAction::OpenOptions,
        );
        dispatch_ui_action(
            &mut state.core,
            &mut effects,
            mir2_ui_core::action::UiAction::SetMusicVolume { volume: 25 },
        );
        dispatch_ui_action(
            &mut state.core,
            &mut effects,
            mir2_ui_core::action::UiAction::ApplyOptions,
        );
        assert_eq!(state.core.options.music_volume, 25);
        assert_eq!(state.core.panel, mir2_ui_core::state::UiPanel::None);
        let effects = effects.drain();
        assert_eq!(effects.len(), 3);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            mir2_ui_core::effect::UiEffect::ApplyAudioSettings {
                music_volume: 25,
                ..
            }
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            mir2_ui_core::effect::UiEffect::ApplyWindowMode { .. }
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            mir2_ui_core::effect::UiEffect::PersistOptions { .. }
        )));
    }

    fn item(key: &str, name: &str, container: u8, slot: u32) -> ItemModel {
        ItemModel {
            unique_id: key.parse().ok(),
            key: key.to_owned(),
            name: name.to_owned(),
            quantity: 1,
            slot,
            container,
        }
    }

    fn mail_msg(id: u64, claimed: bool, locked: bool, gold: u32, items: Vec<&str>) -> MailMessage {
        MailMessage {
            id,
            sender: "System".to_owned(),
            subject: format!("Test {}", id),
            body: "Hello world".to_owned(),
            gold,
            items: items
                .into_iter()
                .map(|s| crate::mail::MailAttachment {
                    name: Some(s.to_owned()),
                    ..Default::default()
                })
                .collect(),
            operation: None,
            claimed,
            locked,
            read: false,
        }
    }

    fn shop_good(id: u64, name: &str, price: u32, stock: i32) -> ShopGood {
        ShopGood {
            unique_id: id,
            name: name.to_owned(),
            price,
            count: 1,
            stock,
            panel_type: 0,
        }
    }

    #[test]
    fn inspect_and_equip_mapping_cover_quest_rewards() {
        let pendant = item("77", "GoldenPendant", 0, 3);
        assert_eq!(item_unique_id(&pendant), Some(77));
        assert_eq!(equip_destination_for_name(&pendant.name), 4);
        assert_eq!(equip_destination_for_name("CopperRing"), 7);
        assert!(inspect_label(&pendant).contains("GoldenPendant"));
    }

    #[test]
    fn chat_focus_blocks_gameplay_keys_not_bag() {
        let mut state = NativePlayerUiState::default();
        state.core.panel = mir2_ui_core::state::UiPanel::Inventory;
        assert!(!state.blocks_gameplay_keys());
        assert!(state.blocks_world_click());
        state.core.chat_focused = true;
        assert!(state.blocks_gameplay_keys());
    }

    #[test]
    fn belt_use_intent_targets_belt_grid() {
        match belt_use_intent(3) {
            NativePlayerUiIntent::UseItem {
                slot,
                grid,
                unique_id,
                key,
            } => {
                assert_eq!(slot, Some(3));
                assert_eq!(grid.as_deref(), Some("belt"));
                assert_eq!(unique_id, None);
                assert_eq!(key, None);
            }
            other => panic!("{other:?}"),
        }
    }

    fn read_player_ui(_state: Res<NativePlayerUiState>) {}

    #[test]
    fn overlay_mutate_set_runs_before_res_readers() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MapModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .configure_sets(
                Update,
                NativePlayerUiSet::Mutate.before(NativePlayerUiSet::Read),
            )
            .add_systems(
                Update,
                process_overlay_keyboard.in_set(NativePlayerUiSet::Mutate),
            )
            .add_systems(Update, read_player_ui.in_set(NativePlayerUiSet::Read));
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        app.insert_resource(shell);
        app.update();
        app.update();
    }

    #[test]
    fn non_ingame_keyboard_defers_session_clear_to_revision_pipeline() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MapModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, process_overlay_keyboard);
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::ConnectionLost;
        app.insert_resource(shell);
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::Inventory;
            state.core.panel = mir2_ui_core::state::UiPanel::Character;
            state.core.panel = mir2_ui_core::state::UiPanel::Menu;
            state.core.panel = mir2_ui_core::state::UiPanel::Skill;
            state.core.panel = mir2_ui_core::state::UiPanel::Mail;
            state.core.panel = mir2_ui_core::state::UiPanel::BigMap;
            state.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
            state.core.panel = mir2_ui_core::state::UiPanel::Storage;
            state.core.chat_focused = true;
            state.chat_draft = "hello".to_owned();
            state.core.minimap_visible = false;
            state.bigmap_zoom = 2.0;
        }
        {
            let mut mail = app.world_mut().resource_mut::<MailModel>();
            mail.mails.push(mail_msg(1, false, false, 100, vec![]));
            mail.selected_id = Some(1);
        }
        {
            let mut shop = app.world_mut().resource_mut::<ShopModel>();
            shop.goods.push(shop_good(1, "Potion", 100, 10));
            shop.selected_id = Some(1);
        }
        {
            let mut storage = app.world_mut().resource_mut::<StorageModel>();
            storage.items.push(item("10", "Sword", 4, 0));
            storage.password_draft = "secret".to_owned();
            storage.has_password = true;
            storage.unlocked = false;
        }
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(state.core.panel, mir2_ui_core::state::UiPanel::Storage);
        assert!(state.core.chat_focused);
        assert_eq!(state.chat_draft, "hello");
        assert!(!state.core.minimap_visible);
        assert_eq!(state.bigmap_zoom, 2.0);
        let mail = app.world().resource::<MailModel>();
        assert_eq!(mail.mails.len(), 1);
        assert_eq!(mail.selected_id, Some(1));
        let shop = app.world().resource::<ShopModel>();
        assert_eq!(shop.goods.len(), 1);
        assert_eq!(shop.selected_id, Some(1));
        let storage = app.world().resource::<StorageModel>();
        assert_eq!(storage.items.len(), 1);
        assert_eq!(storage.password_draft, "secret");
    }

    #[test]
    fn overlay_keyboard_toggles_inventory_with_i() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MapModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, process_overlay_keyboard);
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        app.insert_resource(shell);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyI);
        }
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_open());
    }

    #[test]
    fn mail_open_blocks_world_click_and_uses_reducer() {
        let mut state = NativePlayerUiState::default();
        assert!(!state.blocks_world_click());
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        assert!(state.blocks_world_click());
        if state.mail_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.panel = mir2_ui_core::state::UiPanel::BigMap;
        assert!(state.blocks_world_click());
        // Minimap visible should not block
        if state.bigmap_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.minimap_visible = false;
        assert!(!state.blocks_world_click());
        state.core.minimap_visible = true;
        assert!(!state.blocks_world_click());
        // Verify mail toggle via reducer mirrors inventory logic without opening inventory
        let core_state = mir2_ui_core::state::UiState {
            screen: mir2_ui_core::state::UiScreen::InGame,
            panel: mir2_ui_core::state::UiPanel::None,
            ..Default::default()
        };
        let t =
            mir2_ui_core::reducer::reduce(&core_state, mir2_ui_core::action::UiAction::OpenMail);
        assert_eq!(t.state.panel, mir2_ui_core::state::UiPanel::Mail);
        assert_ne!(t.state.panel, mir2_ui_core::state::UiPanel::Inventory);
        let t2 =
            mir2_ui_core::reducer::reduce(&core_state, mir2_ui_core::action::UiAction::OpenBigMap);
        assert_eq!(t2.state.panel, mir2_ui_core::state::UiPanel::BigMap);
        let core_minimap = mir2_ui_core::state::UiState {
            screen: mir2_ui_core::state::UiScreen::InGame,
            minimap_visible: true,
            ..Default::default()
        };
        let t3 = mir2_ui_core::reducer::reduce(
            &core_minimap,
            mir2_ui_core::action::UiAction::ToggleMinimap,
        );
        assert!(!t3.state.minimap_visible);
    }

    #[test]
    fn mail_claim_and_delete_disabled_states() {
        let unclaimed = mail_msg(1, false, false, 100, vec!["Gold"]);
        let claimed = mail_msg(2, true, false, 100, vec!["Gold"]);
        let locked = mail_msg(3, false, true, 100, vec!["Gold"]);
        let no_attach = mail_msg(4, false, false, 0, vec![]);
        assert!(mail_claim_enabled(&unclaimed));
        assert!(!mail_claim_enabled(&claimed));
        assert!(!mail_claim_enabled(&locked));
        assert!(!mail_claim_enabled(&no_attach));
        assert!(mail_delete_enabled(&unclaimed));
        assert!(mail_delete_enabled(&claimed));
        assert!(!mail_delete_enabled(&locked));
        // Selection and unread count
        let mut model = MailModel {
            mails: vec![unclaimed.clone(), claimed.clone(), locked.clone()],
            selected_id: Some(1),
        };
        assert_eq!(model.unread_count(), 3);
        assert_eq!(model.selected().unwrap().id, 1);
        model.mails[0].read = true;
        assert_eq!(model.unread_count(), 2);
    }

    #[test]
    fn bigmap_zoom_and_asset_and_position() {
        assert_eq!(bigmap_zoom_clamped(0.1), BIGMAP_ZOOM_MIN);
        assert_eq!(bigmap_zoom_clamped(10.0), BIGMAP_ZOOM_MAX);
        assert_eq!(bigmap_zoom_in(1.0), 1.25);
        assert_eq!(bigmap_zoom_out(1.0), 0.75);
        assert_eq!(bigmap_zoom_in(BIGMAP_ZOOM_MAX), BIGMAP_ZOOM_MAX);
        assert_eq!(bigmap_zoom_out(BIGMAP_ZOOM_MIN), BIGMAP_ZOOM_MIN);
        // Asset path for Bichon
        assert_eq!(
            bigmap_asset_path(Some("BichonProvince")),
            Some("original-ui/MMap/101.png".to_owned())
        );
        assert_eq!(bigmap_asset_path(Some("UnknownMap")), None);
        // Player position scales with zoom
        let map = MapModel {
            patches: vec![],
            center_x: 100,
            center_y: 200,
        };
        let (x1, _y1) = bigmap_player_position(&map, 1.0);
        let (x2, _y2) = bigmap_player_position(&map, 2.0);
        assert!(x2 > x1, "zoom should increase offset from center");
        // Close windows retains minimap and zoom
        let mut state = NativePlayerUiState {
            core: mir2_ui_core::state::UiState {
                panel: mir2_ui_core::state::UiPanel::Mail,
                minimap_visible: false,
                screen: mir2_ui_core::state::UiScreen::InGame,
                chat_focused: false,
                ..Default::default()
            },
            bigmap_zoom: 2.0,
            ..Default::default()
        };
        state.close_windows();
        assert!(!state.mail_open());
        assert!(!state.bigmap_open());
        assert!(!state.shop_open());
        assert!(!state.storage_open());
        assert!(!state.minimap_visible());
        assert_eq!(state.bigmap_zoom, 2.0);
    }

    #[test]
    fn minimap_toggle_retains_state_and_mail_bigmap_esc_behavior() {
        let mut state = NativePlayerUiState::default();
        assert!(state.minimap_visible());
        state.core.minimap_visible = false;
        state.core.panel = mir2_ui_core::state::UiPanel::Mail;
        state.core.panel = mir2_ui_core::state::UiPanel::BigMap;
        // Simulate ESC handling via close_windows
        state.close_windows();
        assert!(!state.mail_open());
        assert!(!state.bigmap_open());
        // minimap_visible retained
        assert!(!state.minimap_visible());
        // Zoom retained
        let mut state2 = NativePlayerUiState::default();
        state2.bigmap_zoom = 2.5;
        state2.core.panel = mir2_ui_core::state::UiPanel::BigMap;
        state2.close_windows();
        assert_eq!(state2.bigmap_zoom, 2.5);
    }

    #[test]
    fn overlay_buttons_mail_flow_generates_intents() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MapModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>();
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        app.insert_resource(shell);
        // Seed mail with unclaimed attachment
        {
            let mut mail = app.world_mut().resource_mut::<MailModel>();
            mail.mails
                .push(mail_msg(10, false, false, 500, vec!["Potion"]));
            mail.mails.push(mail_msg(11, true, false, 0, vec![]));
            mail.mails.push(mail_msg(12, false, true, 100, vec![]));
        }
        app.add_systems(Update, process_overlay_buttons);
        // Helper to press button
        fn press(app: &mut App, button: OverlayButton) {
            let entity = app
                .world_mut()
                .spawn((Interaction::Pressed, button, Button))
                .id();
            app.update();
            app.world_mut().despawn(entity);
        }
        // Selection is local, but read state stays authoritative until a
        // refreshed MailModel arrives.
        press(&mut app, OverlayButton::SelectMail(10));
        {
            let mail = app.world().resource::<MailModel>();
            assert_eq!(mail.selected_id, Some(10));
            assert!(!mail.mails.iter().find(|m| m.id == 10).unwrap().read);
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents
                .iter()
                .any(|i| matches!(i, NativePlayerUiIntent::ReadMail { mail_id: 10 })));
        }
        // Clear intents
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        press(&mut app, OverlayButton::SelectMail(10));
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
        assert!(
            !app.world()
                .resource::<MailModel>()
                .mails
                .iter()
                .find(|mail| mail.id == 10)
                .unwrap()
                .read
        );
        // Claim mail 10 should be enabled and push intent
        press(&mut app, OverlayButton::ClaimMail(10));
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents
                .iter()
                .any(|i| matches!(i, NativePlayerUiIntent::ClaimMail { mail_id: 10 })));
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        // Claim mail 11 (already claimed) should not push
        press(&mut app, OverlayButton::ClaimMail(11));
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents.is_empty());
        }
        // Delete locked mail 12 should not push (disabled)
        press(&mut app, OverlayButton::DeleteMail(12));
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents.is_empty());
        }
        // Delete mail 10 should push
        press(&mut app, OverlayButton::DeleteMail(10));
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents
                .iter()
                .any(|i| matches!(i, NativePlayerUiIntent::DeleteMail { mail_id: 10 })));
        }
    }

    #[test]
    fn overlay_bigmap_zoom_buttons_clamp() {
        let mut state = NativePlayerUiState::default();
        assert_eq!(state.bigmap_zoom, 1.0);
        for _ in 0..10 {
            state.zoom_in();
        }
        assert_eq!(state.bigmap_zoom, BIGMAP_ZOOM_MAX);
        for _ in 0..20 {
            state.zoom_out();
        }
        assert_eq!(state.bigmap_zoom, BIGMAP_ZOOM_MIN);
    }

    #[test]
    fn hud_mail_bigmap_minimap_actions_are_exhaustively_handled() {
        // Ensure every CrystalHudAction is handled and Mail/BigMap go through reducer
        let actions = vec![
            CrystalHudAction::Mail,
            CrystalHudAction::BigMap,
            CrystalHudAction::MinimapToggle,
            CrystalHudAction::Inventory,
            CrystalHudAction::Quest,
            CrystalHudAction::Option,
        ];
        for action in actions {
            let state = NativePlayerUiState::default();
            let core_state_mail = mir2_ui_core::state::UiState {
                screen: mir2_ui_core::state::UiScreen::InGame,
                panel: mir2_ui_core::state::UiPanel::None,
                ..Default::default()
            };
            match action {
                CrystalHudAction::Mail => {
                    let t = mir2_ui_core::reducer::reduce(
                        &core_state_mail,
                        mir2_ui_core::action::UiAction::OpenMail,
                    );
                    assert_eq!(t.state.panel, mir2_ui_core::state::UiPanel::Mail);
                }
                CrystalHudAction::BigMap => {
                    let t = mir2_ui_core::reducer::reduce(
                        &core_state_mail,
                        mir2_ui_core::action::UiAction::OpenBigMap,
                    );
                    assert_eq!(t.state.panel, mir2_ui_core::state::UiPanel::BigMap);
                }
                CrystalHudAction::MinimapToggle => {
                    let s = mir2_ui_core::state::UiState {
                        screen: mir2_ui_core::state::UiScreen::InGame,
                        minimap_visible: true,
                        ..Default::default()
                    };
                    let t = mir2_ui_core::reducer::reduce(
                        &s,
                        mir2_ui_core::action::UiAction::ToggleMinimap,
                    );
                    assert!(!t.state.minimap_visible);
                    let _ = state.minimap_visible(); // ensure field exists
                }
                _ => {}
            }
        }
    }

    #[test]
    fn shop_quantity_and_buy_sell_disabled_states() {
        let mut shop = ShopModel::default();
        shop.goods.push(shop_good(1, "Potion", 100, 10));
        shop.goods.push(shop_good(2, "Sword", 400, 1));
        shop.selected_id = Some(1);
        let mut inventory = InventoryModel {
            gold: 50,
            items: vec![],
        };
        // Not enough gold
        assert!(!shop_buy_enabled(&shop, &inventory, 1));
        inventory.gold = 500;
        assert!(shop_buy_enabled(&shop, &inventory, 1));
        // Quantity 2 costs 200, still enabled
        assert!(shop_buy_enabled(&shop, &inventory, 2));
        // Quantity clamping
        assert_eq!(shop_quantity_clamped(0), 1);
        assert_eq!(shop_quantity_clamped(100), 99);
        assert_eq!(shop_quantity_inc(99), 99);
        assert_eq!(shop_quantity_dec(1), 1);
        assert_eq!(shop_quantity_inc(1), 2);
        // Out of stock
        shop.selected_id = Some(2);
        assert!(!shop_buy_enabled(&shop, &inventory, 2)); // stock 1, qty2 fails
        assert!(shop_buy_enabled(&shop, &inventory, 1));
        // Full bag blocks buy
        let mut full_inventory = InventoryModel {
            gold: 10000,
            items: vec![],
        };
        for slot in 0..BAG_SLOTS {
            full_inventory
                .items
                .push(item(&slot.to_string(), "Item", 0, slot));
        }
        shop.selected_id = Some(1);
        assert!(!shop_buy_enabled(&shop, &full_inventory, 1));
        // Sell enabled only when bag slot occupied
        assert!(shop_sell_enabled(&full_inventory, Some(0)));
        assert!(!shop_sell_enabled(&full_inventory, Some(99)));
        assert!(!shop_sell_enabled(&full_inventory, None));
        // Repair
        assert!(crate::shop::shop_repair_enabled(&full_inventory, Some(0)));
        assert!(!crate::shop::shop_repair_enabled(&full_inventory, Some(99)));
    }

    #[test]
    fn shop_blocks_world_click_and_uses_reducer() {
        let mut state = NativePlayerUiState::default();
        assert!(!state.blocks_world_click());
        state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        assert!(state.blocks_world_click());
        if state.shop_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
        assert!(state.blocks_world_click());
        if state.storage_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        assert!(!state.blocks_world_click());
        // GameShop reducer not opening inventory
        let core_state = mir2_ui_core::state::UiState {
            screen: mir2_ui_core::state::UiScreen::InGame,
            panel: mir2_ui_core::state::UiPanel::None,
            ..Default::default()
        };
        let t = mir2_ui_core::reducer::reduce(
            &core_state,
            mir2_ui_core::action::UiAction::OpenGameShop,
        );
        assert_eq!(t.state.panel, mir2_ui_core::state::UiPanel::GameShop);
        assert_ne!(t.state.panel, mir2_ui_core::state::UiPanel::Inventory);
        // toggle off
        let core_shop = mir2_ui_core::state::UiState {
            screen: mir2_ui_core::state::UiScreen::InGame,
            panel: mir2_ui_core::state::UiPanel::GameShop,
            ..Default::default()
        };
        let t2 =
            mir2_ui_core::reducer::reduce(&core_shop, mir2_ui_core::action::UiAction::OpenGameShop);
        assert_eq!(t2.state.panel, mir2_ui_core::state::UiPanel::None);
    }

    #[test]
    fn storage_deposit_withdraw_and_password_disabled_states() {
        let mut storage = StorageModel::default();
        storage.size = STORAGE_BASE_SIZE;
        storage.has_password = true;
        storage.unlocked = false;
        storage.password_draft = "ab".to_owned();
        assert!(!storage_unlock_enabled(&storage));
        storage.password_draft = "abcd".to_owned();
        assert!(storage_unlock_enabled(&storage));
        storage.unlocked = true;
        storage.new_password_draft = "newpass".to_owned();
        storage.confirm_password_draft = "newpass".to_owned();
        storage.password_draft = "oldpass".to_owned();
        assert!(storage_set_password_enabled(&storage));
        storage.confirm_password_draft = "mismatch".to_owned();
        assert!(!storage_set_password_enabled(&storage));
        storage.confirm_password_draft = "newpass".to_owned();
        assert!(storage_remove_password_enabled(&storage));
        storage.has_password = false;
        assert!(!storage_remove_password_enabled(&storage));

        // deposit / withdraw
        let inventory = InventoryModel {
            gold: 500000,
            items: vec![item("1", "Potion", 0, 0)],
        };
        let mut storage2 = StorageModel {
            size: 10,
            has_password: false,
            unlocked: true,
            ..Default::default()
        };
        storage2.selected_bag_slot = Some(0);
        assert!(storage_deposit_enabled(&storage2, &inventory));
        storage2.selected_bag_slot = None;
        assert!(!storage_deposit_enabled(&storage2, &inventory));
        // locked blocks
        storage2.has_password = true;
        storage2.unlocked = false;
        storage2.selected_bag_slot = Some(0);
        assert!(!storage_deposit_enabled(&storage2, &inventory));
        storage2.unlocked = true;
        assert!(storage_deposit_enabled(&storage2, &inventory));
        // withdraw
        storage2.items.push(item("2", "StoredSword", 4, 0));
        storage2.selected_storage_slot = Some(0);
        storage2.selected_bag_slot = None;
        assert!(storage_withdraw_enabled(&storage2, &inventory));
        // bag full blocks withdraw
        let mut full_inv = InventoryModel {
            gold: 0,
            items: vec![],
        };
        for slot in 0..BAG_SLOTS {
            full_inv.items.push(item(&slot.to_string(), "X", 0, slot));
        }
        assert!(!storage_withdraw_enabled(&storage2, &full_inv));
        // expand
        let cheap_storage = StorageModel {
            has_expanded: false,
            ..Default::default()
        };
        assert!(!storage_expand_enabled(&cheap_storage, 500));
        assert!(storage_expand_enabled(&cheap_storage, 2_000_000));
        let expanded = StorageModel {
            has_expanded: true,
            ..Default::default()
        };
        assert!(!storage_expand_enabled(&expanded, 2_000_000));
    }

    #[test]
    fn storage_blocks_world_click_and_close_retains_minimap() {
        let mut state = NativePlayerUiState {
            core: mir2_ui_core::state::UiState {
                panel: mir2_ui_core::state::UiPanel::Storage,
                minimap_visible: false,
                screen: mir2_ui_core::state::UiScreen::InGame,
                chat_focused: false,
                ..Default::default()
            },
            bigmap_zoom: 2.0,
            ..Default::default()
        };
        assert!(state.blocks_world_click());
        state.close_windows();
        assert!(!state.storage_open());
        assert!(!state.minimap_visible());
        assert_eq!(state.bigmap_zoom, 2.0);
    }

    #[test]
    fn inventory_buttons_use_explicit_instance_ids_and_suppress_duplicate_presses() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        });
        app.world_mut().resource_mut::<InventoryModel>().items = vec![
            ItemModel {
                unique_id: Some(42),
                key: "small-hp-drug".into(),
                name: "Small HP Drug".into(),
                quantity: 4,
                slot: 0,
                container: 0,
            },
            ItemModel {
                unique_id: Some(43),
                key: "small-hp-drug".into(),
                name: "Small HP Drug".into(),
                quantity: 2,
                slot: 1,
                container: 0,
            },
            ItemModel {
                unique_id: None,
                key: "legacy-template".into(),
                name: "Legacy".into(),
                quantity: 2,
                slot: 3,
                container: 0,
            },
        ];
        app.add_systems(Update, process_overlay_buttons);
        fn press(app: &mut App, button: OverlayButton) {
            let entity = app
                .world_mut()
                .spawn((Interaction::Pressed, button, Button))
                .id();
            app.update();
            app.world_mut().despawn(entity);
        }

        press(&mut app, OverlayButton::InspectBag(0));
        press(&mut app, OverlayButton::DropInspected);
        press(&mut app, OverlayButton::DropInspected);
        press(&mut app, OverlayButton::SplitInspected);
        press(&mut app, OverlayButton::SplitInspected);
        press(&mut app, OverlayButton::ArmMoveInspected);
        press(&mut app, OverlayButton::InspectBag(2));
        press(&mut app, OverlayButton::InspectBag(0));
        press(&mut app, OverlayButton::ArmMergeInspected);
        press(&mut app, OverlayButton::InspectBag(1));

        let intents = app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        assert_eq!(intents.len(), 4);
        assert!(matches!(
            &intents[0],
            NativePlayerUiIntent::DropItem {
                key,
                unique_id: 42,
                count: 4,
                hero_inventory: false,
            } if key == "small-hp-drug"
        ));
        assert!(matches!(
            &intents[1],
            NativePlayerUiIntent::SplitItem {
                unique_id: 42,
                grid,
                count: 1,
            } if grid == "inventory"
        ));
        assert!(matches!(
            &intents[2],
            NativePlayerUiIntent::MoveItem {
                unique_id: 42,
                from: 0,
                to: 2,
                ..
            }
        ));
        assert!(matches!(
            &intents[3],
            NativePlayerUiIntent::MergeItem {
                id_from: 42,
                id_to: 43,
                ..
            }
        ));

        press(&mut app, OverlayButton::InspectBag(3));
        press(&mut app, OverlayButton::DropInspected);
        press(&mut app, OverlayButton::SplitInspected);
        press(&mut app, OverlayButton::ArmMoveInspected);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_operation
            .is_none());
    }

    #[test]
    fn overlay_buttons_shop_storage_flow_generates_intents() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<MapModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>();
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        app.insert_resource(shell);
        {
            let mut inv = app.world_mut().resource_mut::<InventoryModel>();
            inv.gold = 5000;
            inv.items.push(item("1", "Potion", 0, 0));
            inv.items.push(item("2", "Sword", 0, 1));
        }
        {
            let mut shop = app.world_mut().resource_mut::<ShopModel>();
            shop.goods.push(shop_good(100, "Potion", 100, 10));
            shop.goods.push(shop_good(101, "Sword", 500, 5));
        }
        {
            let mut storage = app.world_mut().resource_mut::<StorageModel>();
            storage.size = 10;
            storage.has_password = false;
            storage.unlocked = true;
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_npc_shop();
        app.add_systems(Update, process_overlay_buttons);
        fn press(app: &mut App, button: OverlayButton) {
            let entity = app
                .world_mut()
                .spawn((Interaction::Pressed, button, Button))
                .id();
            app.update();
            app.world_mut().despawn(entity);
        }
        // Select shop good 100
        press(&mut app, OverlayButton::SelectShopGood(100));
        assert_eq!(app.world().resource::<ShopModel>().selected_id, Some(100));
        // Buy should be enabled and push intent
        press(&mut app, OverlayButton::ShopBuy);
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents.iter().any(|i| matches!(
                i,
                NativePlayerUiIntent::BuyItem {
                    item_index: 100,
                    count: 1
                }
            )));
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        // Select bag for sell
        press(&mut app, OverlayButton::SelectBagForSell(0));
        assert_eq!(
            app.world()
                .resource::<ShopModel>()
                .selected_bag_slot_for_sell,
            Some(0)
        );
        assert_eq!(
            app.world().resource::<StorageModel>().selected_bag_slot,
            None
        );
        assert_eq!(
            app.world()
                .resource::<ShopModel>()
                .selected_bag_slot_for_repair,
            None
        );
        press(&mut app, OverlayButton::ShopSell);
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents
                .iter()
                .any(|i| matches!(i, NativePlayerUiIntent::SellItem { .. })));
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        // Repair does nothing without its own selection, then uses only that selection.
        press(&mut app, OverlayButton::ShopRepair);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        press(&mut app, OverlayButton::SelectBagForRepair(1));
        press(&mut app, OverlayButton::ShopRepair);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .iter()
            .any(|intent| matches!(intent, NativePlayerUiIntent::RepairItem { unique_id: 2 })));
        assert_eq!(
            app.world()
                .resource::<ShopModel>()
                .selected_bag_slot_for_sell,
            Some(0)
        );
        assert_eq!(
            app.world()
                .resource::<ShopModel>()
                .selected_bag_slot_for_repair,
            Some(1)
        );
        assert_eq!(
            app.world().resource::<StorageModel>().selected_bag_slot,
            None
        );
        // Shop quantity inc/dec
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
            state.shop_quantity = 1;
        }
        press(&mut app, OverlayButton::ShopQuantityInc);
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().shop_quantity,
            2
        );
        press(&mut app, OverlayButton::ShopQuantityDec);
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().shop_quantity,
            1
        );
        // Storage deposit
        press(&mut app, OverlayButton::SelectBagForStore(0));
        press(&mut app, OverlayButton::StorageDeposit);
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents
                .iter()
                .any(|i| matches!(i, NativePlayerUiIntent::StoreItem { .. })));
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        // Storage withdraw: need item in storage
        {
            let mut storage = app.world_mut().resource_mut::<StorageModel>();
            storage.items.push(item("5", "StoredItem", 4, 0));
            storage.selected_storage_slot = Some(0);
            storage.selected_bag_slot = None;
        }
        press(&mut app, OverlayButton::StorageWithdraw);
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents
                .iter()
                .any(|i| matches!(i, NativePlayerUiIntent::TakeBackItem { .. })));
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        // Expand storage
        {
            let mut inv = app.world_mut().resource_mut::<InventoryModel>();
            inv.gold = 2_000_000;
        }
        press(&mut app, OverlayButton::StorageExpand);
        {
            let intents = app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .clone();
            assert!(intents
                .iter()
                .any(|i| matches!(i, NativePlayerUiIntent::ExpandStorage)));
        }
    }

    #[test]
    fn cash_shop_selection_pages_beyond_first_24_rows_is_receipt_deduped() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<ShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<GameShopModel>()
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            });
        let mut ui = UiReadModel::default();
        ui.player.gold = 100;
        app.insert_resource(ui);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_shop();
        {
            let mut game_shop = app.world_mut().resource_mut::<GameShopModel>();
            for index in 0..105 {
                game_shop.upsert(crate::game_shop::GameShopEntry {
                    item_index: 1000 + index,
                    game_shop_index: index,
                    item_name: format!("Cash {index}"),
                    gold_price: 10,
                    can_buy_gold: true,
                    ..Default::default()
                });
            }
        }
        app.add_systems(Update, process_overlay_buttons);

        fn press(app: &mut App, button: OverlayButton) {
            let entity = app
                .world_mut()
                .spawn((Interaction::Pressed, button, Button))
                .id();
            app.update();
            app.world_mut().despawn(entity);
        }

        press(&mut app, OverlayButton::SelectGameShopGood(25));
        assert_eq!(
            app.world()
                .resource::<GameShopModel>()
                .selected_game_shop_index,
            Some(25)
        );
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().game_shop_page,
            1,
            "selecting gIndex 25 must move to the second bounded page"
        );

        press(&mut app, OverlayButton::GameShopBuy);
        assert_eq!(app.world().resource::<PendingOperations>().len(), 1);
        assert!(app
            .world()
            .resource::<GameShopModel>()
            .pending_purchase
            .is_some());
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .iter()
            .any(|intent| matches!(
                intent,
                NativePlayerUiIntent::GameShopBuy { g_index: 25, .. }
            )));
        press(&mut app, OverlayButton::GameShopBuy);
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .iter()
                .filter(|intent| matches!(
                    intent,
                    NativePlayerUiIntent::GameShopBuy { g_index: 25, .. }
                ))
                .count(),
            1,
            "same-frame duplicate cash purchase must be blocked without inventing an ACK"
        );
    }

    #[test]
    fn full_social_intent_queue_rejects_without_pending_or_eviction() {
        let mut queue = NativePlayerUiIntentQueue::default();
        let mut social = crate::social::SocialModel::default();
        for index in 0..MAX_QUEUED {
            queue.push_intent(NativePlayerUiIntent::Chat {
                message: format!("queued-{index}"),
            });
        }

        assert!(!queue.push_social_pending(
            &mut social,
            NativePlayerUiIntent::TradeDepositItem { from: 2, to: 0 },
        ));
        assert_eq!(queue.intents.len(), MAX_QUEUED);
        assert!(
            matches!(queue.intents.front(), Some(NativePlayerUiIntent::Chat { message }) if message == "queued-0")
        );
        assert!(
            matches!(queue.intents.back(), Some(NativePlayerUiIntent::Chat { message }) if message == "queued-23")
        );
        assert!(social.pending.is_empty());
    }

    #[test]
    fn full_intent_queue_rejects_game_shop_without_any_pending_state() {
        let mut queue = NativePlayerUiIntentQueue::default();
        for index in 0..MAX_QUEUED {
            assert!(queue.push_intent(NativePlayerUiIntent::Chat {
                message: format!("queued-{index}"),
            }));
        }
        let mut core = mir2_ui_core::state::UiState::default();
        let mut game_shop = GameShopModel::default();
        let mut pending = PendingOperations::default();

        assert!(queue
            .enqueue_game_shop_purchase(&mut core, &mut game_shop, &mut pending, 31, 2, 1)
            .is_none());
        assert!(core.game_shop_pending.is_none());
        assert!(game_shop.pending_purchase.is_none());
        assert!(pending.is_empty());
        assert_eq!(queue.intents.len(), MAX_QUEUED);
    }

    #[test]
    fn queued_game_shop_is_non_evicting_and_drains_exactly_once_after_flood() {
        let mut queue = NativePlayerUiIntentQueue::default();
        for index in 0..MAX_QUEUED - 1 {
            assert!(queue.push_intent(NativePlayerUiIntent::Chat {
                message: format!("before-{index}"),
            }));
        }
        let mut core = mir2_ui_core::state::UiState::default();
        let mut game_shop = GameShopModel::default();
        let mut pending = PendingOperations::default();
        let request = queue
            .enqueue_game_shop_purchase(&mut core, &mut game_shop, &mut pending, 31, 2, 1)
            .expect("transaction fits in final slot");

        for index in 0..(MAX_QUEUED * 4) {
            assert!(queue.push_intent(NativePlayerUiIntent::Chat {
                message: format!("after-{index}"),
            }));
        }
        assert_eq!(queue.intents.len(), MAX_QUEUED);
        assert_eq!(
            queue
                .intents
                .iter()
                .filter(|intent| matches!(
                    intent,
                    NativePlayerUiIntent::GameShopBuy { request_id, .. }
                        if request_id == &request.request_id
                ))
                .count(),
            1
        );

        let drained = queue.drain_intents();
        assert_eq!(
            drained
                .iter()
                .filter(|intent| matches!(
                    intent,
                    NativePlayerUiIntent::GameShopBuy { request_id, .. }
                        if request_id == &request.request_id
                ))
                .count(),
            1
        );
        assert!(queue.drain_intents().is_empty());
        assert!(pending.contains(&PendingOperationKey::GameShop(request.request_id)));
    }

    #[test]
    fn enter_focuses_escape_cancels_and_empty_not_sent() {
        // Enter focuses chat
        let mut state = NativePlayerUiState::default();
        assert!(!state.chat_focused());
        // Simulate process_overlay_keyboard Enter when not focused -> focuses
        state.core.chat_focused = false;
        // focus via Enter
        state.core.chat_focused = true;
        assert!(state.chat_focused());
        // Draft non-empty trimmed sends on second Enter (blur)
        state.chat_draft = "  hello world  ".to_owned();
        let msg = state.trimmed_chat_to_send().unwrap();
        assert_eq!(msg, "hello world");
        state.chat_draft.clear();
        state.core.chat_focused = false;
        assert!(!state.chat_focused());
        // Escape cancels draft
        state.core.chat_focused = true;
        state.chat_draft = "some draft".to_owned();
        // simulate Escape
        state.core.chat_focused = false;
        state.chat_draft.clear();
        assert!(!state.chat_focused());
        assert!(state.chat_draft.is_empty());
        // Empty not sent
        state.chat_draft = "   ".to_owned();
        assert!(state.trimmed_chat_to_send().is_none());
        state.chat_draft = "".to_owned();
        assert!(state.trimmed_chat_to_send().is_none());
    }

    #[test]
    fn send_after_blur_requires_refocus() {
        let mut state = NativePlayerUiState::default();
        state.core.chat_focused = true;
        state.chat_draft = "test message".to_owned();
        let msg = state.trimmed_chat_to_send().expect("should send");
        // Simulate Enter handling: clear and blur
        state.chat_draft.clear();
        state.core.chat_focused = false;
        assert_eq!(msg, "test message");
        assert!(!state.chat_focused());
        // Next Enter should focus, not send
        state.core.chat_focused = true;
        assert!(state.chat_focused());
        assert!(state.chat_draft.is_empty());
    }

    #[test]
    fn any_window_blocks_world_click_and_action() {
        let mut state = NativePlayerUiState::default();
        assert!(!state.blocks_world_click());
        state.core.panel = mir2_ui_core::state::UiPanel::Inventory;
        assert!(state.blocks_world_click());
        if state.inventory_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.panel = mir2_ui_core::state::UiPanel::Skill;
        assert!(state.blocks_world_click(), "skill window must block");
        if state.skill_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        assert!(state.blocks_world_click());
        if state.quest_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
        assert!(state.blocks_world_click());
        if state.shop_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.panel = mir2_ui_core::state::UiPanel::Storage;
        assert!(state.blocks_world_click());
        if state.storage_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.chat_focused = true;
        assert!(state.blocks_world_click());
        state.core.chat_focused = false;
        state.inspect = Some(ItemInspect {
            container: 0,
            slot: 0,
            key: "k".into(),
            name: "n".into(),
            quantity: 1,
        });
        assert!(state.blocks_world_click());
        state.inspect = None;
        assert!(!state.blocks_world_click());
        // With dialog or dead also blocks via blocks_world_action
        assert!(state.blocks_world_action(true, false));
        assert!(state.blocks_world_action(false, true));
        assert!(!state.blocks_world_action(false, false));
    }

    #[test]
    fn dragging_window_scrollbar_or_button_captures_pointer_and_blocks_movement() {
        let mut state = NativePlayerUiState::default();
        // No window, but dragging window captures
        assert!(state.captures_pointer(true, false, false));
        assert!(state.captures_pointer(false, true, false));
        assert!(state.captures_pointer(false, false, true));
        // Even without drag, open window blocks
        state.core.panel = mir2_ui_core::state::UiPanel::Inventory;
        assert!(state.captures_pointer(false, false, false));
        if state.inventory_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        assert!(!state.captures_pointer(false, false, false));
        // Chat focused also blocks
        state.core.chat_focused = true;
        assert!(state.captures_pointer(false, false, false));
    }

    #[test]
    fn z_order_and_modal_priority_correct() {
        assert!(is_overlay_z_order_correct());
        assert_eq!(z_for_modal(OverlayModalPriority::Hud), OVERLAY_HUD_Z);
        assert_eq!(
            z_for_modal(OverlayModalPriority::SystemMenu),
            OVERLAY_MENU_Z
        );
        // Priority ordering: Menu > Death > Chat > NpcDialog > Hud
        assert!(OverlayModalPriority::SystemMenu > OverlayModalPriority::Death);
        assert!(OverlayModalPriority::Death > OverlayModalPriority::Chat);
        assert!(OverlayModalPriority::Chat > OverlayModalPriority::NpcDialog);
        // Test modal_priority_for_state
        let mut state = NativePlayerUiState::default();
        assert_eq!(modal_priority_for_state(&state, false, false), None);
        state.core.panel = mir2_ui_core::state::UiPanel::Menu;
        assert_eq!(
            modal_priority_for_state(&state, false, false),
            Some(OverlayModalPriority::SystemMenu)
        );
        if state.menu_open() {
            state.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        state.core.chat_focused = true;
        assert_eq!(
            modal_priority_for_state(&state, false, true),
            Some(OverlayModalPriority::Death),
            "death overrides chat"
        );
        state.core.chat_focused = true;
        // dead true should prioritize death over chat
        state.core.chat_focused = true;
        // even with chat focused, dead wins
        // quest/dialog
        state.core.chat_focused = false;
        assert_eq!(
            modal_priority_for_state(&state, true, false),
            Some(OverlayModalPriority::NpcDialog)
        );
        state.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        assert_eq!(
            modal_priority_for_state(&state, false, false),
            Some(OverlayModalPriority::NpcDialog)
        );
    }
}

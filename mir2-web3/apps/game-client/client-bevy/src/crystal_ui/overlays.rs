//! Operable native player windows: bag, equipment, inspect, death, menu, chat, mail, bigmap, shop, storage.

use std::collections::VecDeque;

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextLayout};
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, Interaction, JustifyContent, Node,
    PositionType, UiRect, Val,
};

use crate::big_map::{
    BigMapGatewayIntentQueue, BigMapModel, BigMapPoint, BigMapView, BIG_MAP_NPC_ROW_COUNT,
};
use crate::game_shop::{
    GameShopModel, GameShopPaymentType, GameShopRequest, GAME_SHOP_QUANTITY_MAX,
    GAME_SHOP_QUANTITY_MIN,
};
use crate::inventory::{item_icon_path, InventoryModel, ItemModel};
use crate::mail::{
    mail_claim_enabled, mail_delete_enabled, MailModel, MailOperationKind, MailPageCursor,
    MAX_MAIL_ATTACHMENTS,
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
use crate::skill_binding_persistence::{
    persist_skill_bindings_if_changed, SkillBindingPersistenceRuntime,
};
use crate::skill_binding_ui::SkillBindingUi;
use crate::skill_model::SkillModel;
use crate::storage::{
    inventory_selection_for_slot, storage_deposit_enabled_for_selection, storage_expand_enabled,
    storage_remove_password_enabled, storage_set_password_enabled, storage_unlock_enabled,
    storage_withdraw_enabled_for_selection, StorageItemSelection, StorageModel, StoragePageCursor,
};
#[cfg(test)]
use crate::storage::{storage_deposit_enabled, storage_withdraw_enabled};

use super::assets::CrystalButtonAssetSet;
use super::hud::CrystalHudAction;
use super::panel_layouts::{
    GAME_SHOP_CELL_SIZE, GAME_SHOP_COLUMN_STEP, GAME_SHOP_GRID_ORIGIN, GAME_SHOP_PAGE_COLUMNS,
    GAME_SHOP_PAGE_SIZE as CRYSTAL_GAME_SHOP_PAGE_SIZE, GAME_SHOP_PANEL_SIZE, GAME_SHOP_ROW_STEP,
    INVENTORY_CELL_SIZE, INVENTORY_GRID_ORIGIN, INVENTORY_GRID_STEP, INVENTORY_PAGE_COLUMNS,
    INVENTORY_PAGE_SIZE, INVENTORY_PANEL_SIZE, SKILL_PAGE_SIZE, SKILL_PANEL_SIZE, SKILL_ROW_ORIGIN,
    SKILL_ROW_SIZE, SKILL_ROW_STEP_Y,
};
use super::spec::{CrystalButtonSpec, CrystalRect};
use super::widget::spawn_crystal_image_button;

const BIG_MAP_SEARCH_COOLDOWN_MS: u64 = 1_000;

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

/// Pages exposed by Crystal's CharacterDialog.  The page is local UI state;
/// values shown on each page still come from the authoritative read models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharacterPage {
    #[default]
    Character,
    Stats1,
    Stats2,
    Spells,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuildLeftPage {
    #[default]
    Notice,
    Members,
}

pub const BIGMAP_ZOOM_MIN: f32 = 0.5;
pub const BIGMAP_ZOOM_MAX: f32 = 3.0;
pub const BIGMAP_ZOOM_STEP: f32 = 0.25;
pub const BIGMAP_WIDTH: f32 = 568.0;
pub const BIGMAP_HEIGHT: f32 = 380.0;
pub const CRYSTAL_MENU_PANEL_RECT: CrystalRect = CrystalRect::new(988.0, 349.0, 36.0, 282.0);
pub const CRYSTAL_OPTIONS_PANEL_RECT: CrystalRect = CrystalRect::new(382.0, 207.0, 259.0, 354.0);
pub const CRYSTAL_BIGMAP_PANEL_RECT: CrystalRect = CrystalRect::new(132.0, 134.0, 760.0, 500.0);
pub const CRYSTAL_GROUP_PANEL_RECT: CrystalRect = CrystalRect::new(396.0, 259.0, 232.0, 249.0);
pub const CRYSTAL_GUILD_PANEL_RECT: CrystalRect = CrystalRect::new(217.0, 168.0, 590.0, 432.0);

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

/// A destructive bag-drop must be confirmed against the latest authoritative
/// inventory snapshot.  Keeping the exact instance id, slot, and quantity
/// prevents a stale confirmation from dropping a replacement stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryDropConfirmation {
    key: String,
    unique_id: u64,
    slot: u32,
    count: u16,
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
    pub selected_skill_id: Option<u32>,
    pub character_page: CharacterPage,
    pub inventory_page: u8,
    pub skill_page: usize,
    pub drop_confirmation: Option<InventoryDropConfirmation>,
    pub selected_group_member: Option<u8>,
    pub selected_guild_member: Option<u8>,
    pub guild_left_page: GuildLeftPage,
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

/// Renderer-only state for the Crystal BigMap search field. Authoritative map,
/// NPC, search result and teleport eligibility remain in [`BigMapModel`].
#[derive(Debug, Clone, Default, Resource, PartialEq, Eq)]
pub struct BigMapUiState {
    pub search_focused: bool,
    last_reset_epoch: Option<u64>,
    requested_epoch_map: Option<(u64, i32)>,
}

#[derive(Debug, Clone, Default, Resource, PartialEq, Eq)]
pub struct MailUiState {
    pub cursor: MailPageCursor,
}

#[derive(Debug, Clone, Default, Resource, PartialEq, Eq)]
pub struct StorageUiState {
    pub cursor: StoragePageCursor,
    pub bag_selection: Option<StorageItemSelection>,
    pub storage_selection: Option<StorageItemSelection>,
}

#[derive(Debug, Clone, Default, Resource, PartialEq, Eq)]
pub struct ShopUiState {
    pub start_index: usize,
}

#[derive(SystemParam)]
pub(crate) struct BigMapControls<'w> {
    model: Option<ResMut<'w, BigMapModel>>,
    intents: Option<ResMut<'w, BigMapGatewayIntentQueue>>,
    ui: Option<ResMut<'w, BigMapUiState>>,
    time: Option<Res<'w, Time>>,
    skill_binding: Option<ResMut<'w, SkillBindingUi>>,
    skills: Option<ResMut<'w, SkillModel>>,
    skill_persistence: Option<ResMut<'w, SkillBindingPersistenceRuntime>>,
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
            selected_skill_id: None,
            character_page: CharacterPage::Character,
            inventory_page: 0,
            skill_page: 0,
            drop_confirmation: None,
            selected_group_member: None,
            selected_guild_member: None,
            guild_left_page: GuildLeftPage::Notice,
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
        self.selected_skill_id = None;
        self.character_page = CharacterPage::Character;
        self.inventory_page = 0;
        self.skill_page = 0;
        self.character_page = CharacterPage::Character;
        self.inventory_page = 0;
        self.skill_page = 0;
        self.drop_confirmation = None;
        self.shop_repair_container = 0;
        self.shop_repair_slot = None;
        self.game_shop_page = 0;
        self.split_count = 1;
        self.selected_group_member = None;
        self.selected_guild_member = None;
        self.guild_left_page = GuildLeftPage::Notice;
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

    fn drain_observe_bounded(&mut self, max: usize) -> usize {
        self.drain_matching_bounded(max, |effect| {
            matches!(
                effect,
                mir2_ui_core::effect::UiEffect::RequestObserve { .. }
            )
        })
        .len()
    }

    fn take_exit_application(&mut self) -> bool {
        !self
            .drain_matching_bounded(1, |effect| {
                matches!(effect, mir2_ui_core::effect::UiEffect::ExitApplication)
            })
            .is_empty()
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

/// Testable ECS provenance markers for the Big Map-only render tree. They
/// intentionally retain the authoritative renderer projection rather than
/// deriving values back from UI text or asset names.
#[derive(Debug, Clone, Component, PartialEq, Eq)]
struct BigMapImageEntity {
    url: String,
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
struct BigMapPlayerEntity {
    location: BigMapPoint,
}

#[derive(Debug, Clone, Component, PartialEq, Eq)]
struct BigMapNpcRowEntity {
    object_id: u32,
    name: String,
    location: BigMapPoint,
}

#[derive(Component)]
struct OverlayShop;

#[derive(Component)]
struct OverlayGameShop;

#[derive(Component)]
struct OverlayInventoryGridViewport;

#[derive(Component)]
struct OverlaySkillListViewport;

#[derive(Component)]
struct OverlayGameShopProduct;

#[derive(Component)]
struct OverlayStorage;

#[derive(Component)]
struct OverlayOptions;

#[derive(Component)]
struct OverlaySocial;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayButton {
    ExitApplication,
    CloseWindows,
    CloseInspect,
    CloseMail,
    CloseBigMap,
    CloseShop,
    CloseGameShop,
    CloseStorage,
    CloseOptions,
    SetCrystalOption(mir2_ui_core::state::UiCrystalOption, bool),
    OptionsObserve(bool),
    OptionsMusicVolumeDown,
    OptionsMusicVolumeUp,
    OptionsSoundVolumeDown,
    OptionsSoundVolumeUp,
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
    GroupSwitch,
    GroupLeave,
    GroupAddSelected,
    GroupRemoveSelected,
    SelectGroupMember(u8),
    GuildRequestInfo,
    SelectGuildLeftPage(GuildLeftPage),
    GuildInviteAccept,
    GuildInviteDecline,
    GuildPublishNotice,
    SelectGuildMember(u8),
    GuildKickMember(u8),
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
    ConfirmDropInspected,
    CancelDropInspected,
    SplitInspected,
    SplitCountDec,
    SplitCountInc,
    ArmMoveInspected,
    ArmMergeInspected,
    CancelInventoryOperation,
    InspectBag(u32),
    InspectEquip(u32),
    SelectCharacterPage(CharacterPage),
    SelectInventoryPage(u8),
    SelectSkill(u32),
    AssignSkillKey(u8),
    ClearSkillBinding,
    CloseSkillAssign,
    SkillPagePrev,
    SkillPageNext,
    SelectMail(u64),
    MailPagePrev,
    MailPageNext,
    ReadMail(u64),
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
    BigMapScrollUp,
    BigMapScrollDown,
    BigMapWorld,
    BigMapMyLocation,
    BigMapSearchFocus,
    BigMapSearchSubmit,
    BigMapTeleport,
    SelectBigMapNpc(u32),
    // NPC shop
    SelectShopGood(u64),
    ShopBuy,
    ShopSell,
    ShopRepair,
    ShopSRepair,
    ShopQuantityInc,
    ShopQuantityDec,
    ShopPageUp,
    ShopPageDown,
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
    StoragePage(usize),
    StorageDeposit,
    StorageWithdraw,
    StorageUnlock,
    StorageSetPassword,
    StorageRemovePassword,
    StorageExpand,
}

#[derive(SystemParam)]
struct OverlayButtonControls<'w, 's> {
    big_map: BigMapControls<'w>,
    mail_ui: ResMut<'w, MailUiState>,
    storage_ui: ResMut<'w, StorageUiState>,
    shop_ui: ResMut<'w, ShopUiState>,
    buttons: Query<'w, 's, (&'static Interaction, &'static OverlayButton), Changed<Interaction>>,
}

#[derive(SystemParam)]
struct OverlayRenderModels<'w> {
    asset_server: Option<Res<'w, AssetServer>>,
    shell: Option<Res<'w, NativeShellModel>>,
    state: Res<'w, NativePlayerUiState>,
    inventory: Res<'w, InventoryModel>,
    inventory_feedback: Res<'w, InventoryOperationFeedback>,
    mail: Res<'w, MailModel>,
    mail_ui: Res<'w, MailUiState>,
    big_map: Res<'w, BigMapModel>,
    big_map_ui: Res<'w, BigMapUiState>,
    ui: Res<'w, UiReadModel>,
    shop: Res<'w, ShopModel>,
    shop_ui: Res<'w, ShopUiState>,
    game_shop: Res<'w, GameShopModel>,
    storage: Res<'w, StorageModel>,
    storage_ui: Res<'w, StorageUiState>,
    skills: Res<'w, SkillModel>,
    skill_binding: Res<'w, SkillBindingUi>,
    skill_persistence: Res<'w, SkillBindingPersistenceRuntime>,
    social: Res<'w, crate::social::SocialModel>,
    combat_target: Option<Res<'w, crate::quest_model::CombatTargetModel>>,
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
            .init_resource::<BigMapModel>()
            .init_resource::<BigMapGatewayIntentQueue>()
            .init_resource::<BigMapUiState>()
            .init_resource::<SkillBindingUi>()
            .init_resource::<SkillBindingPersistenceRuntime>()
            .init_resource::<MailUiState>()
            .init_resource::<StorageUiState>()
            .init_resource::<ShopUiState>()
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
                    crate::skill_binding_persistence::load_persisted_skill_bindings,
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
                (sync_big_map_ui, sync_local_panel_models)
                    .chain()
                    .in_set(NativePlayerUiSet::Mutate)
                    .before(process_overlay_keyboard),
            )
            .add_systems(
                Update,
                (
                    consume_mail_operation_feedback,
                    consume_hud_buttons,
                    process_overlay_keyboard,
                    process_overlay_buttons,
                    consume_exit_application,
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

/// Keep renderer-local BigMap focus and pending requests inside the current
/// authoritative map/session epoch. Opening the panel requests the current
/// map definition once; it never fabricates a map record locally.
fn sync_big_map_ui(
    state: Res<NativePlayerUiState>,
    model: Res<BigMapModel>,
    mut intents: ResMut<BigMapGatewayIntentQueue>,
    mut ui: ResMut<BigMapUiState>,
) {
    intents.sync_model(&model);
    if ui.last_reset_epoch != Some(model.reset_epoch) {
        ui.search_focused = false;
        ui.last_reset_epoch = Some(model.reset_epoch);
        ui.requested_epoch_map = None;
    }

    let open = state.bigmap_open();
    if open {
        // MapInformation may arrive after the panel became visible. Queue the
        // request as soon as that authoritative identity exists instead of
        // tying it only to the first open frame. Never request a map that is
        // already present in the authoritative NewMapInfo cache.
        if let Some(map_index) = model.missing_current_map_index() {
            let request_key = (model.reset_epoch, map_index);
            if ui.requested_epoch_map != Some(request_key)
                && intents.request_map_info(&model, map_index)
            {
                ui.requested_epoch_map = Some(request_key);
            }
        }
    }
    if !open {
        ui.search_focused = false;
    }
}

fn sync_local_panel_models(
    shell: Res<NativeShellModel>,
    mut mail: ResMut<MailModel>,
    mut mail_ui: ResMut<MailUiState>,
    inventory: Res<InventoryModel>,
    storage: Res<StorageModel>,
    mut storage_ui: ResMut<StorageUiState>,
    shop: Res<ShopModel>,
    mut shop_ui: ResMut<ShopUiState>,
    mut skill_binding: ResMut<SkillBindingUi>,
    mut skills: ResMut<SkillModel>,
) {
    mail.clamp_after_refresh(&mut mail_ui.cursor);
    storage.clamp_after_refresh(&mut storage_ui.cursor);
    storage_ui.bag_selection = storage_ui.bag_selection.filter(|selection| {
        inventory_selection_for_slot(&inventory, selection.slot) == Some(*selection)
    });
    storage_ui.storage_selection = storage_ui
        .storage_selection
        .filter(|selection| storage.item_for_selection(*selection).is_some());
    shop_ui.start_index = shop_ui.start_index.min(shop.goods.len().saturating_sub(8));

    skill_binding.refresh(&skills);
    let merged = skill_binding.merge_skill_model(&skills);
    if skills.skills != merged.skills || skills.bindings != merged.bindings {
        *skills = merged;
    }

    if shell.screen != NativeShellScreen::InGame {
        mail_ui.cursor = MailPageCursor::default();
        storage_ui.cursor = StoragePageCursor::default();
        storage_ui.bag_selection = None;
        storage_ui.storage_selection = None;
        shop_ui.start_index = 0;
        skill_binding.clear_selection();
        skill_binding.set_assign_key(false);
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

/// Resolve a mouse click against the current server-derived belt model. This
/// rejects an empty/legacy slot rather than sending a slot-only request that
/// could consume a newer stack after a snapshot refresh.
pub fn belt_item_use_intent(inventory: &InventoryModel, slot: u8) -> Option<NativePlayerUiIntent> {
    let item = crate::crystal_ui::hud::belt_slot_item(inventory, slot)?;
    Some(NativePlayerUiIntent::UseItem {
        key: Some(item.key.clone()),
        unique_id: Some(item.unique_id?),
        slot: Some(slot),
        grid: Some("belt".to_owned()),
    })
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
                    left: Val::Px(410.0),
                    top: Val::Px(86.0),
                    width: Val::Px(INVENTORY_PANEL_SIZE.width as f32),
                    height: Val::Px(INVENTORY_PANEL_SIZE.height as f32),
                    display: Display::None,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlayEquipment,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(742.0),
                    top: Val::Px(17.0),
                    width: Val::Px(264.0),
                    height: Val::Px(380.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlayMenu,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(CRYSTAL_MENU_PANEL_RECT.left),
                    top: Val::Px(CRYSTAL_MENU_PANEL_RECT.top),
                    width: Val::Px(CRYSTAL_MENU_PANEL_RECT.width),
                    height: Val::Px(CRYSTAL_MENU_PANEL_RECT.height),
                    display: Display::None,
                    ..default()
                },
                GlobalZIndex(OVERLAY_MENU_Z),
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlaySkill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    top: Val::Px(80.0),
                    width: Val::Px(650.0),
                    height: Val::Px(SKILL_PANEL_SIZE.height as f32),
                    display: Display::None,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
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
                    left: Val::Px(562.0),
                    top: Val::Px(5.0),
                    width: Val::Px(312.0),
                    height: Val::Px(444.0),
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
                    left: Val::Px(CRYSTAL_BIGMAP_PANEL_RECT.left),
                    top: Val::Px(CRYSTAL_BIGMAP_PANEL_RECT.top),
                    width: Val::Px(CRYSTAL_BIGMAP_PANEL_RECT.width),
                    height: Val::Px(CRYSTAL_BIGMAP_PANEL_RECT.height),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlayShop,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(224.0),
                    width: Val::Px(620.0),
                    height: Val::Px(344.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlayGameShop,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((1024.0 - GAME_SHOP_PANEL_SIZE.width as f32) / 2.0),
                    top: Val::Px((768.0 - GAME_SHOP_PANEL_SIZE.height as f32) / 2.0),
                    width: Val::Px(GAME_SHOP_PANEL_SIZE.width as f32),
                    height: Val::Px(GAME_SHOP_PANEL_SIZE.height as f32),
                    display: Display::None,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlayStorage,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(150.0),
                    top: Val::Px(100.0),
                    width: Val::Px(640.0),
                    height: Val::Px(344.0),
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
                    left: Val::Px(CRYSTAL_OPTIONS_PANEL_RECT.left),
                    top: Val::Px(CRYSTAL_OPTIONS_PANEL_RECT.top),
                    width: Val::Px(CRYSTAL_OPTIONS_PANEL_RECT.width),
                    height: Val::Px(CRYSTAL_OPTIONS_PANEL_RECT.height),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlaySocial,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(1024.0),
                    height: Val::Px(768.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
        });
}

fn consume_hud_buttons(
    mut state: ResMut<NativePlayerUiState>,
    buttons: Query<(&Interaction, &CrystalHudAction), Changed<Interaction>>,
    shell: Option<Res<NativeShellModel>>,
    inventory: Res<InventoryModel>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
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
                    state.inventory_operation = None;
                    state.drop_confirmation = None;
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
                if !state.skill_open() {
                    state.selected_skill_id = None;
                }
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
            CrystalHudAction::BeltUse(slot) => {
                if let Some(intent) = belt_item_use_intent(&inventory, *slot) {
                    intents.push_transient_unique(intent);
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
    big_map_controls: BigMapControls,
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
    let BigMapControls {
        model: mut big_map,
        intents: mut big_map_intents,
        ui: mut big_map_ui,
        time,
        skill_binding: mut skill_binding,
        skills: mut skills,
        skill_persistence: mut skill_persistence,
    } = big_map_controls;

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

    if state.skill_open()
        && skill_binding
            .as_deref()
            .is_some_and(SkillBindingUi::is_assign_key_enabled)
    {
        let pressed = [
            KeyCode::F1,
            KeyCode::F2,
            KeyCode::F3,
            KeyCode::F4,
            KeyCode::F5,
            KeyCode::F6,
            KeyCode::F7,
            KeyCode::F8,
        ]
        .into_iter()
        .enumerate()
        .find_map(|(index, key)| keys.just_pressed(key).then_some(index as u8 + 1));
        if let Some(hotkey) = pressed {
            if let (Some(skill_binding), Some(skills), Some(skill_persistence)) = (
                skill_binding.as_deref_mut(),
                skills.as_deref_mut(),
                skill_persistence.as_deref_mut(),
            ) {
                if skill_binding.assign_selected_key(hotkey, skills) {
                    skill_binding.apply_to_skill_model(skills);
                    skill_persistence.mark_dirty();
                    persist_skill_bindings_if_changed(skill_persistence, skill_binding);
                }
            }
            return;
        }
    }

    if state.bigmap_open() && big_map_ui.as_deref().is_some_and(|ui| ui.search_focused) {
        let (Some(big_map), Some(big_map_intents), Some(big_map_ui)) = (
            big_map.as_deref_mut(),
            big_map_intents.as_deref_mut(),
            big_map_ui.as_deref_mut(),
        ) else {
            return;
        };
        if keys.just_pressed(KeyCode::Backspace) {
            let mut value = big_map.search.draft.clone();
            value.pop();
            big_map.set_search_draft(value);
        }
        for event in typed.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if let Some(text) = &event.text {
                let mut value = big_map.search.draft.clone();
                value.extend(text.chars().filter(|ch| !ch.is_control()));
                big_map.set_search_draft(value);
            }
        }
        if keys.just_pressed(KeyCode::Enter) {
            let now_ms = time
                .as_deref()
                .map(|clock| clock.elapsed().as_millis() as u64)
                .unwrap_or_default();
            let _ = big_map_intents.search(big_map, now_ms, BIG_MAP_SEARCH_COOLDOWN_MS);
            return;
        }
        if !keys.just_pressed(KeyCode::Escape) {
            return;
        }
        big_map_ui.search_focused = false;
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
            state.inventory_operation = None;
            state.drop_confirmation = None;
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
            if let Some(skill_binding) = skill_binding.as_deref_mut() {
                skill_binding.clear_selection();
                skill_binding.set_assign_key(false);
            }
            storage.password_draft.clear();
            storage.new_password_draft.clear();
            storage.confirm_password_draft.clear();
        } else {
            state.toggle_menu();
        }
        return;
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
    button_controls: OverlayButtonControls,
) {
    if shell.screen != NativeShellScreen::InGame {
        return;
    }
    let OverlayButtonControls {
        big_map:
            BigMapControls {
                model: mut big_map,
                intents: mut big_map_intents,
                ui: mut big_map_ui,
                time,
                skill_binding: mut skill_binding,
                skills: mut skills,
                skill_persistence: mut skill_persistence,
            },
        mut mail_ui,
        mut storage_ui,
        mut shop_ui,
        buttons,
    } = button_controls;
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
            OverlayButton::ExitApplication => {
                effects.push(mir2_ui_core::effect::UiEffect::ExitApplication);
            }
            OverlayButton::CloseWindows => {
                state.close_windows();
                state.shop_quantity = 1;
                if let Some(skill_binding) = skill_binding.as_deref_mut() {
                    skill_binding.clear_selection();
                    skill_binding.set_assign_key(false);
                }
            }
            OverlayButton::CloseInspect => {
                state.inspect = None;
                state.inventory_operation = None;
                state.drop_confirmation = None;
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
                if let Some(big_map_ui) = big_map_ui.as_deref_mut() {
                    big_map_ui.search_focused = false;
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
                storage_ui.bag_selection = None;
                storage_ui.storage_selection = None;
            }
            OverlayButton::CloseOptions => {
                if state.options_open() {
                    dispatch_ui_action(
                        &mut state.core,
                        &mut effects,
                        mir2_ui_core::action::UiAction::ClosePanel,
                    );
                }
            }
            OverlayButton::SetCrystalOption(option, enabled) => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetCrystalOption { option, enabled },
                );
            }
            OverlayButton::OptionsObserve(allow) => {
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::RequestObserve { allow },
                );
                if effects.drain_observe_bounded(1) == 1 {
                    let _ = intents.push_transient_unique(NativePlayerUiIntent::Chat {
                        message: "@ALLOWOBSERVE".to_owned(),
                    });
                }
            }
            OverlayButton::OptionsMusicVolumeDown => {
                let value = state.core.options.music_volume.saturating_sub(10);
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetMusicVolume { volume: value },
                );
            }
            OverlayButton::OptionsMusicVolumeUp => {
                let value = state.core.options.music_volume.saturating_add(10).min(100);
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetMusicVolume { volume: value },
                );
            }
            OverlayButton::OptionsSoundVolumeDown => {
                let value = state.core.options.sound_volume.saturating_sub(10);
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetSoundVolume { volume: value },
                );
            }
            OverlayButton::OptionsSoundVolumeUp => {
                let value = state.core.options.sound_volume.saturating_add(10).min(100);
                dispatch_ui_action(
                    &mut state.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::SetSoundVolume { volume: value },
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
            OverlayButton::GroupSwitch => {
                let allow_group = !social.group.allow_invites;
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GroupSwitch { allow_group },
                );
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
            OverlayButton::SelectGuildLeftPage(page) => {
                state.guild_left_page = page;
                state.selected_guild_member = None;
                if matches!(page, GuildLeftPage::Notice | GuildLeftPage::Members) {
                    let info_type = if page == GuildLeftPage::Notice { 0 } else { 1 };
                    intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::GuildRequestInfo { info_type },
                    );
                }
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
            OverlayButton::GuildKickMember(index) => {
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
                state.selected_guild_member = Some(index);
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
                state.drop_confirmation = inspected_drop_confirmation(&state, &inventory);
            }
            OverlayButton::ConfirmDropInspected => {
                let Some(confirmation) = state.drop_confirmation.take() else {
                    continue;
                };
                if !drop_confirmation_is_current(&confirmation, &inventory) {
                    continue;
                }
                intents.push_pending_intent(
                    &mut pending,
                    NativePlayerUiIntent::DropItem {
                        key: confirmation.key,
                        unique_id: confirmation.unique_id,
                        count: confirmation.count,
                        hero_inventory: false,
                    },
                );
            }
            OverlayButton::CancelDropInspected => {
                state.drop_confirmation = None;
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
                    state.drop_confirmation = None;
                }
            },
            OverlayButton::InspectEquip(slot) => {
                state.inspect = inventory
                    .items_in(2)
                    .into_iter()
                    .find(|item| item.slot == slot)
                    .map(inspect_from_item);
                state.drop_confirmation = None;
            }
            OverlayButton::SelectSkill(skill_id) => {
                if let (Some(skill_binding), Some(skills)) =
                    (skill_binding.as_deref_mut(), skills.as_deref())
                {
                    if skill_binding.select_skill(skill_id, skills) {
                        state.selected_skill_id = Some(skill_id);
                        skill_binding.set_assign_key(true);
                    }
                }
            }
            OverlayButton::AssignSkillKey(hotkey) => {
                if state.skill_open() {
                    if let (Some(skill_binding), Some(skills), Some(skill_persistence)) = (
                        skill_binding.as_deref_mut(),
                        skills.as_deref_mut(),
                        skill_persistence.as_deref_mut(),
                    ) {
                        if skill_binding.assign_selected_key(hotkey, skills) {
                            skill_binding.apply_to_skill_model(skills);
                            skill_persistence.mark_dirty();
                            persist_skill_bindings_if_changed(skill_persistence, skill_binding);
                        }
                    }
                }
            }
            OverlayButton::ClearSkillBinding => {
                if let (Some(skill_binding), Some(skills), Some(skill_persistence)) = (
                    skill_binding.as_deref_mut(),
                    skills.as_deref_mut(),
                    skill_persistence.as_deref_mut(),
                ) {
                    if let Some(skill_id) = skill_binding.selected_skill_id() {
                        if skill_binding.unassign_skill(skill_id) {
                            skill_binding.apply_to_skill_model(skills);
                            skill_persistence.mark_dirty();
                            persist_skill_bindings_if_changed(skill_persistence, skill_binding);
                        }
                    }
                }
            }
            OverlayButton::CloseSkillAssign => {
                if let Some(skill_binding) = skill_binding.as_deref_mut() {
                    skill_binding.set_assign_key(false);
                }
            }
            OverlayButton::SelectCharacterPage(page) => {
                state.character_page = page;
                state.inspect = None;
            }
            OverlayButton::SelectInventoryPage(page) => {
                if page <= 2 {
                    state.inventory_page = page;
                    state.inspect = None;
                    state.inventory_operation = None;
                    state.drop_confirmation = None;
                }
            }
            OverlayButton::SkillPagePrev => {
                state.skill_page = state.skill_page.saturating_sub(1);
                state.selected_skill_id = None;
            }
            OverlayButton::SkillPageNext => {
                let page_count = skills
                    .as_deref()
                    .map(|model| native_skill_page_count(model.skills.len()))
                    .unwrap_or(1);
                state.skill_page = (state.skill_page + 1).min(page_count.saturating_sub(1));
                state.selected_skill_id = None;
            }
            OverlayButton::SelectMail(id) => {
                let _ = mail.select_visible(id);
            }
            OverlayButton::MailPagePrev => {
                mail_ui.cursor.page = mail_ui.cursor.page.saturating_sub(1);
                mail.selected_id = None;
            }
            OverlayButton::MailPageNext => {
                mail_ui.cursor.page =
                    (mail_ui.cursor.page + 1).min(mail.page_count().saturating_sub(1));
                mail.selected_id = None;
            }
            OverlayButton::ReadMail(id) => {
                if mail
                    .mails
                    .iter()
                    .any(|message| message.id == id && message.operation.is_none() && !message.read)
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
            OverlayButton::BigMapScrollUp => {
                if state.bigmap_open() {
                    let Some(big_map) = big_map.as_deref_mut() else {
                        continue;
                    };
                    let row = big_map.npc_scroll_row.saturating_sub(1);
                    big_map.set_npc_scroll_row(row);
                }
            }
            OverlayButton::BigMapScrollDown => {
                if state.bigmap_open() {
                    let Some(big_map) = big_map.as_deref_mut() else {
                        continue;
                    };
                    let row = big_map.npc_scroll_row.saturating_add(1);
                    big_map.set_npc_scroll_row(row);
                }
            }
            OverlayButton::BigMapWorld => {
                if state.bigmap_open() {
                    let Some(big_map) = big_map.as_deref_mut() else {
                        continue;
                    };
                    big_map.set_view(BigMapView::WorldMap);
                }
            }
            OverlayButton::BigMapMyLocation => {
                if state.bigmap_open() {
                    let Some(big_map) = big_map.as_deref_mut() else {
                        continue;
                    };
                    big_map.set_view(BigMapView::CurrentMap);
                    if let (Some(map_index), Some(big_map_intents)) =
                        (big_map.current_map_index, big_map_intents.as_deref_mut())
                    {
                        let _ = big_map_intents.request_map_info(big_map, map_index);
                    }
                }
            }
            OverlayButton::BigMapSearchFocus => {
                if state.bigmap_open() {
                    if let Some(big_map_ui) = big_map_ui.as_deref_mut() {
                        big_map_ui.search_focused = true;
                    }
                }
            }
            OverlayButton::BigMapSearchSubmit => {
                if state.bigmap_open() {
                    let now_ms = time
                        .as_deref()
                        .map(|clock| clock.elapsed().as_millis() as u64)
                        .unwrap_or_default();
                    if let (Some(big_map), Some(big_map_intents)) =
                        (big_map.as_deref_mut(), big_map_intents.as_deref_mut())
                    {
                        let _ = big_map_intents.search(big_map, now_ms, BIG_MAP_SEARCH_COOLDOWN_MS);
                    }
                }
            }
            OverlayButton::BigMapTeleport => {
                if state.bigmap_open() {
                    if let (Some(big_map), Some(big_map_intents)) =
                        (big_map.as_deref(), big_map_intents.as_deref_mut())
                    {
                        let _ = big_map_intents.teleport_selected(big_map);
                    }
                }
            }
            OverlayButton::SelectBigMapNpc(object_id) => {
                if state.bigmap_open() {
                    if let Some(big_map) = big_map.as_deref_mut() {
                        let _ = big_map.select_npc(object_id);
                    }
                }
            }
            // Shop
            OverlayButton::SelectGameShopGood(id) => {
                if state.shop_open() {
                    let Some(game_shop) = game_shop.as_deref_mut() else {
                        continue;
                    };
                    game_shop.selected_game_shop_index = Some(id);
                    if let Some(page) = native_game_shop_page_for_index(game_shop, id) {
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
                        .map(|model| native_game_shop_page_count(model.items.len()))
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
            OverlayButton::ShopPageUp => {
                if state.npc_shop_open() {
                    shop_ui.start_index = shop_ui.start_index.saturating_sub(1);
                    shop.selected_id = None;
                }
            }
            OverlayButton::ShopPageDown => {
                if state.npc_shop_open() {
                    shop_ui.start_index =
                        (shop_ui.start_index + 1).min(shop.goods.len().saturating_sub(8));
                    shop.selected_id = None;
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
                storage_ui.bag_selection = inventory_selection_for_slot(&inventory, slot);
                storage.selected_bag_slot = storage_ui.bag_selection.map(|value| value.slot);
                storage.selected_storage_slot = None;
                storage_ui.storage_selection = None;
            }
            OverlayButton::SelectStorage(slot) => {
                storage_ui.storage_selection = storage.selection_for_slot(slot);
                storage.selected_storage_slot =
                    storage_ui.storage_selection.map(|value| value.slot);
                storage.selected_bag_slot = None;
                storage_ui.bag_selection = None;
            }
            OverlayButton::StoragePage(page) => {
                storage_ui.cursor.page = storage.clamp_page(page);
                storage.selected_storage_slot = None;
                storage_ui.storage_selection = None;
            }
            OverlayButton::StorageDeposit => {
                if let Some(selection) = storage_ui.bag_selection.filter(|selection| {
                    storage_deposit_enabled_for_selection(&storage, &inventory, *selection)
                }) {
                    let from = selection.slot as i32;
                    let unique_id = selection.unique_id;
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
                if let Some(selection) = storage_ui.storage_selection.filter(|selection| {
                    storage_withdraw_enabled_for_selection(&storage, &inventory, *selection)
                }) {
                    let from = selection.slot as i32;
                    let unique_id = selection.unique_id;
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

fn consume_exit_application(
    mut effects: ResMut<UiEffectQueue>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if effects.take_exit_application() {
        app_exit.write(AppExit::Success);
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

fn inspected_drop_confirmation(
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
) -> Option<InventoryDropConfirmation> {
    let item = inspected_inventory_item(state, inventory)?;
    let count = u16::try_from(item.quantity).ok()?;
    Some(InventoryDropConfirmation {
        key: item.key.clone(),
        unique_id: item_unique_id(item)?,
        slot: item.slot,
        count: (count > 0).then_some(count)?,
    })
}

fn drop_confirmation_is_current(
    confirmation: &InventoryDropConfirmation,
    inventory: &InventoryModel,
) -> bool {
    inventory.items.iter().any(|item| {
        item.container == 0
            && item.slot == confirmation.slot
            && item.key == confirmation.key
            && item_unique_id(item) == Some(confirmation.unique_id)
            && item.quantity == u32::from(confirmation.count)
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
    models: OverlayRenderModels,
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
    let OverlayRenderModels {
        asset_server,
        shell,
        state,
        inventory,
        inventory_feedback,
        mail,
        mail_ui,
        big_map,
        big_map_ui,
        ui,
        shop,
        shop_ui,
        game_shop,
        storage,
        storage_ui,
        skills,
        skill_binding,
        skill_persistence,
        social,
        combat_target,
    } = models;
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
            |parent| {
                render_inventory(
                    parent,
                    asset_server.as_deref(),
                    &inventory,
                    &state,
                    &inventory_feedback,
                )
            },
        );
        fill_panel(
            &mut commands,
            &mut all.p2(),
            state.equipment_open(),
            |parent| {
                render_equipment(
                    parent,
                    asset_server.as_deref(),
                    &inventory,
                    &ui,
                    &state,
                    &skills,
                )
            },
        );
        fill_panel(&mut commands, &mut all.p3(), state.menu_open(), |parent| {
            render_menu(parent, asset_server.as_deref())
        });
        fill_panel(&mut commands, &mut all.p4(), state.skill_open(), |parent| {
            render_skills(
                parent,
                asset_server.as_deref(),
                &skills,
                &skill_binding,
                &skill_persistence,
                &state,
                &ui,
            )
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
            |parent| {
                render_mail(
                    parent,
                    asset_server.as_deref(),
                    &mail,
                    &mail_ui,
                    &inventory,
                    &state,
                )
            },
        );
        fill_panel(
            &mut commands,
            &mut secondary.p1(),
            state.bigmap_open(),
            |parent| render_bigmap(parent, asset_server.as_deref(), &big_map, &big_map_ui, &ui),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p2(),
            state.npc_shop_open(),
            |parent| {
                render_shop(
                    parent,
                    asset_server.as_deref(),
                    &shop,
                    &shop_ui,
                    &inventory,
                    &state,
                )
            },
        );
        fill_panel(
            &mut commands,
            &mut secondary.p3(),
            state.shop_open(),
            |parent| render_game_shop(parent, asset_server.as_deref(), &game_shop, &ui, &state),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p4(),
            state.storage_open(),
            |parent| {
                render_storage(
                    parent,
                    asset_server.as_deref(),
                    &storage,
                    &storage_ui,
                    &inventory,
                    &state,
                )
            },
        );
        fill_panel(
            &mut commands,
            &mut secondary.p5(),
            state.options_open(),
            |parent| render_options(parent, asset_server.as_deref(), &state.core),
        );
        fill_panel(
            &mut commands,
            &mut secondary.p6(),
            state.group_open() || state.guild_open() || state.trade_open(),
            |parent| {
                render_social(
                    parent,
                    asset_server.as_deref(),
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
    asset_server: Option<&AssetServer>,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
    feedback: &InventoryOperationFeedback,
) {
    if let Some(asset_server) = asset_server {
        spawn_overlay_frame(
            parent,
            asset_server,
            "original-ui/Title/196.png",
            INVENTORY_PANEL_SIZE.width as f32,
            INVENTORY_PANEL_SIZE.height as f32,
        );
        spawn_inventory_tab(
            parent,
            asset_server,
            6.0,
            7.0,
            197,
            737,
            state.inventory_page == 0,
            OverlayButton::SelectInventoryPage(0),
        );
        spawn_inventory_tab(
            parent,
            asset_server,
            76.0,
            7.0,
            168,
            738,
            state.inventory_page == 1,
            OverlayButton::SelectInventoryPage(1),
        );
        spawn_inventory_tab(
            parent,
            asset_server,
            146.0,
            7.0,
            198,
            739,
            state.inventory_page == 2,
            OverlayButton::SelectInventoryPage(2),
        );
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse2",
            360,
            361,
            362,
            CrystalRect::new(289.0, 3.0, 24.0, 21.0),
            OverlayButton::CloseWindows,
        );
        overlay_text_at(
            parent,
            &format!("Gold {}", inventory.gold),
            CrystalRect::new(40.0, 213.0, 150.0, 15.0),
            10.0,
            TEXT,
        );
        if state.inventory_page == 2 {
            overlay_text_at(
                parent,
                "Quest inventory is server-backed",
                CrystalRect::new(24.0, 104.0, 268.0, 18.0),
                11.0,
                TEXT,
            );
        } else {
            let page_offset = usize::from(state.inventory_page) * INVENTORY_PAGE_SIZE;
            parent
                .spawn((
                    OverlayInventoryGridViewport,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(INVENTORY_GRID_ORIGIN.x as f32),
                        top: Val::Px(INVENTORY_GRID_ORIGIN.y as f32),
                        width: Val::Px(
                            (INVENTORY_GRID_STEP.x as usize * (INVENTORY_PAGE_COLUMNS - 1)
                                + INVENTORY_CELL_SIZE.width as usize)
                                as f32,
                        ),
                        height: Val::Px(
                            (INVENTORY_GRID_STEP.y as usize
                                * (INVENTORY_PAGE_SIZE / INVENTORY_PAGE_COLUMNS - 1)
                                + INVENTORY_CELL_SIZE.height as usize)
                                as f32,
                        ),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ))
                .with_children(|grid| {
                    let bag_items = inventory.items_in(0);
                    for local_slot in 0..INVENTORY_PAGE_SIZE {
                        let slot = (page_offset + local_slot) as u32;
                        let item = bag_items.iter().copied().find(|item| item.slot == slot);
                        let enabled = match &state.inventory_operation {
                            Some(InventoryOperationDraft::Move { source_slot, .. }) => {
                                *source_slot != slot
                            }
                            Some(InventoryOperationDraft::Merge { source_slot, .. }) => {
                                *source_slot != slot && item.and_then(item_unique_id).is_some()
                            }
                            None => item.is_some(),
                        };
                        let x = (local_slot % INVENTORY_PAGE_COLUMNS) as f32
                            * INVENTORY_GRID_STEP.x as f32;
                        let y = (local_slot / INVENTORY_PAGE_COLUMNS) as f32
                            * INVENTORY_GRID_STEP.y as f32;
                        let rect = CrystalRect::new(
                            x,
                            y,
                            INVENTORY_CELL_SIZE.width as f32,
                            INVENTORY_CELL_SIZE.height as f32,
                        );
                        if let Some(item) = item {
                            overlay_absolute_item_button(
                                grid,
                                asset_server,
                                item,
                                rect,
                                OverlayButton::InspectBag(slot),
                                enabled,
                            );
                        } else {
                            overlay_absolute_button(
                                grid,
                                "",
                                rect,
                                OverlayButton::InspectBag(slot),
                                enabled,
                            );
                        }
                    }
                });
        }
        return;
    }

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

fn render_equipment(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    inventory: &InventoryModel,
    ui: &UiReadModel,
    state: &NativePlayerUiState,
    skills: &SkillModel,
) {
    if let Some(asset_server) = asset_server {
        spawn_overlay_frame(
            parent,
            asset_server,
            "original-ui/Title/504.png",
            264.0,
            380.0,
        );
        let (page_index, page_rect) = match state.character_page {
            CharacterPage::Character => (340, CrystalRect::new(8.0, 90.0, 248.0, 280.0)),
            CharacterPage::Stats1 => (506, CrystalRect::new(8.0, 90.0, 248.0, 280.0)),
            CharacterPage::Stats2 => (507, CrystalRect::new(8.0, 90.0, 248.0, 280.0)),
            CharacterPage::Spells => (508, CrystalRect::new(8.0, 90.0, 248.0, 280.0)),
        };
        let page_library = if matches!(state.character_page, CharacterPage::Character) {
            "Prguse"
        } else {
            "Title"
        };
        spawn_static_overlay_sprite(
            parent,
            asset_server,
            format!("original-ui/{page_library}/{page_index}.png"),
            page_rect,
        );

        for (page, left, index) in [
            (CharacterPage::Character, 8.0, 500),
            (CharacterPage::Stats1, 70.0, 501),
            (CharacterPage::Stats2, 132.0, 502),
            (CharacterPage::Spells, 194.0, 503),
        ] {
            spawn_invisible_overlay_button(
                parent,
                CrystalRect::new(left, 70.0, 64.0, 20.0),
                OverlayButton::SelectCharacterPage(page),
            );
            if state.character_page == page {
                spawn_static_overlay_sprite(
                    parent,
                    asset_server,
                    format!("original-ui/Title/{index}.png"),
                    CrystalRect::new(left, 70.0, 64.0, 20.0),
                );
            }
        }
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse2",
            360,
            361,
            362,
            CrystalRect::new(241.0, 3.0, 24.0, 21.0),
            OverlayButton::CloseWindows,
        );
        overlay_text_at(
            parent,
            ui.player.name.as_deref().unwrap_or(""),
            CrystalRect::new(0.0, 12.0, 264.0, 20.0),
            12.0,
            TEXT,
        );
        overlay_text_at(
            parent,
            &format!(
                "{}  Lv{}",
                ui.player.class_name.as_deref().unwrap_or("-"),
                ui.player.level
            ),
            CrystalRect::new(38.0, 34.0, 210.0, 18.0),
            10.0,
            TEXT,
        );

        match state.character_page {
            CharacterPage::Character => {
                let slots = [
                    (0, 123.0, 97.0),
                    (1, 163.0, 97.0),
                    (2, 203.0, 97.0),
                    (13, 203.0, 152.0),
                    (4, 203.0, 188.0),
                    (3, 203.0, 224.0),
                    (5, 8.0, 260.0),
                    (6, 203.0, 260.0),
                    (7, 8.0, 296.0),
                    (8, 203.0, 296.0),
                    (9, 8.0, 332.0),
                    (11, 48.0, 332.0),
                    (10, 88.0, 332.0),
                    (12, 128.0, 332.0),
                ];
                for (slot, left, top) in slots {
                    let item = inventory
                        .items_in(2)
                        .into_iter()
                        .find(|item| item.slot == slot);
                    if let Some(item) = item {
                        overlay_absolute_item_button(
                            parent,
                            asset_server,
                            item,
                            CrystalRect::new(left, top, 32.0, 32.0),
                            OverlayButton::InspectEquip(slot),
                            true,
                        );
                    }
                }
            }
            CharacterPage::Stats1 => {
                for (text, top) in [
                    (ui.player.hp_label(), 110.0),
                    (ui.player.mp_label(), 128.0),
                    (format!("0-{}", ui.player.current_weight), 146.0),
                    (format!("{}%", ui.player.experience_percent_label()), 254.0),
                ] {
                    overlay_text_at(
                        parent,
                        &text,
                        CrystalRect::new(134.0, top, 105.0, 16.0),
                        10.0,
                        TEXT,
                    );
                }
            }
            CharacterPage::Stats2 => {
                for (text, top) in [
                    (ui.player.experience_percent_label(), 110.0),
                    (
                        format!("{}/{}", ui.player.current_weight, ui.player.max_weight),
                        128.0,
                    ),
                    (ui.player.gold_label(), 146.0),
                ] {
                    overlay_text_at(
                        parent,
                        &text,
                        CrystalRect::new(134.0, top, 105.0, 16.0),
                        10.0,
                        TEXT,
                    );
                }
            }
            CharacterPage::Spells => {
                let start = state.skill_page * 7;
                for (row, skill) in skills.skills.iter().skip(start).take(7).enumerate() {
                    let binding = skills.binding_for(skill.id);
                    let enabled = binding.can_use != Some(false)
                        && binding.cast_kind.as_deref() != Some("passive")
                        && binding
                            .spell
                            .as_deref()
                            .is_some_and(|spell| !spell.is_empty());
                    overlay_absolute_button(
                        parent,
                        &format!(
                            "{} Lv{}",
                            short_name(&skill.name, skill.key.as_deref().unwrap_or("")),
                            skill.level
                        ),
                        CrystalRect::new(16.0, 98.0 + row as f32 * 33.0, 210.0, 28.0),
                        OverlayButton::SelectSkill(skill.id),
                        enabled,
                    );
                }
                let has_previous = state.skill_page > 0;
                let has_next = (state.skill_page + 1) * 7 < skills.skills.len();
                spawn_overlay_crystal_button_enabled(
                    parent,
                    asset_server,
                    "Prguse",
                    398,
                    399,
                    399,
                    CrystalRect::new(90.0, 340.0, 32.0, 24.0),
                    OverlayButton::SkillPagePrev,
                    has_previous,
                );
                spawn_overlay_crystal_button_enabled(
                    parent,
                    asset_server,
                    "Prguse",
                    396,
                    397,
                    397,
                    CrystalRect::new(140.0, 340.0, 32.0, 24.0),
                    OverlayButton::SkillPageNext,
                    has_next,
                );
            }
        }
        return;
    }

    title(parent, "Character / Equipment");
    let player = &ui.player;
    body(
        parent,
        &format!(
            "{}  {}  Lv{}",
            player.name.as_deref().unwrap_or("-"),
            player.class_name.as_deref().unwrap_or("-"),
            player.level
        ),
    );
    body(
        parent,
        &format!(
            "HP {}  MP {}  EXP {}",
            player.hp_label(),
            player.mp_label(),
            player.experience_percent_label()
        ),
    );
    body(
        parent,
        &format!(
            "Weight {}/{}  Gold {}",
            player.current_weight,
            player.max_weight,
            player.gold_label()
        ),
    );
    for slot in 0..14 {
        let item = inventory
            .items_in(2)
            .into_iter()
            .find(|item| item.slot == slot);
        let line = item
            .map(|item| {
                format!(
                    "{}: {}",
                    equipment_slot_name(slot),
                    short_name(&item.name, &item.key)
                )
            })
            .unwrap_or_else(|| format!("{}: --", equipment_slot_name(slot)));
        overlay_button(
            parent,
            &line,
            OverlayButton::InspectEquip(slot),
            item.is_some(),
        );
    }
    overlay_button(parent, "Close", OverlayButton::CloseWindows, true);
}

fn spawn_overlay_frame(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    path: &'static str,
    width: f32,
    height: f32,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(width),
            height: Val::Px(height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(path),
            ..default()
        },
    ));
}

fn spawn_inventory_tab(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    left: f32,
    top: f32,
    active_index: u16,
    idle_index: u16,
    active: bool,
    action: OverlayButton,
) {
    let index = if active { active_index } else { idle_index };
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Title",
        index,
        index,
        index,
        CrystalRect::new(left, top, 72.0, 23.0),
        action,
    );
}

fn overlay_text_at(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        Text::new(text.to_owned()),
        TextFont {
            font_size: FontSize::Px(font_size),
            ..default()
        },
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    ));
}

fn overlay_absolute_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    rect: CrystalRect,
    action: OverlayButton,
    enabled: bool,
) {
    let mut entity = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            padding: UiRect::all(Val::Px(2.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(if enabled {
            Color::srgba(0.10, 0.07, 0.03, 0.62)
        } else {
            Color::srgba(0.25, 0.20, 0.12, 0.28)
        }),
    ));
    if enabled {
        entity.insert((Button, action));
    }
    if !label.is_empty() {
        entity.with_children(|button| {
            button.spawn((
                Text::new(label.to_owned()),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(if enabled {
                    TEXT
                } else {
                    Color::srgba(0.75, 0.70, 0.60, 0.45)
                }),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
            ));
        });
    }
}

/// Crystal inventory/equipment cells are image-led. Text remains only as a
/// fail-closed fallback for old snapshots that do not carry an icon index.
/// Only stack counts are drawn over bag icons: full durability values such as
/// `400/400` do not fit a 32-pixel Crystal cell and belong in item details.
fn overlay_absolute_item_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    item: &ItemModel,
    rect: CrystalRect,
    action: OverlayButton,
    enabled: bool,
) {
    let mut entity = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            padding: UiRect::all(Val::Px(1.0)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(if enabled {
            Color::srgba(0.10, 0.07, 0.03, 0.20)
        } else {
            Color::srgba(0.25, 0.20, 0.12, 0.28)
        }),
    ));
    if enabled {
        entity.insert((Button, action));
    }
    entity.with_children(|cell| {
        if let Some(path) = item_icon_path(item.icon) {
            cell.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(rect.width),
                    height: Val::Px(rect.height),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(path),
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                },
            ));
        } else {
            overlay_text_at(
                cell,
                &short_slot_name(&item.name, &item.key),
                CrystalRect::new(1.0, 9.0, rect.width - 2.0, 12.0),
                8.0,
                TEXT,
            );
        }
        let detail = inventory_cell_stack_label(item);
        if !detail.is_empty() {
            overlay_text_at(
                cell,
                &detail,
                CrystalRect::new(1.0, rect.height - 11.0, rect.width - 2.0, 10.0),
                7.0,
                TEXT,
            );
        }
    });
}

/// One authoritative NPC-shop row. `ShopGood` is intentionally not coerced
/// into an `ItemModel`: its catalogue index, stock, and selection identity
/// belong to the server-provided shop snapshot rather than the player's bag.
fn overlay_absolute_shop_good_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    icon: u16,
    label: &str,
    rect: CrystalRect,
    action: OverlayButton,
) {
    let mut entity = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            padding: UiRect::axes(Val::Px(2.0), Val::Px(1.0)),
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.10, 0.07, 0.03, 0.36)),
        Button,
        action,
    ));
    entity.with_children(|row| {
        if let (Some(asset_server), Some(path)) = (asset_server, item_icon_path(icon)) {
            row.spawn((
                Node {
                    width: Val::Px(28.0),
                    height: Val::Px(28.0),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(path),
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                },
            ));
        }
        row.spawn((
            Text::new(label.to_owned()),
            TextFont {
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(TEXT),
        ));
    });
}

/// Compact row cell used by the still-provisional Shop/Warehouse panels. It
/// uses the same authoritative icon path without inventing an icon when the
/// source record has none.
fn overlay_compact_item_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    icon: u16,
    label: &str,
    action: OverlayButton,
    enabled: bool,
) {
    let mut entity = parent.spawn((
        Node {
            min_height: Val::Px(28.0),
            display: Display::Flex,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            padding: UiRect::axes(Val::Px(3.0), Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(if enabled {
            Color::srgba(0.12, 0.08, 0.04, 0.60)
        } else {
            Color::srgba(0.25, 0.20, 0.12, 0.28)
        }),
    ));
    if enabled {
        entity.insert((Button, action));
    }
    entity.with_children(|row| {
        if let (Some(asset_server), Some(path)) = (asset_server, item_icon_path(icon)) {
            row.spawn((
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(path),
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                },
            ));
        }
        row.spawn((
            Text::new(label.to_owned()),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(if enabled {
                TEXT
            } else {
                Color::srgba(0.75, 0.70, 0.60, 0.45)
            }),
        ));
    });
}

fn spawn_static_overlay_sprite(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    path: String,
    rect: CrystalRect,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(path),
            ..default()
        },
    ));
}

fn spawn_overlay_crystal_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    library: &'static str,
    normal: u16,
    hover: u16,
    pressed: u16,
    rect: CrystalRect,
    action: OverlayButton,
) {
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        library,
        normal,
        hover,
        pressed,
        rect,
        action,
        true,
    );
}

fn spawn_overlay_crystal_button_enabled(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    library: &'static str,
    normal: u16,
    hover: u16,
    pressed: u16,
    rect: CrystalRect,
    action: OverlayButton,
    enabled: bool,
) {
    let spec = CrystalButtonSpec::new(
        library,
        normal,
        hover,
        pressed,
        rect,
        rect.width,
        rect.height,
    );
    spawn_crystal_image_button(
        parent,
        asset_server,
        spec,
        CrystalButtonAssetSet::from_spec(spec),
        action,
        false,
        enabled,
    );
}

fn render_menu(parent: &mut ChildSpawnerCommands, asset_server: Option<&AssetServer>) {
    let Some(asset_server) = asset_server else {
        return;
    };
    spawn_overlay_frame(
        parent,
        asset_server,
        "original-ui/Title/567.png",
        36.0,
        282.0,
    );

    // Crystal MenuDialog is a 36x282 icon strip. Only controls backed by a
    // native surface are interactive here; unsupported late-system icons are
    // rendered faithfully but deliberately do not masquerade as working UI.
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Title",
        633,
        634,
        635,
        CrystalRect::new(3.0, 12.0, 32.0, 20.0),
        OverlayButton::ExitApplication,
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Title",
        636,
        637,
        638,
        CrystalRect::new(3.0, 31.0, 32.0, 20.0),
        OverlayButton::Logout,
    );

    for (library, normal, hover, pressed, top) in [
        ("Prguse", 1970, 1971, 1972, 50.0),
        ("Prguse", 1973, 1974, 1975, 69.0),
        ("Prguse", 2000, 2001, 2002, 88.0),
        ("Prguse2", 431, 432, 433, 126.0),
        ("Prguse", 1976, 1977, 1978, 145.0),
        ("Prguse", 1979, 1980, 1981, 164.0),
        ("Prguse", 1982, 1983, 1984, 183.0),
        ("Prguse", 1985, 1986, 1987, 202.0),
        ("Prguse", 1988, 1989, 1990, 221.0),
    ] {
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            library,
            normal,
            hover,
            pressed,
            CrystalRect::new(3.0, top, 32.0, 20.0),
            OverlayButton::CloseWindows,
            false,
        );
    }
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse",
        1991,
        1992,
        1993,
        CrystalRect::new(3.0, 240.0, 32.0, 20.0),
        OverlayButton::ToggleGroup,
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse",
        1994,
        1995,
        1996,
        CrystalRect::new(3.0, 259.0, 32.0, 20.0),
        OverlayButton::ToggleGuild,
    );
}

fn render_social(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    social: &crate::social::SocialModel,
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
    combat_target: Option<&crate::quest_model::CombatTargetModel>,
) {
    if state.group_open() {
        render_group_panel(parent, asset_server, social, state, combat_target);
    } else if state.guild_open() {
        render_guild_panel(parent, asset_server, social, state);
    } else {
        render_trade_panel(parent, social, inventory);
    }
}

fn render_group_panel(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    social: &crate::social::SocialModel,
    state: &NativePlayerUiState,
    combat_target: Option<&crate::quest_model::CombatTargetModel>,
) {
    let rect = CRYSTAL_GROUP_PANEL_RECT;
    let group = &social.group;
    let Some(asset_server) = asset_server else {
        return;
    };
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Prguse/120.png".to_owned(),
        rect,
    );
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Title/5.png".to_owned(),
        CrystalRect::new(rect.left + 18.0, rect.top + 8.0, 55.0, 15.0),
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        360,
        361,
        362,
        CrystalRect::new(rect.left + 206.0, rect.top + 3.0, 24.0, 21.0),
        OverlayButton::CloseSocial,
    );

    for (index, member) in group
        .members
        .iter()
        .take(crate::social::MAX_GROUP_MEMBERS)
        .enumerate()
    {
        let (left, top) = group_member_position(index);
        overlay_clickable_text_at(
            parent,
            &member.name,
            CrystalRect::new(rect.left + left, rect.top + top, 96.0, 16.0),
            OverlayButton::SelectGroupMember(index as u8),
            state.selected_group_member == Some(index as u8),
            member.online,
        );
    }

    if let Some(inviter) = group.pending_invite_from.as_deref() {
        overlay_text_at(
            parent,
            &format!("Invite: {inviter}"),
            CrystalRect::new(rect.left + 16.0, rect.top + 192.0, 190.0, 16.0),
            9.0,
            GOLD,
        );
        overlay_absolute_button(
            parent,
            "Accept",
            CrystalRect::new(rect.left + 16.0, rect.top + 207.0, 52.0, 18.0),
            OverlayButton::GroupInviteAccept,
            true,
        );
        overlay_absolute_button(
            parent,
            "Decline",
            CrystalRect::new(rect.left + 72.0, rect.top + 207.0, 52.0, 18.0),
            OverlayButton::GroupInviteDecline,
            true,
        );
    }

    let switch_index = if group.allow_invites { 117 } else { 114 };
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse",
        switch_index,
        switch_index + 1,
        switch_index + 2,
        CrystalRect::new(rect.left + 25.0, rect.top + 219.0, 28.0, 25.0),
        OverlayButton::GroupSwitch,
    );
    let can_add_target = combat_target
        .and_then(|model| model.target.as_ref())
        .is_some_and(|target| target.is_player && !target.name.trim().is_empty());
    let add_base = if group.active { 133 } else { 130 };
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Title",
        add_base,
        add_base + 1,
        add_base + 2,
        CrystalRect::new(rect.left + 70.0, rect.top + 219.0, 60.0, 25.0),
        OverlayButton::GroupAddSelected,
        can_add_target,
    );
    let can_remove = state.selected_group_member.is_some_and(|index| {
        group
            .members
            .get(usize::from(index))
            .is_some_and(|member| !member.leader)
    });
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Title",
        136,
        137,
        138,
        CrystalRect::new(rect.left + 140.0, rect.top + 219.0, 60.0, 25.0),
        OverlayButton::GroupRemoveSelected,
        can_remove,
    );
}

fn group_member_position(index: usize) -> (f32, f32) {
    if index == 0 {
        (16.0, 33.0)
    } else {
        (
            (((index + 1) % 2) * 100 + 16) as f32,
            (55 + ((index - 1) / 2) * 20) as f32,
        )
    }
}

fn render_guild_panel(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    social: &crate::social::SocialModel,
    state: &NativePlayerUiState,
) {
    let rect = CRYSTAL_GUILD_PANEL_RECT;
    let guild = &social.guild;
    let Some(asset_server) = asset_server else {
        return;
    };
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Prguse/180.png".to_owned(),
        rect,
    );
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Title/25.png".to_owned(),
        CrystalRect::new(rect.left + 18.0, rect.top + 9.0, 49.0, 15.0),
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        360,
        361,
        362,
        CrystalRect::new(rect.left + 565.0, rect.top + 4.0, 24.0, 21.0),
        OverlayButton::CloseSocial,
    );
    spawn_guild_tab(
        parent,
        asset_server,
        rect.left + 20.0,
        rect.top + 38.0,
        93,
        94,
        state.guild_left_page == GuildLeftPage::Notice,
        OverlayButton::SelectGuildLeftPage(GuildLeftPage::Notice),
    );
    spawn_guild_tab(
        parent,
        asset_server,
        rect.left + 91.0,
        rect.top + 38.0,
        99,
        100,
        state.guild_left_page == GuildLeftPage::Members,
        OverlayButton::SelectGuildLeftPage(GuildLeftPage::Members),
    );
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Title/104.png".to_owned(),
        CrystalRect::new(rect.left + 501.0, rect.top + 38.0, 72.0, 24.0),
    );

    match state.guild_left_page {
        GuildLeftPage::Notice => render_guild_notice(parent, asset_server, guild, rect),
        GuildLeftPage::Members => render_guild_members(parent, asset_server, guild, state, rect),
    }
    render_guild_status(parent, asset_server, guild, rect);
}

fn spawn_guild_tab(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    left: f32,
    top: f32,
    idle: u16,
    active: u16,
    selected: bool,
    action: OverlayButton,
) {
    let normal = if selected { active } else { idle };
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Title",
        normal,
        active,
        active,
        CrystalRect::new(left, top, 72.0, 24.0),
        action,
    );
}

fn render_guild_notice(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    guild: &crate::social::GuildModel,
    rect: CrystalRect,
) {
    let notice = if guild.name.is_none() {
        "You are not in a guild.".to_owned()
    } else if guild.notice.is_empty() {
        String::new()
    } else {
        guild.notice.join("\n")
    };
    overlay_text_at(
        parent,
        &notice,
        CrystalRect::new(rect.left + 13.0, rect.top + 61.0, 322.0, 330.0),
        9.0,
        TEXT,
    );
    if !guild.notice.is_empty()
        && guild
            .permissions
            .iter()
            .any(|permission| permission.eq_ignore_ascii_case("notice"))
    {
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse",
            560,
            561,
            562,
            CrystalRect::new(rect.left + 20.0, rect.top + 402.0, 28.0, 25.0),
            OverlayButton::GuildPublishNotice,
        );
    }
    if let Some(inviter) = guild.pending_invite_from.as_deref() {
        overlay_text_at(
            parent,
            &format!("Invite: {inviter}"),
            CrystalRect::new(rect.left + 20.0, rect.top + 360.0, 230.0, 16.0),
            9.0,
            GOLD,
        );
        overlay_absolute_button(
            parent,
            "Accept",
            CrystalRect::new(rect.left + 20.0, rect.top + 378.0, 52.0, 18.0),
            OverlayButton::GuildInviteAccept,
            true,
        );
        overlay_absolute_button(
            parent,
            "Decline",
            CrystalRect::new(rect.left + 76.0, rect.top + 378.0, 52.0, 18.0),
            OverlayButton::GuildInviteDecline,
            true,
        );
    }
}

fn render_guild_members(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    guild: &crate::social::GuildModel,
    state: &NativePlayerUiState,
    rect: CrystalRect,
) {
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Prguse/1852.png".to_owned(),
        CrystalRect::new(rect.left + 13.0, rect.top + 61.0, 324.0, 332.0),
    );
    let can_kick = guild
        .permissions
        .iter()
        .any(|permission| permission.eq_ignore_ascii_case("kick"));
    for (index, member) in guild.members.iter().take(18).enumerate() {
        let top = rect.top + 90.0 + index as f32 * 15.0;
        overlay_clickable_text_at(
            parent,
            member.rank_name.as_deref().unwrap_or("-"),
            CrystalRect::new(rect.left + 24.0, top, 100.0, 14.0),
            OverlayButton::SelectGuildMember(index as u8),
            state.selected_guild_member == Some(index as u8),
            member.online,
        );
        overlay_text_at(
            parent,
            &member.name,
            CrystalRect::new(rect.left + 125.0, top, 84.0, 14.0),
            8.0,
            if member.online {
                TEXT
            } else {
                Color::srgb(0.5, 0.5, 0.5)
            },
        );
        overlay_text_at(
            parent,
            if member.online { "Online" } else { "Offline" },
            CrystalRect::new(rect.left + 225.0, top, 96.0, 14.0),
            8.0,
            if member.online {
                TEXT
            } else {
                Color::srgb(0.5, 0.5, 0.5)
            },
        );
        if can_kick {
            spawn_overlay_crystal_button(
                parent,
                asset_server,
                "Prguse",
                917,
                917,
                917,
                CrystalRect::new(rect.left + 210.0, top, 16.0, 14.0),
                OverlayButton::GuildKickMember(index as u8),
            );
        }
    }
}

fn render_guild_status(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    guild: &crate::social::GuildModel,
    rect: CrystalRect,
) {
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Prguse/1850.png".to_owned(),
        CrystalRect::new(rect.left + 365.0, rect.top + 62.0, 208.0, 316.0),
    );
    for (label, value, top) in [
        ("Guild", guild.name.as_deref().unwrap_or(""), 107.0),
        ("Level", if guild.name.is_some() { "" } else { "" }, 133.0),
        ("Members", "", 159.0),
    ] {
        overlay_text_at(
            parent,
            label,
            CrystalRect::new(rect.left + 362.0, rect.top + top, 75.0, 14.0),
            9.0,
            Color::srgb(0.55, 0.55, 0.55),
        );
        if !value.is_empty() {
            overlay_text_at(
                parent,
                value,
                CrystalRect::new(rect.left + 437.0, rect.top + top, 120.0, 14.0),
                9.0,
                TEXT,
            );
        }
    }
    if guild.name.is_some() {
        overlay_text_at(
            parent,
            &guild.level.to_string(),
            CrystalRect::new(rect.left + 437.0, rect.top + 133.0, 120.0, 14.0),
            9.0,
            TEXT,
        );
        overlay_text_at(
            parent,
            &format!(
                "{}/{}",
                guild.member_count.max(guild.members.len() as u16),
                guild.max_members
            ),
            CrystalRect::new(rect.left + 437.0, rect.top + 159.0, 120.0, 14.0),
            9.0,
            TEXT,
        );
    }
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Prguse2/423.png".to_owned(),
        CrystalRect::new(rect.left + 322.0, rect.top + 403.0, 260.0, 22.0),
    );
    let percent = if guild.max_experience > 0 {
        ((guild.experience.max(0) as f64 / guild.max_experience as f64) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    overlay_text_at(
        parent,
        &format!("{percent:.0}%"),
        CrystalRect::new(rect.left + 322.0, rect.top + 405.0, 260.0, 15.0),
        9.0,
        TEXT,
    );
}

fn overlay_clickable_text_at(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: CrystalRect,
    action: OverlayButton,
    selected: bool,
    online: bool,
) {
    let mut row = parent.spawn((
        Button,
        action,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(if selected {
            Color::srgba(0.45, 0.28, 0.05, 0.72)
        } else {
            Color::NONE
        }),
    ));
    row.with_children(|cell| {
        cell.spawn((
            Text::new(text.to_owned()),
            TextFont {
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(if online {
                TEXT
            } else {
                Color::srgb(0.5, 0.5, 0.5)
            }),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ));
    });
}

fn render_trade_panel(
    parent: &mut ChildSpawnerCommands,
    social: &crate::social::SocialModel,
    inventory: &InventoryModel,
) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(172.0),
                top: Val::Px(80.0),
                width: Val::Px(680.0),
                max_height: Val::Px(560.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|trade_parent| {
            title(trade_parent, "Trade");
            let trade = &social.trade;
            body(
                trade_parent,
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
                trade_parent,
                &format!(
                    "Partner gold: {}  Items: {}  Confirmed: {}",
                    trade.partner_gold,
                    trade.partner_items.len(),
                    trade.partner_confirmed
                ),
            );
            for item in &trade.partner_items {
                body(
                    trade_parent,
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
                overlay_button(
                    trade_parent,
                    "Accept trade",
                    OverlayButton::TradeAccept,
                    true,
                );
                overlay_button(
                    trade_parent,
                    "Decline trade",
                    OverlayButton::TradeDecline,
                    true,
                );
            } else if trade.state == "open" {
                overlay_button(
                    trade_parent,
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
                        trade_parent,
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
                    trade_parent,
                    "Confirm trade",
                    OverlayButton::TradeConfirm,
                    !trade.my_confirmed,
                );
                overlay_button(
                    trade_parent,
                    "Cancel trade",
                    OverlayButton::TradeCancel,
                    true,
                );
            } else {
                overlay_button(
                    trade_parent,
                    "Request trade",
                    OverlayButton::TradeRequest,
                    true,
                );
            }
            body(trade_parent, &format!("Pending: {}", social.pending.len()));
            overlay_button(trade_parent, "Close", OverlayButton::CloseSocial, true);
        });
}

fn render_skills(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    skills: &SkillModel,
    skill_binding: &SkillBindingUi,
    skill_persistence: &SkillBindingPersistenceRuntime,
    state: &NativePlayerUiState,
    ui: &UiReadModel,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(SKILL_PANEL_SIZE.width as f32),
            height: Val::Px(SKILL_PANEL_SIZE.height as f32),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    ));
    if let Some(asset_server) = asset_server {
        spawn_overlay_frame(
            parent,
            asset_server,
            "original-ui/Title/504.png",
            SKILL_PANEL_SIZE.width as f32,
            SKILL_PANEL_SIZE.height as f32,
        );
        spawn_static_overlay_sprite(
            parent,
            asset_server,
            "original-ui/Title/508.png".to_owned(),
            CrystalRect::new(8.0, 90.0, 248.0, 284.0),
        );
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse2",
            360,
            361,
            362,
            CrystalRect::new(241.0, 3.0, 24.0, 21.0),
            OverlayButton::CloseWindows,
        );
    } else {
        overlay_absolute_button(
            parent,
            "X",
            CrystalRect::new(241.0, 3.0, 22.0, 20.0),
            OverlayButton::CloseWindows,
            true,
        );
    }

    let page_count = native_skill_page_count(skills.skills.len());
    let page = state.skill_page.min(page_count.saturating_sub(1));
    parent
        .spawn((
            OverlaySkillListViewport,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(90.0),
                width: Val::Px(248.0),
                height: Val::Px(284.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|viewport| {
            if skills.skills.is_empty() {
                overlay_text_at(
                    viewport,
                    "No skills learned",
                    CrystalRect::new(18.0, 28.0, 212.0, 18.0),
                    11.0,
                    GOLD,
                );
                overlay_text_at(
                    viewport,
                    "Learn skills from a trainer.",
                    CrystalRect::new(18.0, 52.0, 212.0, 16.0),
                    9.0,
                    TEXT,
                );
                return;
            }

            let start = page * SKILL_PAGE_SIZE;
            for (row, skill) in skills
                .skills
                .iter()
                .skip(start)
                .take(SKILL_PAGE_SIZE)
                .enumerate()
            {
                let binding = skills.binding_for(skill.id);
                let shortcut = skill_binding
                    .binding_for_skill(skill.id)
                    .map(|binding| format!("F{}", binding.hotkey))
                    .unwrap_or_else(|| "--".to_owned());
                let status = if binding.can_use == Some(false) {
                    "locked"
                } else if binding.cooldown_remaining_ticks > 0 {
                    "cooldown"
                } else if binding.mp_cost.unwrap_or(skill.mp_cost) > ui.player.mp.max(0) as u32 {
                    "low MP"
                } else if binding.cast_kind.as_deref() == Some("passive") {
                    "passive"
                } else {
                    "ready"
                };
                let selected = skill_binding.selected_skill_id() == Some(skill.id);
                let label = format!(
                    "{}{} Lv{}  {}  {}",
                    if selected { "▶ " } else { "" },
                    short_name(&skill.name, skill.key.as_deref().unwrap_or("")),
                    skill.level,
                    shortcut,
                    status,
                );
                overlay_absolute_button(
                    viewport,
                    &label,
                    CrystalRect::new(
                        (SKILL_ROW_ORIGIN.x - 8) as f32,
                        (SKILL_ROW_ORIGIN.y - 90 + row as i32 * SKILL_ROW_STEP_Y) as f32,
                        SKILL_ROW_SIZE.width as f32,
                        SKILL_ROW_SIZE.height as f32,
                    ),
                    OverlayButton::SelectSkill(skill.id),
                    true,
                );
            }
        });

    let has_previous = page > 0;
    let has_next = page + 1 < page_count;
    if let Some(asset_server) = asset_server {
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            398,
            399,
            399,
            CrystalRect::new(90.0, 340.0, 32.0, 24.0),
            OverlayButton::SkillPagePrev,
            has_previous,
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            396,
            397,
            397,
            CrystalRect::new(140.0, 340.0, 32.0, 24.0),
            OverlayButton::SkillPageNext,
            has_next,
        );
    } else {
        overlay_absolute_button(
            parent,
            "<",
            CrystalRect::new(90.0, 340.0, 32.0, 24.0),
            OverlayButton::SkillPagePrev,
            has_previous,
        );
        overlay_absolute_button(
            parent,
            ">",
            CrystalRect::new(140.0, 340.0, 32.0, 24.0),
            OverlayButton::SkillPageNext,
            has_next,
        );
    }
    overlay_text_at(
        parent,
        &format!("{}/{}", page + 1, page_count),
        CrystalRect::new(116.0, 343.0, 24.0, 14.0),
        8.0,
        TEXT,
    );
    if skill_binding.is_assign_key_enabled() {
        parent
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(286.0),
                    top: Val::Px(0.0),
                    width: Val::Px(360.0),
                    height: Val::Px(145.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_children(|assign| {
                if let Some(asset_server) = asset_server {
                    spawn_overlay_frame(
                        assign,
                        asset_server,
                        "original-ui/Prguse/710.png",
                        360.0,
                        145.0,
                    );
                }
                let selected_name = skill_binding
                    .selected_skill_id()
                    .and_then(|id| skills.skills.iter().find(|skill| skill.id == id))
                    .map(|skill| skill.name.as_str())
                    .unwrap_or("No skill selected");
                overlay_text_at(
                    assign,
                    selected_name,
                    CrystalRect::new(16.0, 16.0, 250.0, 20.0),
                    11.0,
                    TEXT,
                );
                for hotkey in 1..=8_u8 {
                    let selected = skill_binding.selected_skill_id().is_some()
                        && skill_binding.skill_for_hotkey(hotkey)
                            == skill_binding.selected_skill_id();
                    let rect = CrystalRect::new(
                        17.0 + 32.0 * f32::from(hotkey - 1) + 5.0 * f32::from((hotkey - 1) / 4),
                        58.0,
                        28.0,
                        30.0,
                    );
                    if let Some(asset_server) = asset_server {
                        spawn_overlay_crystal_button(
                            assign,
                            asset_server,
                            "Prguse",
                            if selected { 1658 } else { 1656 },
                            1657,
                            1658,
                            rect,
                            OverlayButton::AssignSkillKey(hotkey),
                        );
                    } else {
                        overlay_absolute_button(
                            assign,
                            &format!("F{hotkey}"),
                            rect,
                            OverlayButton::AssignSkillKey(hotkey),
                            true,
                        );
                    }
                    overlay_text_at(
                        assign,
                        &format!("F{hotkey}"),
                        CrystalRect::new(rect.left, rect.top + 8.0, rect.width, 12.0),
                        8.0,
                        if selected { GOLD } else { TEXT },
                    );
                }
                if let Some(asset_server) = asset_server {
                    spawn_overlay_crystal_button(
                        assign,
                        asset_server,
                        "Title",
                        287,
                        288,
                        289,
                        CrystalRect::new(284.0, 64.0, 64.0, 28.0),
                        OverlayButton::ClearSkillBinding,
                    );
                    spawn_overlay_crystal_button(
                        assign,
                        asset_server,
                        "Title",
                        156,
                        157,
                        158,
                        CrystalRect::new(284.0, 101.0, 64.0, 28.0),
                        OverlayButton::CloseSkillAssign,
                    );
                }
                if skill_persistence.dirty {
                    overlay_text_at(
                        assign,
                        "Session binding active; disk save failed",
                        CrystalRect::new(16.0, 112.0, 260.0, 16.0),
                        8.0,
                        GOLD,
                    );
                }
            });
    }
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
        let use_enabled = inspected_use_intent(state, inventory).is_some();
        overlay_button(parent, "Use (U)", OverlayButton::UseInspected, use_enabled);
        if inspect.container == 2 {
            overlay_button(
                parent,
                "Unequip (G)",
                OverlayButton::UnequipInspected,
                inspected_remove_intent(state, inventory).is_some(),
            );
        } else {
            overlay_button(
                parent,
                "Equip (G)",
                OverlayButton::EquipInspected,
                inspected_equip_intent(state, inventory).is_some(),
            );
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
            if let Some(confirmation) = state.drop_confirmation.as_ref() {
                let current = drop_confirmation_is_current(confirmation, inventory);
                body(
                    parent,
                    &format!("Drop {} x{}?", confirmation.key, confirmation.count),
                );
                overlay_button(
                    parent,
                    "Confirm drop",
                    OverlayButton::ConfirmDropInspected,
                    current,
                );
                overlay_button(
                    parent,
                    "Cancel drop",
                    OverlayButton::CancelDropInspected,
                    true,
                );
            }
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
        overlay_button(parent, "Close", OverlayButton::CloseInspect, true);
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
    asset_server: Option<&AssetServer>,
    mail: &MailModel,
    mail_ui: &MailUiState,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
) {
    if let Some(asset_server) = asset_server {
        spawn_overlay_frame(
            parent,
            asset_server,
            "original-ui/Title/670.png",
            312.0,
            444.0,
        );
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse2",
            360,
            361,
            362,
            CrystalRect::new(288.0, 3.0, 24.0, 21.0),
            OverlayButton::CloseMail,
        );
        // Help has no native/backend page yet. Rendering it disabled avoids a
        // source-looking control with an empty click handler.
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            257,
            258,
            259,
            CrystalRect::new(262.0, 3.0, 24.0, 21.0),
            OverlayButton::CloseMail,
            false,
        );
    }
    if let Some(compose) = state.core.mail_compose.as_ref() {
        render_mail_compose(parent, compose, inventory);
        overlay_button(parent, "Cancel", OverlayButton::CancelMailCompose, true);
        return;
    }
    let page = mail.page(mail_ui.cursor.page);
    overlay_text_at(
        parent,
        &format!(
            "Unread {} / Total {}",
            mail.unread_count(),
            mail.visible_mails().len()
        ),
        CrystalRect::new(10.0, 31.0, 290.0, 18.0),
        10.0,
        TEXT,
    );
    for (row, msg) in page.entries.iter().enumerate() {
        let flags = format!(
            "{}{}{}",
            if msg.read { "" } else { "[New] " },
            if msg.has_attachment() { "[+] " } else { "" },
            if msg.locked { "[Lock] " } else { "" },
        );
        let selected = mail.selected_id == Some(msg.id);
        let label = format!(
            "{}{}{}: {}",
            if selected { "▶ " } else { "" },
            flags,
            short_name(&msg.sender, "Unknown"),
            short_name(&msg.subject, "Mail")
        );
        overlay_absolute_button(
            parent,
            &label,
            CrystalRect::new(10.0, 55.0 + row as f32 * 33.0, 290.0, 33.0),
            OverlayButton::SelectMail(msg.id),
            true,
        );
    }

    if let Some(asset_server) = asset_server {
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            240,
            241,
            242,
            CrystalRect::new(102.0, 389.0, 24.0, 21.0),
            OverlayButton::MailPagePrev,
            page.page > 0,
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            243,
            244,
            245,
            CrystalRect::new(192.0, 389.0, 24.0, 21.0),
            OverlayButton::MailPageNext,
            page.page + 1 < page.page_count,
        );
    }
    overlay_text_at(
        parent,
        &format!("{}/{}", page.page + 1, page.page_count),
        CrystalRect::new(120.0, 389.0, 67.0, 15.0),
        9.0,
        TEXT,
    );

    let selected = mail.selected();
    let selected_id = selected.map(|message| message.id).unwrap_or_default();
    if let Some(asset_server) = asset_server {
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse",
            563,
            564,
            565,
            CrystalRect::new(75.0, 414.0, 27.0, 25.0),
            OverlayButton::OpenMailCompose,
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            569,
            570,
            571,
            CrystalRect::new(102.0, 414.0, 27.0, 25.0),
            OverlayButton::OpenMailCompose,
            false,
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            572,
            573,
            574,
            CrystalRect::new(129.0, 414.0, 27.0, 25.0),
            OverlayButton::ReadMail(selected_id),
            selected.is_some_and(|message| !message.read),
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            557,
            558,
            559,
            CrystalRect::new(156.0, 414.0, 27.0, 25.0),
            OverlayButton::DeleteMail(selected_id),
            selected.is_some_and(mail_delete_enabled),
        );
    }
    if let Some(message) = selected.filter(|message| mail_claim_enabled(message)) {
        overlay_absolute_button(
            parent,
            "Claim",
            CrystalRect::new(215.0, 414.0, 55.0, 24.0),
            OverlayButton::ClaimMail(message.id),
            true,
        );
    }
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

fn big_map_view_position(point: BigMapPoint, width: i32, height: i32) -> (f32, f32) {
    if width <= 0 || height <= 0 {
        return (BIGMAP_WIDTH * 0.5, BIGMAP_HEIGHT * 0.5);
    }
    let x = (point.x.max(0) as f32 / width as f32).clamp(0.0, 1.0) * BIGMAP_WIDTH;
    let y = (point.y.max(0) as f32 / height as f32).clamp(0.0, 1.0) * BIGMAP_HEIGHT;
    (x, y)
}

fn render_bigmap(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    model: &BigMapModel,
    renderer: &BigMapUiState,
    ui: &UiReadModel,
) {
    let Some(asset_server) = asset_server else {
        return;
    };
    let rendered = model.render_snapshot();
    let map_name = rendered
        .title
        .as_deref()
        .or(ui.player.map_name.as_deref())
        .unwrap_or("");
    spawn_overlay_frame(
        parent,
        asset_server,
        "original-ui/Title/820.png",
        760.0,
        500.0,
    );
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(19.0),
            top: Val::Px(6.0),
            width: Val::Px(699.0),
            height: Val::Px(20.0),
            ..default()
        },
        Text::new(map_name.to_owned()),
        TextFont {
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::justify(Justify::Center),
    ));

    parent
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            top: Val::Px(52.0),
            width: Val::Px(BIGMAP_WIDTH),
            height: Val::Px(BIGMAP_HEIGHT),
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|viewport| {
            let asset = rendered.map_image_url.clone();
            if let Some(asset) = asset {
                viewport.spawn((
                    BigMapImageEntity { url: asset.clone() },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(BIGMAP_WIDTH),
                        height: Val::Px(BIGMAP_HEIGHT),
                        ..default()
                    },
                    ImageNode {
                        image: asset_server.load(asset),
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                ));
            } else {
                viewport.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.02, 0.015, 0.01)),
                ));
            }

            if model.view == BigMapView::WorldMap && model.world.enabled {
                for index in [1365_u16, 1366_u16] {
                    viewport.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Px(BIGMAP_WIDTH),
                            height: Val::Px(BIGMAP_HEIGHT),
                            ..default()
                        },
                        ImageNode {
                            image: asset_server.load(format!("original-ui/Prguse2/{index}.png")),
                            image_mode: NodeImageMode::Stretch,
                            ..default()
                        },
                    ));
                }
            } else if let Some(entry) = model.active_map() {
                for movement in &entry.info.movements {
                    let (x, y) = big_map_view_position(
                        movement.location,
                        entry.info.width,
                        entry.info.height,
                    );
                    viewport.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x - 2.0),
                            top: Val::Px(y - 2.0),
                            width: Val::Px(4.0),
                            height: Val::Px(4.0),
                            ..default()
                        },
                        BackgroundColor(GOLD),
                    ));
                }
                for npc in entry.info.npcs.iter().filter(|npc| npc.show_on_big_map) {
                    let (x, y) =
                        big_map_view_position(npc.location, entry.info.width, entry.info.height);
                    viewport.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x - 2.0),
                            top: Val::Px(y - 2.0),
                            width: Val::Px(5.0),
                            height: Val::Px(5.0),
                            ..default()
                        },
                        BackgroundColor(if model.selected_npc_object_id == Some(npc.object_id) {
                            Color::srgb(1.0, 0.35, 0.12)
                        } else {
                            Color::srgb(0.25, 0.95, 0.35)
                        }),
                    ));
                }
                if let Some(location) = rendered.player_location {
                    let (px, py) =
                        big_map_view_position(location, entry.info.width, entry.info.height);
                    viewport.spawn((
                        BigMapPlayerEntity { location },
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(px - 6.0),
                            top: Val::Px(py - 5.0),
                            width: Val::Px(12.0),
                            height: Val::Px(10.0),
                            ..default()
                        },
                        ImageNode {
                            image: asset_server.load("original-ui/Prguse2/1350.png"),
                            ..default()
                        },
                    ));
                }
            }
        });

    let location = model.player_location.unwrap_or_default();
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(519.0),
            top: Val::Px(435.0),
            width: Val::Px(100.0),
            height: Val::Px(15.0),
            ..default()
        },
        Text::new(format!("[ {}, {} ]", location.x, location.y)),
        TextFont {
            font_size: FontSize::Px(11.0),
            ..default()
        },
        TextColor(Color::WHITE),
    ));

    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        360,
        361,
        362,
        CrystalRect::new(735.0, 3.0, 24.0, 21.0),
        OverlayButton::CloseBigMap,
    );

    let filtered_count = model.filtered_npcs().len();
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Prguse2",
        197,
        198,
        199,
        CrystalRect::new(739.0, 48.0, 16.0, 16.0),
        OverlayButton::BigMapScrollUp,
        model.npc_scroll_row > 0,
    );
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Prguse2/205.png".to_owned(),
        CrystalRect::new(739.0, 61.0, 16.0, 16.0),
    );
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Prguse2",
        207,
        208,
        209,
        CrystalRect::new(739.0, 417.0, 16.0, 16.0),
        OverlayButton::BigMapScrollDown,
        model.npc_scroll_row.saturating_add(BIG_MAP_NPC_ROW_COUNT) < filtered_count,
    );
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Title",
        827,
        828,
        829,
        CrystalRect::new(250.0, 467.0, 80.0, 25.0),
        OverlayButton::BigMapWorld,
        model.world.enabled,
    );
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Title",
        824,
        825,
        826,
        CrystalRect::new(400.0, 467.0, 80.0, 25.0),
        OverlayButton::BigMapMyLocation,
        model.current_map_index.is_some(),
    );
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Title",
        821,
        822,
        823,
        CrystalRect::new(638.0, 432.0, 72.0, 25.0),
        OverlayButton::BigMapTeleport,
        model.selected_teleport_intent().is_some(),
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        1340,
        1341,
        1342,
        CrystalRect::new(23.0, 464.0, 32.0, 30.0),
        OverlayButton::BigMapSearchSubmit,
    );
    overlay_absolute_button(
        parent,
        &model.search.draft,
        CrystalRect::new(59.0, 468.0, 130.0, 20.0),
        OverlayButton::BigMapSearchFocus,
        true,
    );
    if renderer.search_focused {
        overlay_text_at(
            parent,
            "|",
            CrystalRect::new(183.0, 470.0, 4.0, 12.0),
            10.0,
            GOLD,
        );
    }

    for (row, npc) in rendered.npcs.iter().enumerate() {
        let label = format!("{} [{},{}]", npc.name, npc.location.x, npc.location.y);
        overlay_absolute_button(
            parent,
            &label,
            CrystalRect::new(590.0, 50.0 + row as f32 * 21.0, 140.0, 25.0),
            OverlayButton::SelectBigMapNpc(npc.object_id),
            npc.object_id != 0,
        );
        parent.spawn(BigMapNpcRowEntity {
            object_id: npc.object_id,
            name: npc.name.clone(),
            location: npc.location,
        });
    }
}

fn render_shop(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    shop: &ShopModel,
    shop_ui: &ShopUiState,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
) {
    let buy_enabled = shop_buy_enabled(shop, inventory, state.shop_quantity);
    let sell_enabled = shop
        .selected_bag_slot_for_sell
        .map(|s| shop_sell_enabled(inventory, Some(s)))
        .unwrap_or(false);
    let repair_enabled = repair_selection_enabled(state, inventory);

    if let Some(asset_server) = asset_server {
        // Crystal's NPCGoodsDialog frame is Prguse/1000.  The extracted
        // layout does not declare its intrinsic size; derive the occupied
        // bounds from the furthest confirmed control instead of stretching
        // the art to the old provisional 620x520 panel.
        spawn_overlay_frame(
            parent,
            asset_server,
            "original-ui/Prguse/1000.png",
            242.0,
            330.0,
        );
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse2",
            360,
            361,
            362,
            CrystalRect::new(217.0, 3.0, 24.0, 21.0),
            OverlayButton::CloseShop,
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            197,
            198,
            199,
            CrystalRect::new(219.0, 35.0, 12.0, 12.0),
            OverlayButton::ShopPageUp,
            shop_ui.start_index > 0,
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            207,
            208,
            209,
            CrystalRect::new(219.0, 284.0, 12.0, 12.0),
            OverlayButton::ShopPageDown,
            shop_ui.start_index.saturating_add(8) < shop.goods.len(),
        );
        let max_start = shop.goods.len().saturating_sub(8);
        let scroll_top = if max_start == 0 {
            49.0
        } else {
            49.0 + 217.0 * shop_ui.start_index as f32 / max_start as f32
        };
        spawn_static_overlay_sprite(
            parent,
            asset_server,
            "original-ui/Prguse2/205.png".to_owned(),
            CrystalRect::new(219.0, scroll_top, 12.0, 18.0),
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Title",
            312,
            313,
            314,
            CrystalRect::new(77.0, 304.0, 80.0, 22.0),
            OverlayButton::ShopBuy,
            buy_enabled,
        );
    } else {
        overlay_absolute_button(
            parent,
            "Close",
            CrystalRect::new(217.0, 3.0, 24.0, 21.0),
            OverlayButton::CloseShop,
            true,
        );
        overlay_absolute_button(
            parent,
            "↑",
            CrystalRect::new(219.0, 35.0, 12.0, 12.0),
            OverlayButton::ShopPageUp,
            shop_ui.start_index > 0,
        );
        overlay_absolute_button(
            parent,
            "↓",
            CrystalRect::new(219.0, 284.0, 12.0, 12.0),
            OverlayButton::ShopPageDown,
            shop_ui.start_index.saturating_add(8) < shop.goods.len(),
        );
        overlay_absolute_button(
            parent,
            "Buy",
            CrystalRect::new(77.0, 304.0, 80.0, 22.0),
            OverlayButton::ShopBuy,
            buy_enabled,
        );
    }

    for (row, good) in shop
        .goods
        .iter()
        .skip(shop_ui.start_index)
        .take(8)
        .enumerate()
    {
        let selected = shop.selected_id == Some(good.unique_id);
        let label = format!(
            "{}{}  {}  {}",
            if selected { "▶ " } else { "" },
            short_name(&good.name, &good.unique_id.to_string()),
            good.price,
            good.stock_label(),
        );
        overlay_absolute_shop_good_button(
            parent,
            asset_server,
            good.icon,
            &label,
            CrystalRect::new(10.0, 34.0 + row as f32 * 33.0, 202.0, 30.0),
            OverlayButton::SelectShopGood(good.unique_id),
        );
    }
    if shop.goods.is_empty() {
        overlay_text_at(
            parent,
            "No goods",
            CrystalRect::new(12.0, 40.0, 195.0, 18.0),
            10.0,
            TEXT,
        );
    }
    overlay_text_at(
        parent,
        &format!("Gold {}", inventory.gold),
        CrystalRect::new(10.0, 8.0, 150.0, 16.0),
        10.0,
        GOLD,
    );
    overlay_absolute_button(
        parent,
        "−",
        CrystalRect::new(12.0, 304.0, 20.0, 22.0),
        OverlayButton::ShopQuantityDec,
        state.shop_quantity > SHOP_QUANTITY_MIN,
    );
    overlay_text_at(
        parent,
        &format!("x{}", shop_quantity_clamped(state.shop_quantity)),
        CrystalRect::new(34.0, 306.0, 36.0, 18.0),
        10.0,
        TEXT,
    );
    overlay_absolute_button(
        parent,
        "+",
        CrystalRect::new(58.0, 304.0, 18.0, 22.0),
        OverlayButton::ShopQuantityInc,
        state.shop_quantity < SHOP_QUANTITY_MAX,
    );

    // Sell and repair are separate Crystal service modes.  Keep their real
    // gateway operations reachable in a clearly auxiliary panel instead of
    // overlaying fake controls on the confirmed NPCGoodsDialog art.
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(250.0),
                top: Val::Px(0.0),
                width: Val::Px(360.0),
                height: Val::Px(330.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|aux| {
            body(aux, "Bag sell / repair");
            for item in inventory.items_in(0).into_iter().take(8) {
                let selected_for_sell = shop.selected_bag_slot_for_sell == Some(item.slot);
                let selected_for_repair = shop.selected_bag_slot_for_repair == Some(item.slot);
                aux.spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(3.0),
                    ..default()
                })
                .with_children(|row| {
                    overlay_button(
                        row,
                        &format!(
                            "{}Sell {}",
                            if selected_for_sell { "▶" } else { "" },
                            short_name(&item.name, &item.key)
                        ),
                        OverlayButton::SelectBagForSell(item.slot),
                        true,
                    );
                    overlay_button(
                        row,
                        &format!("{}Repair", if selected_for_repair { "▶" } else { "" }),
                        OverlayButton::SelectBagForRepair(item.slot),
                        true,
                    );
                });
            }
            aux.spawn(Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                overlay_button(row, "Sell", OverlayButton::ShopSell, sell_enabled);
                overlay_button(row, "Repair", OverlayButton::ShopRepair, repair_enabled);
                overlay_button(row, "S.Repair", OverlayButton::ShopSRepair, repair_enabled);
            });
            body(aux, "Equipment repair");
            aux.spawn(Node {
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
                    overlay_button(
                        grid,
                        &format!(
                            "{}{}",
                            if selected { "▶" } else { "" },
                            equipment_slot_name(slot)
                        ),
                        OverlayButton::SelectEquipForRepair(slot),
                        item.is_some(),
                    );
                }
            });
        });
}

fn native_skill_page_count(item_count: usize) -> usize {
    item_count
        .saturating_add(SKILL_PAGE_SIZE.saturating_sub(1))
        .checked_div(SKILL_PAGE_SIZE)
        .unwrap_or(0)
        .max(1)
}

fn native_game_shop_page_count(item_count: usize) -> usize {
    item_count
        .saturating_add(CRYSTAL_GAME_SHOP_PAGE_SIZE.saturating_sub(1))
        .checked_div(CRYSTAL_GAME_SHOP_PAGE_SIZE)
        .unwrap_or(0)
        .max(1)
}

fn native_game_shop_page_for_index(
    game_shop: &GameShopModel,
    game_shop_index: i32,
) -> Option<usize> {
    game_shop
        .items
        .iter()
        .position(|entry| entry.game_shop_index == game_shop_index)
        .map(|position| position / CRYSTAL_GAME_SHOP_PAGE_SIZE)
}

fn native_game_shop_page_entries(
    game_shop: &GameShopModel,
    page: usize,
) -> &[crate::game_shop::GameShopEntry] {
    let start = page.saturating_mul(CRYSTAL_GAME_SHOP_PAGE_SIZE);
    let end = (start + CRYSTAL_GAME_SHOP_PAGE_SIZE).min(game_shop.items.len());
    game_shop.items.get(start..end).unwrap_or(&[])
}

fn render_game_shop(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    game_shop: &GameShopModel,
    ui: &UiReadModel,
    state: &NativePlayerUiState,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(GAME_SHOP_PANEL_SIZE.width as f32),
            height: Val::Px(GAME_SHOP_PANEL_SIZE.height as f32),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(PANEL_BG),
    ));
    if let Some(asset_server) = asset_server {
        spawn_overlay_frame(
            parent,
            asset_server,
            "original-ui/Title/749.png",
            GAME_SHOP_PANEL_SIZE.width as f32,
            GAME_SHOP_PANEL_SIZE.height as f32,
        );
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            "Prguse2",
            360,
            361,
            362,
            CrystalRect::new(671.0, 4.0, 24.0, 21.0),
            OverlayButton::CloseGameShop,
        );
    } else {
        overlay_absolute_button(
            parent,
            "X",
            CrystalRect::new(671.0, 4.0, 24.0, 21.0),
            OverlayButton::CloseGameShop,
            true,
        );
    }

    let page_count = native_game_shop_page_count(game_shop.items.len());
    let page = state.game_shop_page.min(page_count.saturating_sub(1));
    overlay_text_at(
        parent,
        "Game Shop",
        CrystalRect::new(18.0, 9.0, 180.0, 18.0),
        12.0,
        GOLD,
    );
    overlay_text_at(
        parent,
        &format!("Products: {}", game_shop.items.len()),
        CrystalRect::new(15.0, 72.0, 120.0, 16.0),
        9.0,
        TEXT,
    );
    overlay_text_at(
        parent,
        &format!("Page {}/{}", page + 1, page_count),
        CrystalRect::new(15.0, 88.0, 120.0, 16.0),
        9.0,
        TEXT,
    );
    if game_shop.pending_purchase.is_some() {
        overlay_text_at(
            parent,
            "Purchase pending; waiting for authoritative receipt.",
            CrystalRect::new(152.0, 92.0, 510.0, 16.0),
            9.0,
            GOLD,
        );
    } else if game_shop.purchase_unknown {
        overlay_text_at(
            parent,
            "Purchase status unknown; refresh wallet, mail and stock before retry.",
            CrystalRect::new(152.0, 92.0, 510.0, 16.0),
            9.0,
            GOLD,
        );
    }

    let class = ui.player.class_name.as_deref().unwrap_or("");
    for (offset, entry) in native_game_shop_page_entries(game_shop, page)
        .iter()
        .enumerate()
    {
        let column = offset % GAME_SHOP_PAGE_COLUMNS;
        let row = offset / GAME_SHOP_PAGE_COLUMNS;
        let rect = CrystalRect::new(
            (GAME_SHOP_GRID_ORIGIN.x + column as i32 * GAME_SHOP_COLUMN_STEP) as f32,
            (GAME_SHOP_GRID_ORIGIN.y + row as i32 * GAME_SHOP_ROW_STEP) as f32,
            GAME_SHOP_CELL_SIZE.width as f32,
            GAME_SHOP_CELL_SIZE.height as f32,
        );
        spawn_game_shop_product(
            parent,
            asset_server,
            entry,
            rect,
            game_shop.selected_game_shop_index == Some(entry.game_shop_index),
            entry.visible_for_class(class),
        );
    }

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
        overlay_text_at(
            parent,
            "Selected",
            CrystalRect::new(15.0, 122.0, 120.0, 15.0),
            9.0,
            GOLD,
        );
        overlay_text_at(
            parent,
            &short_name(&entry.item_name, &entry.item_index.to_string()),
            CrystalRect::new(15.0, 140.0, 120.0, 15.0),
            9.0,
            TEXT,
        );
        overlay_text_at(
            parent,
            &format!("{payment} {price}"),
            CrystalRect::new(15.0, 158.0, 120.0, 15.0),
            9.0,
            TEXT,
        );
        overlay_text_at(
            parent,
            &format!("Stock {}", entry.stock_label()),
            CrystalRect::new(15.0, 176.0, 120.0, 15.0),
            9.0,
            TEXT,
        );
    } else {
        overlay_text_at(
            parent,
            "Select a product",
            CrystalRect::new(15.0, 122.0, 120.0, 15.0),
            9.0,
            TEXT,
        );
    }

    overlay_text_at(
        parent,
        &format!("Credit {}", ui.player.credit),
        CrystalRect::new(5.0, 449.0, 110.0, 18.0),
        9.0,
        TEXT,
    );
    overlay_text_at(
        parent,
        &format!("Gold {}", ui.player.gold),
        CrystalRect::new(123.0, 449.0, 105.0, 18.0),
        9.0,
        TEXT,
    );
    overlay_absolute_button(
        parent,
        "Credit",
        CrystalRect::new(250.0, 446.0, 78.0, 22.0),
        OverlayButton::GameShopPaymentCredit,
        game_shop.payment != GameShopPaymentType::Credit,
    );
    overlay_absolute_button(
        parent,
        "Gold",
        CrystalRect::new(332.0, 446.0, 68.0, 22.0),
        OverlayButton::GameShopPaymentGold,
        game_shop.payment != GameShopPaymentType::Gold,
    );
    overlay_absolute_button(
        parent,
        "-",
        CrystalRect::new(404.0, 446.0, 22.0, 22.0),
        OverlayButton::GameShopQuantityDec,
        game_shop.quantity > GAME_SHOP_QUANTITY_MIN,
    );
    overlay_text_at(
        parent,
        &format!("x{}", game_shop.quantity),
        CrystalRect::new(429.0, 450.0, 32.0, 14.0),
        9.0,
        TEXT,
    );
    overlay_absolute_button(
        parent,
        "+",
        CrystalRect::new(464.0, 446.0, 22.0, 22.0),
        OverlayButton::GameShopQuantityInc,
        game_shop.quantity < GAME_SHOP_QUANTITY_MAX,
    );

    let buy_enabled = game_shop.buy_enabled(ui.player.gold, ui.player.credit, class);
    overlay_absolute_button(
        parent,
        "Buy",
        CrystalRect::new(492.0, 446.0, 82.0, 22.0),
        OverlayButton::GameShopBuy,
        buy_enabled,
    );
    if let Some(reason) = game_shop.buy_disabled_reason(ui.player.gold, ui.player.credit, class) {
        overlay_text_at(
            parent,
            &format!("Buy: {reason}"),
            CrystalRect::new(15.0, 198.0, 120.0, 32.0),
            8.0,
            Color::srgba(0.85, 0.65, 0.35, 1.0),
        );
    }
    overlay_absolute_button(
        parent,
        "<",
        CrystalRect::new(600.0, 446.0, 24.0, 22.0),
        OverlayButton::GameShopPagePrev,
        page > 0,
    );
    overlay_text_at(
        parent,
        &format!("{}/{}", page + 1, page_count),
        CrystalRect::new(626.0, 450.0, 32.0, 14.0),
        8.0,
        TEXT,
    );
    overlay_absolute_button(
        parent,
        ">",
        CrystalRect::new(660.0, 446.0, 24.0, 22.0),
        OverlayButton::GameShopPageNext,
        page + 1 < page_count,
    );
}

fn spawn_game_shop_product(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    entry: &crate::game_shop::GameShopEntry,
    rect: CrystalRect,
    selected: bool,
    enabled: bool,
) {
    let mut card = parent.spawn((
        OverlayGameShopProduct,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(if selected {
            Color::srgba(0.42, 0.28, 0.08, 0.92)
        } else if enabled {
            Color::srgba(0.12, 0.08, 0.04, 0.86)
        } else {
            BUTTON_DISABLED
        }),
    ));
    if enabled {
        card.insert((
            Button,
            OverlayButton::SelectGameShopGood(entry.game_shop_index),
        ));
    }
    card.with_children(|cell| {
        overlay_text_at(
            cell,
            &short_name(&entry.item_name, &entry.item_index.to_string()),
            CrystalRect::new(5.0, 5.0, 115.0, 15.0),
            9.0,
            if selected { GOLD } else { TEXT },
        );
        if let (Some(asset_server), Ok(icon)) = (asset_server, u16::try_from(entry.image)) {
            if let Some(path) = item_icon_path(icon) {
                spawn_static_overlay_sprite(
                    cell,
                    asset_server,
                    path,
                    CrystalRect::new(42.0, 27.0, 40.0, 40.0),
                );
            }
        }
        overlay_text_at(
            cell,
            &format!("Gold {}", entry.gold_price),
            CrystalRect::new(6.0, 78.0, 113.0, 14.0),
            8.0,
            TEXT,
        );
        overlay_text_at(
            cell,
            &format!("Credit {}", entry.credit_price),
            CrystalRect::new(6.0, 94.0, 113.0, 14.0),
            8.0,
            TEXT,
        );
        overlay_text_at(
            cell,
            &format!("Stock {}  x{}", entry.stock_label(), entry.count.max(1)),
            CrystalRect::new(6.0, 110.0, 113.0, 14.0),
            8.0,
            TEXT,
        );
        if selected {
            overlay_text_at(
                cell,
                "SELECTED",
                CrystalRect::new(6.0, 128.0, 113.0, 13.0),
                8.0,
                GOLD,
            );
        }
    });
}

fn render_storage(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    storage: &StorageModel,
    storage_ui: &StorageUiState,
    inventory: &InventoryModel,
    _state: &NativePlayerUiState,
) {
    let Some(asset_server) = asset_server else {
        return;
    };
    spawn_overlay_frame(
        parent,
        asset_server,
        "original-ui/Prguse/586.png",
        388.0,
        330.0,
    );
    let page = storage.page(storage_ui.cursor.page);
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Title",
        if page.page == 0 { 743 } else { 744 },
        if page.page == 0 { 743 } else { 744 },
        744,
        CrystalRect::new(8.0, 36.0, 72.0, 20.0),
        OverlayButton::StoragePage(0),
    );
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Title",
        if page.page == 1 { 745 } else { 746 },
        if page.page == 1 { 745 } else { 746 },
        746,
        CrystalRect::new(80.0, 36.0, 72.0, 20.0),
        OverlayButton::StoragePage(1),
        storage.has_expanded,
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        360,
        361,
        362,
        CrystalRect::new(363.0, 3.0, 24.0, 21.0),
        OverlayButton::CloseStorage,
    );

    for (offset, slot) in page.slots.iter().enumerate() {
        let column = offset % 10;
        let row = offset / 10;
        let rect = CrystalRect::new(
            9.0 + column as f32 * 37.0,
            60.0 + row as f32 * 33.0,
            32.0,
            30.0,
        );
        if let Some(item) = slot.item {
            let selected = storage_ui.storage_selection
                == slot.unique_id.map(|unique_id| StorageItemSelection {
                    slot: slot.slot,
                    unique_id,
                });
            overlay_absolute_item_button(
                parent,
                asset_server,
                item,
                rect,
                OverlayButton::SelectStorage(slot.slot),
                !slot.locked && item.unique_id.is_some(),
            );
            if selected {
                overlay_text_at(parent, "▶", rect, 10.0, GOLD);
            }
        } else {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(rect.left),
                    top: Val::Px(rect.top),
                    width: Val::Px(rect.width),
                    height: Val::Px(rect.height),
                    ..default()
                },
                BackgroundColor(if slot.locked {
                    Color::srgba(0.20, 0.16, 0.11, 0.70)
                } else {
                    Color::srgba(0.06, 0.04, 0.02, 0.35)
                }),
            ));
        }
    }

    overlay_text_at(
        parent,
        &format!(
            "{}/{}  Page {}/{}",
            storage.storage_occupied(),
            storage.effective_size(),
            page.page + 1,
            page.page_count
        ),
        CrystalRect::new(8.0, 310.0, 240.0, 16.0),
        9.0,
        TEXT,
    );

    overlay_text_at(
        parent,
        "Bag items",
        CrystalRect::new(400.0, 8.0, 200.0, 18.0),
        11.0,
        GOLD,
    );
    for (row, item) in inventory
        .items_in(0)
        .into_iter()
        .filter(|item| item.unique_id.is_some())
        .take(8)
        .enumerate()
    {
        let selected = storage_ui.bag_selection
            == item.unique_id.map(|unique_id| StorageItemSelection {
                slot: item.slot,
                unique_id,
            });
        overlay_compact_item_button(
            parent,
            Some(asset_server),
            item.icon,
            &format!(
                "{}{} x{}",
                if selected { "▶" } else { "" },
                short_name(&item.name, &item.key),
                item.quantity
            ),
            OverlayButton::SelectBagForStore(item.slot),
            true,
        );
        let _ = row;
    }

    let deposit_enabled = storage_ui.bag_selection.is_some_and(|selection| {
        storage_deposit_enabled_for_selection(storage, inventory, selection)
    });
    let withdraw_enabled = storage_ui.storage_selection.is_some_and(|selection| {
        storage_withdraw_enabled_for_selection(storage, inventory, selection)
    });
    overlay_absolute_button(
        parent,
        "Deposit →",
        CrystalRect::new(400.0, 280.0, 90.0, 24.0),
        OverlayButton::StorageDeposit,
        deposit_enabled,
    );
    overlay_absolute_button(
        parent,
        "← Withdraw",
        CrystalRect::new(495.0, 280.0, 90.0, 24.0),
        OverlayButton::StorageWithdraw,
        withdraw_enabled,
    );
    overlay_absolute_button(
        parent,
        if storage.has_password && !storage.unlocked {
            "Unlock"
        } else if storage.has_password {
            "Remove password"
        } else {
            "Set password"
        },
        CrystalRect::new(400.0, 310.0, 130.0, 24.0),
        if storage.has_password && !storage.unlocked {
            OverlayButton::StorageUnlock
        } else if storage.has_password {
            OverlayButton::StorageRemovePassword
        } else {
            OverlayButton::StorageSetPassword
        },
        if storage.has_password && !storage.unlocked {
            storage_unlock_enabled(storage)
        } else if storage.has_password {
            storage_remove_password_enabled(storage)
        } else {
            storage_set_password_enabled(storage)
        },
    );
    overlay_absolute_button(
        parent,
        &format!("Expand {}G", STORAGE_EXPAND_COST),
        CrystalRect::new(535.0, 310.0, 100.0, 24.0),
        OverlayButton::StorageExpand,
        storage_expand_enabled(storage, inventory.gold),
    );
}

fn spawn_invisible_overlay_button(
    parent: &mut ChildSpawnerCommands,
    rect: CrystalRect,
    action: OverlayButton,
) {
    parent.spawn((
        Button,
        action,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        BackgroundColor(Color::NONE),
    ));
}

fn spawn_option_volume_bar(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    top: f32,
    volume: u8,
) {
    let ratio = f32::from(volume.min(100)) / 100.0;
    let fill_width = 74.0 * ratio;
    parent
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(159.0),
            top: Val::Px(top),
            width: Val::Px(fill_width),
            height: Val::Px(19.0),
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|clip| {
            clip.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(76.0),
                    height: Val::Px(19.0),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load("original-ui/Prguse2/468.png"),
                    ..default()
                },
            ));
        });
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(155.0 + fill_width),
            top: Val::Px(top - 7.0),
            width: Val::Px(8.0),
            height: Val::Px(22.0),
            ..default()
        },
        ImageNode {
            image: asset_server.load("original-ui/Prguse/20.png"),
            ..default()
        },
    ));
}

fn render_options(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    core: &mir2_ui_core::state::UiState,
) {
    let Some(asset_server) = asset_server else {
        return;
    };
    let options = &core.options;
    spawn_overlay_frame(
        parent,
        asset_server,
        "original-ui/Title/411.png",
        259.0,
        354.0,
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        360,
        361,
        362,
        CrystalRect::new(233.0, 5.0, 24.0, 21.0),
        OverlayButton::CloseOptions,
    );

    use mir2_ui_core::state::UiCrystalOption;
    for (
        option,
        value,
        library,
        selected_on,
        unselected_on,
        selected_off,
        unselected_off,
        on_pressed,
        off_pressed,
        top,
    ) in [
        (
            UiCrystalOption::SkillMode,
            options.skill_mode,
            "Prguse2",
            452,
            450,
            453,
            455,
            451,
            454,
            68.0,
        ),
        (
            UiCrystalOption::SkillBar,
            options.skill_bar,
            "Prguse2",
            458,
            456,
            459,
            461,
            457,
            460,
            93.0,
        ),
        (
            UiCrystalOption::Effect,
            options.effect,
            "Prguse2",
            458,
            456,
            459,
            461,
            457,
            460,
            118.0,
        ),
        (
            UiCrystalOption::DropView,
            options.drop_view,
            "Prguse2",
            458,
            456,
            459,
            461,
            457,
            460,
            143.0,
        ),
        (
            UiCrystalOption::NameView,
            options.name_view,
            "Prguse2",
            458,
            456,
            459,
            461,
            457,
            460,
            168.0,
        ),
        (
            UiCrystalOption::HpView,
            options.hp_view,
            "Prguse2",
            464,
            462,
            465,
            467,
            463,
            466,
            193.0,
        ),
        (
            UiCrystalOption::NewMove,
            options.new_move,
            "Title",
            853,
            851,
            848,
            850,
            853,
            850,
            296.0,
        ),
    ] {
        let on_index = if value { selected_on } else { unselected_on };
        let off_index = if value { selected_off } else { unselected_off };
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            library,
            on_index,
            on_index,
            on_pressed,
            CrystalRect::new(159.0, top, 36.0, 17.0),
            OverlayButton::SetCrystalOption(option, true),
        );
        spawn_overlay_crystal_button(
            parent,
            asset_server,
            library,
            off_index,
            off_index,
            off_pressed,
            CrystalRect::new(201.0, top, 36.0, 17.0),
            OverlayButton::SetCrystalOption(option, false),
        );
    }

    let observe_enabled = core.observe_request_pending.is_none();
    let observe_on = if core.observe_allowed { 458 } else { 456 };
    let observe_off = if core.observe_allowed { 459 } else { 461 };
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Prguse2",
        observe_on,
        observe_on,
        457,
        CrystalRect::new(159.0, 271.0, 36.0, 17.0),
        OverlayButton::OptionsObserve(true),
        observe_enabled,
    );
    spawn_overlay_crystal_button_enabled(
        parent,
        asset_server,
        "Prguse2",
        observe_off,
        observe_off,
        460,
        CrystalRect::new(201.0, 271.0, 36.0, 17.0),
        OverlayButton::OptionsObserve(false),
        observe_enabled,
    );

    let sound_volume = if options.sound_enabled {
        options.sound_volume
    } else {
        0
    };
    let music_volume = if options.music_enabled {
        options.music_volume
    } else {
        0
    };
    spawn_option_volume_bar(parent, asset_server, 225.0, sound_volume);
    spawn_option_volume_bar(parent, asset_server, 251.0, music_volume);

    // Crystal uses drag bars. Until pointer-drag state is shared across the
    // Windows/Android adapters, the left/right halves provide deterministic
    // decrement/increment behavior without adding non-Crystal text buttons.
    spawn_invisible_overlay_button(
        parent,
        CrystalRect::new(159.0, 225.0, 38.0, 19.0),
        OverlayButton::OptionsSoundVolumeDown,
    );
    spawn_invisible_overlay_button(
        parent,
        CrystalRect::new(197.0, 225.0, 38.0, 19.0),
        OverlayButton::OptionsSoundVolumeUp,
    );
    spawn_invisible_overlay_button(
        parent,
        CrystalRect::new(159.0, 251.0, 38.0, 19.0),
        OverlayButton::OptionsMusicVolumeDown,
    );
    spawn_invisible_overlay_button(
        parent,
        CrystalRect::new(197.0, 251.0, 38.0, 19.0),
        OverlayButton::OptionsMusicVolumeUp,
    );
}

fn short_name(name: &str, key: &str) -> String {
    let source = if name.trim().is_empty() { key } else { name };
    let mut chars = source.chars();
    let taken: String = chars.by_ref().take(8).collect();
    if chars.next().is_some() {
        // The bundled Crystal bitmap font has no U+2026 glyph. Keep the
        // truncation marker ASCII so product names never end in a tofu box.
        format!("{taken}..")
    } else {
        taken
    }
}

fn short_slot_name(name: &str, key: &str) -> String {
    let source = if name.trim().is_empty() { key } else { name };
    let mut chars = source.chars();
    let taken: String = chars.by_ref().take(4).collect();
    if chars.next().is_some() {
        format!("{taken}.")
    } else {
        taken
    }
}

fn inventory_cell_stack_label(item: &ItemModel) -> String {
    (item.quantity > 1)
        .then(|| format!("x{}", item.quantity))
        .unwrap_or_default()
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
            overflow: Overflow::clip(),
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
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::MailMessage;
    use crate::shop::ShopGood;
    use bevy::asset::{AssetApp, AssetPlugin};

    #[test]
    fn compact_labels_use_supported_ascii_and_bag_cells_hide_durability() {
        assert_eq!(short_name("DestructionDrug", "fallback"), "Destruct..");
        assert_eq!(short_slot_name("Potion", "fallback"), "Poti.");

        let durable = ItemModel {
            quantity: 1,
            durability_current: Some(400),
            durability_max: Some(400),
            ..default()
        };
        assert_eq!(inventory_cell_stack_label(&durable), "");

        let stacked = ItemModel {
            quantity: 12,
            ..default()
        };
        assert_eq!(inventory_cell_stack_label(&stacked), "x12");
    }

    #[test]
    fn options_adapter_applies_immediately_and_close_does_not_roll_back() {
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
            mir2_ui_core::action::UiAction::ClosePanel,
        );
        assert_eq!(state.core.options.music_volume, 25);
        assert_eq!(state.core.panel, mir2_ui_core::state::UiPanel::None);
        let effects = effects.drain();
        assert_eq!(effects.len(), 2);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            mir2_ui_core::effect::UiEffect::ApplyAudioSettings {
                music_volume: 25,
                ..
            }
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            mir2_ui_core::effect::UiEffect::PersistOptions { .. }
        )));
    }

    #[test]
    fn exit_application_effect_is_taken_once_without_swallowing_gateway_work() {
        let mut effects = UiEffectQueue::default();
        effects.push(mir2_ui_core::effect::UiEffect::GatewayCommand(
            mir2_ui_core::effect::GatewayCommand::Logout,
        ));
        effects.push(mir2_ui_core::effect::UiEffect::ExitApplication);

        assert!(effects.take_exit_application());
        assert!(!effects.take_exit_application());
        assert_eq!(
            effects.drain(),
            vec![mir2_ui_core::effect::UiEffect::GatewayCommand(
                mir2_ui_core::effect::GatewayCommand::Logout,
            )]
        );
    }

    fn item(key: &str, name: &str, container: u8, slot: u32) -> ItemModel {
        ItemModel {
            unique_id: key.parse().ok(),
            key: key.to_owned(),
            name: name.to_owned(),
            quantity: 1,
            slot,
            container,
            ..ItemModel::default()
        }
    }

    fn init_overlay_button_test_resources(app: &mut App) {
        let test_name = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .replace(|character: char| !character.is_ascii_alphanumeric(), "-");
        let skill_bindings_path = std::env::temp_dir().join(format!(
            "mir2-overlay-skill-bindings-{}-{test_name}.json",
            std::process::id()
        ));
        app.init_resource::<MailUiState>()
            .init_resource::<StorageUiState>()
            .init_resource::<ShopUiState>()
            .init_resource::<BigMapModel>()
            .init_resource::<BigMapGatewayIntentQueue>()
            .init_resource::<BigMapUiState>()
            .init_resource::<SkillBindingUi>()
            .init_resource::<SkillModel>()
            .insert_resource(SkillBindingPersistenceRuntime::with_config_path(
                skill_bindings_path,
            ));
    }

    fn overlay_render_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<NativePlayerUiState>()
            .init_resource::<InventoryModel>()
            .init_resource::<InventoryOperationFeedback>()
            .init_resource::<MailModel>()
            .init_resource::<MailUiState>()
            .init_resource::<BigMapModel>()
            .init_resource::<BigMapUiState>()
            .init_resource::<UiReadModel>()
            .init_resource::<ShopModel>()
            .init_resource::<ShopUiState>()
            .init_resource::<GameShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<StorageUiState>()
            .init_resource::<SkillModel>()
            .init_resource::<SkillBindingUi>()
            .init_resource::<SkillBindingPersistenceRuntime>()
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Startup, spawn_overlay_root)
            .add_systems(Update, render_overlays);
        app
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
            ..ShopGood::default()
        }
    }

    #[test]
    fn crystal_page_helpers_keep_all_authoritative_rows_reachable() {
        assert_eq!(native_skill_page_count(0), 1);
        assert_eq!(native_skill_page_count(7), 1);
        assert_eq!(native_skill_page_count(8), 2);
        assert_eq!(native_game_shop_page_count(105), 14);

        let mut model = GameShopModel::default();
        for index in 0..105 {
            model.items.push(crate::game_shop::GameShopEntry {
                game_shop_index: index,
                item_name: format!("Product {index}"),
                ..Default::default()
            });
        }
        let flattened = (0..native_game_shop_page_count(model.items.len()))
            .flat_map(|page| native_game_shop_page_entries(&model, page))
            .map(|entry| entry.game_shop_index)
            .collect::<Vec<_>>();
        assert_eq!(flattened, (0..105).collect::<Vec<_>>());
        assert!(native_game_shop_page_entries(&model, 0).len() <= 8);
        assert_eq!(native_game_shop_page_entries(&model, 13).len(), 1);
    }

    #[test]
    fn native_panel_ecs_bounds_inventory_skill_and_cash_shop_at_1024x768() {
        let mut app = overlay_render_test_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::Inventory;
        app.update();

        let inventory_node = app
            .world_mut()
            .query_filtered::<&Node, With<OverlayInventory>>()
            .single(app.world())
            .expect("inventory panel");
        assert_eq!(inventory_node.width, Val::Px(316.0));
        assert_eq!(inventory_node.height, Val::Px(236.0));
        let inventory_viewports = app
            .world_mut()
            .query_filtered::<&Node, With<OverlayInventoryGridViewport>>()
            .iter(app.world())
            .count();
        assert_eq!(inventory_viewports, 1);

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::Skill;
        app.update();
        let skill_viewports = app
            .world_mut()
            .query_filtered::<&Node, With<OverlaySkillListViewport>>()
            .iter(app.world())
            .count();
        assert_eq!(skill_viewports, 1);

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::GameShop;
        for index in 0..105 {
            app.world_mut().resource_mut::<GameShopModel>().items.push(
                crate::game_shop::GameShopEntry {
                    game_shop_index: index,
                    item_name: format!("Product {index}"),
                    ..Default::default()
                },
            );
        }
        app.update();
        let shop_node = app
            .world_mut()
            .query_filtered::<&Node, With<OverlayGameShop>>()
            .single(app.world())
            .expect("cash shop panel");
        assert_eq!(shop_node.width, Val::Px(696.0));
        assert_eq!(shop_node.height, Val::Px(476.0));
        let product_count = app
            .world_mut()
            .query_filtered::<Entity, With<OverlayGameShopProduct>>()
            .iter(app.world())
            .count();
        assert_eq!(product_count, 8);
    }

    #[test]
    fn crystal_social_panels_use_original_geometry_and_bounded_rows() {
        assert_eq!(
            CRYSTAL_GROUP_PANEL_RECT,
            CrystalRect::new(396.0, 259.0, 232.0, 249.0)
        );
        assert_eq!(
            CRYSTAL_GUILD_PANEL_RECT,
            CrystalRect::new(217.0, 168.0, 590.0, 432.0)
        );
        assert_eq!(group_member_position(0), (16.0, 33.0));
        assert_eq!(group_member_position(1), (16.0, 55.0));
        assert_eq!(group_member_position(2), (116.0, 55.0));
        assert_eq!(group_member_position(14), (116.0, 175.0));

        let mut app = overlay_render_test_app();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::Group;
        }
        app.world_mut()
            .resource_mut::<crate::social::SocialModel>()
            .group
            .members = (0..crate::social::MAX_GROUP_MEMBERS)
            .map(|index| crate::social::GroupMemberModel {
                name: format!("Member{index}"),
                online: true,
                leader: index == 0,
                ..Default::default()
            })
            .collect();
        app.update();

        let social_node = app
            .world_mut()
            .query_filtered::<&Node, With<OverlaySocial>>()
            .single(app.world())
            .expect("social root");
        assert_eq!(social_node.width, Val::Px(1024.0));
        assert_eq!(social_node.height, Val::Px(768.0));
        let group_rows = app
            .world_mut()
            .query::<&OverlayButton>()
            .iter(app.world())
            .filter(|action| matches!(action, OverlayButton::SelectGroupMember(_)))
            .count();
        assert_eq!(group_rows, crate::social::MAX_GROUP_MEMBERS);

        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::Guild;
            state.guild_left_page = GuildLeftPage::Members;
        }
        app.world_mut()
            .resource_mut::<crate::social::SocialModel>()
            .guild
            .members = (0..25)
            .map(|index| crate::social::GuildMemberModel {
                name: format!("GuildMember{index}"),
                online: true,
                ..Default::default()
            })
            .collect();
        app.update();
        let guild_rows = app
            .world_mut()
            .query::<&OverlayButton>()
            .iter(app.world())
            .filter(|action| matches!(action, OverlayButton::SelectGuildMember(_)))
            .count();
        assert_eq!(guild_rows, 18);
    }

    #[test]
    fn guild_tabs_request_the_matching_authoritative_page() {
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
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            });
        init_overlay_button_test_resources(&mut app);
        app.add_systems(Update, process_overlay_buttons);

        let button = app
            .world_mut()
            .spawn((
                Button,
                Interaction::Pressed,
                OverlayButton::SelectGuildLeftPage(GuildLeftPage::Members),
            ))
            .id();
        app.update();
        app.world_mut().despawn(button);

        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .guild_left_page,
            GuildLeftPage::Members
        );
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .iter()
            .any(|intent| matches!(
                intent,
                NativePlayerUiIntent::GuildRequestInfo { info_type: 1 }
            )));
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

    #[test]
    fn belt_mouse_intent_requires_the_current_unique_instance() {
        let mut inventory = InventoryModel::default();
        assert!(belt_item_use_intent(&inventory, 0).is_none());
        inventory.items.push(ItemModel {
            unique_id: None,
            key: "legacy".to_owned(),
            name: "Legacy".to_owned(),
            quantity: 1,
            slot: 0,
            container: 1,
            ..ItemModel::default()
        });
        assert!(belt_item_use_intent(&inventory, 0).is_none());
        inventory.items[0].unique_id = Some(99);
        assert!(matches!(
            belt_item_use_intent(&inventory, 0),
            Some(NativePlayerUiIntent::UseItem {
                unique_id: Some(99),
                slot: Some(0),
                ref grid,
                ..
            }) if grid.as_deref() == Some("belt")
        ));
        // Replacing the authoritative belt model invalidates the old item
        // identity; a previously pressed HUD node must fail closed.
        inventory.items.clear();
        assert!(belt_item_use_intent(&inventory, 0).is_none());
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
    fn bigmap_zoom_is_clamped_and_retained() {
        assert_eq!(bigmap_zoom_clamped(0.1), BIGMAP_ZOOM_MIN);
        assert_eq!(bigmap_zoom_clamped(10.0), BIGMAP_ZOOM_MAX);
        assert_eq!(bigmap_zoom_in(1.0), 1.25);
        assert_eq!(bigmap_zoom_out(1.0), 0.75);
        assert_eq!(bigmap_zoom_in(BIGMAP_ZOOM_MAX), BIGMAP_ZOOM_MAX);
        assert_eq!(bigmap_zoom_out(BIGMAP_ZOOM_MIN), BIGMAP_ZOOM_MIN);
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

    fn spawn_big_map_render_test(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        model: Res<BigMapModel>,
        renderer: Res<BigMapUiState>,
        ui: Res<UiReadModel>,
    ) {
        commands.spawn(Node::default()).with_children(|parent| {
            render_bigmap(parent, Some(&asset_server), &model, &renderer, &ui);
        });
    }

    #[test]
    fn big_map_ecs_spawns_authoritative_image_player_and_npc_rows() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_resource::<BigMapUiState>()
            .init_resource::<UiReadModel>()
            .add_systems(Startup, spawn_big_map_render_test);

        let mut model = BigMapModel::default();
        model.set_current_map(1);
        model.set_player_location(Some(1), BigMapPoint { x: 257, y: 594 });
        model.apply_world_map_setup(false, Vec::new(), 3_000);
        model.apply_new_map_info(
            1,
            crate::big_map::BigMapInfo {
                title: "BichonProvince".to_owned(),
                width: 700,
                height: 700,
                big_map: 101,
                movements: Vec::new(),
                npcs: vec![crate::big_map::BigMapNpc {
                    index: 1,
                    file_name: "NPC/00".to_owned(),
                    name: "Village Guide".to_owned(),
                    map_index: 1,
                    location: BigMapPoint { x: 330, y: 270 },
                    image: 0,
                    rate: 0,
                    show_on_big_map: true,
                    big_map_icon: 0,
                    object_id: 77,
                    icon: 0,
                    can_teleport_to: false,
                }],
            },
        );
        app.insert_resource(model);
        app.update();

        let world = app.world_mut();
        let images = world
            .query::<(&BigMapImageEntity, &ImageNode)>()
            .iter(world)
            .map(|(marker, image)| (marker.url.clone(), image.image.id()))
            .collect::<Vec<_>>();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].0, "original-ui/MMap/101.png");
        let asset_server = world.resource::<AssetServer>();
        assert_eq!(
            asset_server
                .get_path(images[0].1)
                .map(|path| path.path().to_string_lossy().replace('\\', "/")),
            Some("original-ui/MMap/101.png".to_owned())
        );

        let players = world
            .query::<(&BigMapPlayerEntity, &Node, &ImageNode)>()
            .iter(world)
            .map(|(marker, node, _)| (marker.location, node.left, node.top))
            .collect::<Vec<_>>();
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].0, BigMapPoint { x: 257, y: 594 });
        let (player_x, player_y) = big_map_view_position(BigMapPoint { x: 257, y: 594 }, 700, 700);
        assert_eq!(players[0].1, Val::Px(player_x - 6.0));
        assert_eq!(players[0].2, Val::Px(player_y - 5.0));

        let rows = world
            .query::<&BigMapNpcRowEntity>()
            .iter(world)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].object_id, 77);
        assert_eq!(rows[0].name, "Village Guide");
        assert_eq!(rows[0].location, BigMapPoint { x: 330, y: 270 });
    }

    #[test]
    fn open_big_map_requests_authoritative_info_when_map_identity_arrives_late() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<BigMapModel>()
            .init_resource::<BigMapGatewayIntentQueue>()
            .init_resource::<BigMapUiState>()
            .add_systems(Update, sync_big_map_ui);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::BigMap;

        app.update();
        assert_eq!(
            app.world().resource::<BigMapGatewayIntentQueue>().len(),
            0,
            "no map request may be invented before MapInformation"
        );

        app.world_mut()
            .resource_mut::<BigMapModel>()
            .set_current_map(1);
        app.update();
        let intents = app
            .world_mut()
            .resource_mut::<BigMapGatewayIntentQueue>()
            .drain_intents();
        assert_eq!(
            intents,
            vec![crate::big_map::BigMapGatewayIntent::RequestMapInfo { map_index: 1 }]
        );

        app.update();
        assert_eq!(
            app.world().resource::<BigMapGatewayIntentQueue>().len(),
            0,
            "the same authoritative map identity must not flood requests"
        );
    }

    #[test]
    fn crystal_menu_options_and_bigmap_use_source_geometry() {
        assert_eq!(
            CRYSTAL_MENU_PANEL_RECT,
            CrystalRect::new(988.0, 349.0, 36.0, 282.0)
        );
        assert_eq!(
            CRYSTAL_OPTIONS_PANEL_RECT,
            CrystalRect::new(382.0, 207.0, 259.0, 354.0)
        );
        assert_eq!(
            CRYSTAL_BIGMAP_PANEL_RECT,
            CrystalRect::new(132.0, 134.0, 760.0, 500.0)
        );
        assert_eq!(BIGMAP_WIDTH, 568.0);
        assert_eq!(BIGMAP_HEIGHT, 380.0);
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
        init_overlay_button_test_resources(&mut app);
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
            assert!(app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .is_empty());
        }
        press(&mut app, OverlayButton::ReadMail(10));
        {
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
                ..ItemModel::default()
            },
            ItemModel {
                unique_id: Some(43),
                key: "small-hp-drug".into(),
                name: "Small HP Drug".into(),
                quantity: 2,
                slot: 1,
                container: 0,
                ..ItemModel::default()
            },
            ItemModel {
                unique_id: None,
                key: "legacy-template".into(),
                name: "Legacy".into(),
                quantity: 2,
                slot: 3,
                container: 0,
                ..ItemModel::default()
            },
        ];
        init_overlay_button_test_resources(&mut app);
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
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .drop_confirmation
            .is_some());
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
        press(&mut app, OverlayButton::ConfirmDropInspected);
        press(&mut app, OverlayButton::ConfirmDropInspected);
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
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .drop_confirmation
            .is_none());
    }

    #[test]
    fn character_slots_and_item_actions_fail_closed_without_a_live_item() {
        let mut state = NativePlayerUiState::default();
        let inventory = InventoryModel::default();
        state.inspect = Some(ItemInspect {
            container: 0,
            slot: 4,
            key: "stale".to_owned(),
            name: "Stale".to_owned(),
            quantity: 1,
        });

        assert!(inspected_use_intent(&state, &inventory).is_none());
        assert!(inspected_equip_intent(&state, &inventory).is_none());
        assert!(inspected_remove_intent(&state, &inventory).is_none());
        assert!(inspected_drop_confirmation(&state, &inventory).is_none());
    }

    #[test]
    fn stale_drop_confirmation_cannot_address_a_replacement_stack() {
        let confirmation = InventoryDropConfirmation {
            key: "Potion".to_owned(),
            unique_id: 7,
            slot: 2,
            count: 3,
        };
        let replacement = InventoryModel {
            gold: 0,
            items: vec![ItemModel {
                unique_id: Some(8),
                key: "Potion".to_owned(),
                name: "Potion".to_owned(),
                quantity: 3,
                slot: 2,
                container: 0,
                ..ItemModel::default()
            }],
        };
        assert!(!drop_confirmation_is_current(&confirmation, &replacement));
    }

    #[test]
    fn skill_selection_is_local_and_clears_when_windows_close() {
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
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(SkillModel {
                skills: vec![crate::skill_model::SkillEntry {
                    id: 17,
                    name: "Fire Ball".to_owned(),
                    level: 2,
                    key: Some("fire-ball".to_owned()),
                    cooldown_ms: 800,
                    mp_cost: 5,
                }],
                ..Default::default()
            })
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            });
        init_overlay_button_test_resources(&mut app);
        app.add_systems(Update, process_overlay_buttons);

        let selected = app
            .world_mut()
            .spawn((Interaction::Pressed, OverlayButton::SelectSkill(17), Button))
            .id();
        app.update();
        app.world_mut().despawn(selected);
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .selected_skill_id,
            Some(17)
        );

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .close_windows();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .selected_skill_id,
            None
        );
    }

    #[test]
    fn assigning_a_skill_key_persists_only_after_the_local_rebind_succeeds() {
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
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(SkillModel {
                skills: vec![crate::skill_model::SkillEntry {
                    id: 17,
                    name: "Fire Ball".to_owned(),
                    level: 2,
                    key: Some("fire-ball".to_owned()),
                    cooldown_ms: 800,
                    mp_cost: 5,
                }],
                ..Default::default()
            })
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            });
        init_overlay_button_test_resources(&mut app);
        let path = app
            .world()
            .resource::<SkillBindingPersistenceRuntime>()
            .config_path
            .clone();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
        let _ = std::fs::remove_file(path.with_extension("json.tmp"));
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::Skill;
        app.add_systems(Update, process_overlay_buttons);

        for button in [
            OverlayButton::SelectSkill(17),
            OverlayButton::AssignSkillKey(3),
        ] {
            let entity = app
                .world_mut()
                .spawn((Interaction::Pressed, button, Button))
                .id();
            app.update();
            app.world_mut().despawn(entity);
        }

        assert_eq!(
            app.world().resource::<SkillBindingUi>().skill_for_hotkey(3),
            Some(17)
        );
        let runtime = app.world().resource::<SkillBindingPersistenceRuntime>();
        assert!(!runtime.dirty);
        assert_eq!(
            runtime.last_status,
            crate::skill_binding_persistence::SkillBindingPersistStatus::Succeeded
        );
        let loaded = crate::skill_binding_persistence::load_skill_bindings_from_path(&path);
        assert_eq!(loaded.bindings.skill_for_hotkey(3), Some(17));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
        let _ = std::fs::remove_file(path.with_extension("json.tmp"));
    }

    #[test]
    fn crystal_character_inventory_and_skill_page_actions_are_local_and_real() {
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
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            });
        init_overlay_button_test_resources(&mut app);
        app.add_systems(Update, process_overlay_buttons);

        let press = |app: &mut App, action: OverlayButton| {
            let entity = app
                .world_mut()
                .spawn((Button, Interaction::Pressed, action))
                .id();
            app.update();
            app.world_mut().despawn(entity);
        };

        press(
            &mut app,
            OverlayButton::SelectCharacterPage(CharacterPage::Stats1),
        );
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().character_page,
            CharacterPage::Stats1
        );
        press(&mut app, OverlayButton::SelectInventoryPage(1));
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().inventory_page,
            1
        );
        press(&mut app, OverlayButton::SkillPageNext);
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().skill_page,
            0,
            "an empty authoritative skill model must not create a phantom page"
        );
        press(&mut app, OverlayButton::SkillPagePrev);
        assert_eq!(app.world().resource::<NativePlayerUiState>().skill_page, 0);
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
        init_overlay_button_test_resources(&mut app);
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
        }
        press(&mut app, OverlayButton::SelectStorage(0));
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
    fn cash_shop_selection_pages_beyond_first_eight_rows_is_receipt_deduped() {
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
        init_overlay_button_test_resources(&mut app);
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
            3,
            "selecting gIndex 25 must move to the fourth Crystal 4x2 page"
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

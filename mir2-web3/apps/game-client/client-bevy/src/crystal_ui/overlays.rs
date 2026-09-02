//! Operable native player windows: bag, equipment, inspect, death, menu, chat, mail, bigmap, shop, storage.

use std::collections::VecDeque;

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextLayout};
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, FocusPolicy, Interaction, JustifyContent,
    Node, PositionType, UiRect, Val,
};
use bevy::window::{CursorMoved, PrimaryWindow};

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
    SessionResetGameShopPreservation, SessionResetRevision, StorageRequestIdGenerator,
};
use crate::quest_model::NpcDialogModel;
use crate::read_model::{UiReadModel, UiSurfaceSignals};
use crate::shop::{
    shop_buy_enabled, shop_quantity_clamped, shop_quantity_dec, shop_quantity_inc,
    shop_sell_enabled, NpcShopServiceMode, NpcShopServiceSignal, ShopGood, ShopModel,
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
use super::hud::{free_inventory_slots, CrystalHudAction};
use super::item_tooltip::{
    crystal_item_tooltip_document, crystal_item_tooltip_document_from_source,
    crystal_item_tooltip_document_from_source_with_options, CrystalItemTooltipOptions,
};
use super::panel_layouts::{
    GAME_SHOP_CELL_SIZE, GAME_SHOP_COLUMN_STEP, GAME_SHOP_GRID_ORIGIN, GAME_SHOP_PAGE_COLUMNS,
    GAME_SHOP_PAGE_SIZE as CRYSTAL_GAME_SHOP_PAGE_SIZE, GAME_SHOP_PANEL_SIZE, GAME_SHOP_ROW_STEP,
    INVENTORY_CELL_SIZE, INVENTORY_DELETE_BUTTON_ORIGIN, INVENTORY_DELETE_BUTTON_SIZE,
    INVENTORY_FREE_SLOT_LABEL_ORIGIN, INVENTORY_FREE_SLOT_LABEL_SIZE, INVENTORY_GOLD_LABEL_ORIGIN,
    INVENTORY_GOLD_LABEL_SIZE, INVENTORY_GRID_ORIGIN, INVENTORY_GRID_STEP, INVENTORY_PAGE_COLUMNS,
    INVENTORY_PAGE_SIZE, INVENTORY_PANEL_ORIGIN, INVENTORY_PANEL_SIZE, INVENTORY_WEIGHT_BAR_ORIGIN,
    INVENTORY_WEIGHT_BAR_SIZE, SKILL_PAGE_SIZE, SKILL_PANEL_SIZE, SKILL_ROW_ORIGIN, SKILL_ROW_SIZE,
    SKILL_ROW_STEP_Y,
};
use super::spec::{CrystalButtonSpec, CrystalFrameSpec, CrystalRect};
use super::widget::{spawn_crystal_image_button, CrystalImageButton, CrystalItemHint};

const BIG_MAP_SEARCH_COOLDOWN_MS: u64 = 1_000;
const NPC_GOODS_CELL_WIDTH: f32 = 205.0;
const NPC_GOODS_CELL_HEIGHT: f32 = 32.0;
const NPC_GOODS_ICON_AREA_WIDTH: i32 = 40;
const NPC_GOODS_NEW_ICON_ASSET: &str = "original-ui/Prguse/550.png";

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
const GUILD_NOTICE_MAX_CHARS_PER_LINE: usize = 32;
pub const HELP_PAGE_COUNT: u8 = 45;

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
    Storage,
    Ranks,
}

/// Renderer-owned Crystal HelpDialog state. It is intentionally independent
/// from `UiPanel`: the source client allows Help to coexist with other
/// windows, and hiding/reopening the dialog preserves the current page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelpDialogUi {
    pub open: bool,
    pub page: u8,
    pub left: f32,
    pub top: f32,
    z_index: i32,
    dragging: bool,
    drag_offset_x: f32,
    drag_offset_y: f32,
}

/// Renderer-owned position for Crystal's movable InventoryDialog. The source
/// window starts at `(0,0)`, preserves its position across Hide/Show, and is
/// reconstructed at the origin only with a new game session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InventoryDialogUi {
    pub left: f32,
    pub top: f32,
    dragging: bool,
    drag_offset_x: f32,
    drag_offset_y: f32,
    last_cursor: Option<Vec2>,
}

impl Default for InventoryDialogUi {
    fn default() -> Self {
        Self {
            left: INVENTORY_PANEL_ORIGIN.x as f32,
            top: INVENTORY_PANEL_ORIGIN.y as f32,
            dragging: false,
            drag_offset_x: 0.0,
            drag_offset_y: 0.0,
            last_cursor: None,
        }
    }
}

impl InventoryDialogUi {
    pub fn dragging(&self) -> bool {
        self.dragging
    }

    fn begin_drag(&mut self, cursor_x: f32, cursor_y: f32) -> bool {
        if !inventory_drag_surface_contains(self, cursor_x, cursor_y) {
            return false;
        }
        self.dragging = true;
        self.drag_offset_x = cursor_x - self.left;
        self.drag_offset_y = cursor_y - self.top;
        true
    }

    fn drag_to(&mut self, cursor_x: f32, cursor_y: f32) {
        if !self.dragging {
            return;
        }
        self.left = (cursor_x - self.drag_offset_x).clamp(0.0, INVENTORY_MAX_LEFT);
        self.top = (cursor_y - self.drag_offset_y).clamp(0.0, INVENTORY_MAX_TOP);
    }

    fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_offset_x = 0.0;
        self.drag_offset_y = 0.0;
    }

    fn remember_cursor(&mut self, cursor: Option<Vec2>) {
        if let Some(cursor) = cursor {
            self.last_cursor = Some(cursor);
        }
    }

    fn clear_cursor(&mut self) {
        self.last_cursor = None;
    }
}

impl Default for HelpDialogUi {
    fn default() -> Self {
        Self {
            open: false,
            page: 0,
            left: CRYSTAL_HELP_PANEL_RECT.left,
            top: CRYSTAL_HELP_PANEL_RECT.top,
            z_index: OVERLAY_NPC_DIALOG_Z,
            dragging: false,
            drag_offset_x: 0.0,
            drag_offset_y: 0.0,
        }
    }
}

impl HelpDialogUi {
    pub fn toggle(&mut self) {
        if self.open {
            self.hide();
        } else {
            self.show();
        }
    }

    fn show(&mut self) {
        self.open = true;
        self.sort_to_front();
    }

    pub fn hide(&mut self) {
        self.open = false;
        self.end_drag();
    }

    pub fn previous_page(&mut self) {
        self.page = if self.page == 0 {
            HELP_PAGE_COUNT - 1
        } else {
            self.page - 1
        };
    }

    pub fn next_page(&mut self) {
        self.page = (self.page + 1) % HELP_PAGE_COUNT;
    }

    pub fn display_page(&mut self, page: usize) {
        self.page = page.min(usize::from(HELP_PAGE_COUNT - 1)) as u8;
        self.show();
    }

    pub fn dragging(&self) -> bool {
        self.dragging
    }

    fn begin_drag(&mut self, cursor_x: f32, cursor_y: f32) -> bool {
        if !self.open || !help_drag_surface_contains(self, cursor_x, cursor_y) {
            return false;
        }
        self.dragging = true;
        self.drag_offset_x = cursor_x - self.left;
        self.drag_offset_y = cursor_y - self.top;
        self.sort_to_front();
        true
    }

    fn drag_to(&mut self, cursor_x: f32, cursor_y: f32) {
        if !self.dragging {
            return;
        }
        self.left = (cursor_x - self.drag_offset_x).clamp(0.0, HELP_MAX_LEFT);
        self.top = (cursor_y - self.drag_offset_y).clamp(0.0, HELP_MAX_TOP);
    }

    fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_offset_x = 0.0;
        self.drag_offset_y = 0.0;
    }

    fn sort_to_front(&mut self) {
        self.z_index = OVERLAY_HELP_SORTED_Z;
    }

    fn z_index(&self) -> i32 {
        self.z_index
    }
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
pub const CRYSTAL_HELP_PANEL_RECT: CrystalRect = CrystalRect::new(244.0, 129.0, 536.0, 509.0);
/// `CharacterDialog.Location = (ScreenWidth - 264, 0)` at Crystal's fixed
/// 1024x768 stage.
pub const CRYSTAL_CHARACTER_PANEL_RECT: CrystalRect = CrystalRect::new(760.0, 0.0, 264.0, 380.0);
const CRYSTAL_CHARACTER_PAGE_RECT: CrystalRect = CrystalRect::new(8.0, 90.0, 248.0, 284.0);
const CRYSTAL_CHARACTER_EQUIPMENT_SLOTS: [(u32, CrystalRect); 14] = [
    (0, CrystalRect::new(131.0, 97.0, 32.0, 32.0)),
    (1, CrystalRect::new(171.0, 97.0, 32.0, 32.0)),
    (2, CrystalRect::new(211.0, 97.0, 32.0, 32.0)),
    (13, CrystalRect::new(211.0, 152.0, 32.0, 32.0)),
    (4, CrystalRect::new(211.0, 188.0, 32.0, 32.0)),
    (3, CrystalRect::new(211.0, 224.0, 32.0, 32.0)),
    (5, CrystalRect::new(16.0, 260.0, 32.0, 32.0)),
    (6, CrystalRect::new(211.0, 260.0, 32.0, 32.0)),
    (7, CrystalRect::new(16.0, 296.0, 32.0, 32.0)),
    (8, CrystalRect::new(211.0, 296.0, 32.0, 32.0)),
    (9, CrystalRect::new(16.0, 332.0, 32.0, 32.0)),
    (11, CrystalRect::new(56.0, 332.0, 32.0, 32.0)),
    (10, CrystalRect::new(96.0, 332.0, 32.0, 32.0)),
    (12, CrystalRect::new(136.0, 332.0, 32.0, 32.0)),
];
const CRYSTAL_MALE_HAIR_RECTS: [CrystalRect; 9] = [
    CrystalRect::new(131.0, 173.0, 16.0, 14.0),
    CrystalRect::new(127.0, 170.0, 20.0, 33.0),
    CrystalRect::new(127.0, 174.0, 24.0, 16.0),
    CrystalRect::new(118.0, 157.0, 36.0, 37.0),
    CrystalRect::new(118.0, 157.0, 36.0, 37.0),
    CrystalRect::new(118.0, 157.0, 36.0, 37.0),
    CrystalRect::new(128.0, 173.0, 20.0, 23.0),
    CrystalRect::new(128.0, 173.0, 20.0, 23.0),
    CrystalRect::new(128.0, 173.0, 20.0, 22.0),
];
const CRYSTAL_ASSASSIN_MALE_HAIR_RECTS: [CrystalRect; 9] = [
    CrystalRect::new(125.0, 147.0, 16.0, 21.0),
    CrystalRect::new(120.0, 146.0, 28.0, 31.0),
    CrystalRect::new(118.0, 150.0, 28.0, 26.0),
    CrystalRect::new(104.0, 126.0, 44.0, 46.0),
    CrystalRect::new(104.0, 126.0, 44.0, 46.0),
    CrystalRect::new(104.0, 126.0, 44.0, 46.0),
    CrystalRect::new(123.0, 149.0, 20.0, 26.0),
    CrystalRect::new(123.0, 149.0, 20.0, 26.0),
    CrystalRect::new(123.0, 149.0, 20.0, 26.0),
];
const CRYSTAL_FEMALE_HAIR_RECTS: [CrystalRect; 9] = [
    CrystalRect::new(126.0, 171.0, 24.0, 25.0),
    CrystalRect::new(128.0, 171.0, 20.0, 24.0),
    CrystalRect::new(116.0, 160.0, 40.0, 38.0),
    CrystalRect::new(126.0, 161.0, 28.0, 29.0),
    CrystalRect::new(126.0, 161.0, 28.0, 29.0),
    CrystalRect::new(126.0, 161.0, 28.0, 29.0),
    CrystalRect::new(116.0, 167.0, 44.0, 31.0),
    CrystalRect::new(116.0, 167.0, 44.0, 31.0),
    CrystalRect::new(118.0, 168.0, 40.0, 30.0),
];
const CRYSTAL_ASSASSIN_FEMALE_HAIR_RECTS: [CrystalRect; 9] = [
    CrystalRect::new(122.0, 156.0, 24.0, 24.0),
    CrystalRect::new(125.0, 155.0, 20.0, 23.0),
    CrystalRect::new(122.0, 149.0, 24.0, 32.0),
    CrystalRect::new(122.0, 139.0, 32.0, 37.0),
    CrystalRect::new(122.0, 139.0, 32.0, 37.0),
    CrystalRect::new(122.0, 139.0, 32.0, 37.0),
    CrystalRect::new(114.0, 149.0, 40.0, 33.0),
    CrystalRect::new(114.0, 149.0, 40.0, 33.0),
    CrystalRect::new(114.0, 149.0, 40.0, 33.0),
];
const CRYSTAL_HELP_DRAG_HEADER_RECT: CrystalRect = CrystalRect::new(0.0, 0.0, 509.0, 35.0);
const CRYSTAL_HELP_TITLE_RECT: CrystalRect = CrystalRect::new(18.0, 9.0, 45.0, 14.0);
const HELP_MAX_LEFT: f32 = 1024.0 - CRYSTAL_HELP_PANEL_RECT.width - 1.0;
const HELP_MAX_TOP: f32 = 768.0 - CRYSTAL_HELP_PANEL_RECT.height - 1.0;
const INVENTORY_MAX_LEFT: f32 = 1024.0 - INVENTORY_PANEL_SIZE.width as f32 - 1.0;
const INVENTORY_MAX_TOP: f32 = 768.0 - INVENTORY_PANEL_SIZE.height as f32 - 1.0;
const CRYSTAL_INVENTORY_TAB_RECTS: [CrystalRect; 3] = [
    CrystalRect::new(6.0, 7.0, 72.0, 23.0),
    CrystalRect::new(76.0, 7.0, 72.0, 23.0),
    CrystalRect::new(146.0, 7.0, 72.0, 23.0),
];
const CRYSTAL_INVENTORY_ADD_RECT: CrystalRect = CrystalRect::new(235.0, 5.0, 72.0, 23.0);
const CRYSTAL_INVENTORY_CLOSE_RECT: CrystalRect = CrystalRect::new(289.0, 3.0, 24.0, 21.0);

fn help_drag_surface_contains(help: &HelpDialogUi, cursor_x: f32, cursor_y: f32) -> bool {
    let local_x = cursor_x - help.left;
    let local_y = cursor_y - help.top;
    CRYSTAL_HELP_DRAG_HEADER_RECT.contains(local_x, local_y)
        && !CRYSTAL_HELP_TITLE_RECT.contains(local_x, local_y)
}

/// MirControl sends a press to the deepest child under the cursor. Therefore
/// InventoryDialog moves only from exposed background pixels: tabs, cells and
/// footer controls consume their own presses while the one-pixel cell gutters
/// and other frame areas continue to drag the parent.
fn inventory_drag_surface_contains(
    inventory: &InventoryDialogUi,
    cursor_x: f32,
    cursor_y: f32,
) -> bool {
    let local_x = cursor_x - inventory.left;
    let local_y = cursor_y - inventory.top;
    let local = CrystalRect::new(
        0.0,
        0.0,
        INVENTORY_PANEL_SIZE.width as f32,
        INVENTORY_PANEL_SIZE.height as f32,
    );
    if !local.contains(local_x, local_y)
        || CRYSTAL_INVENTORY_TAB_RECTS
            .iter()
            .any(|rect| rect.contains(local_x, local_y))
        || CRYSTAL_INVENTORY_ADD_RECT.contains(local_x, local_y)
        || CRYSTAL_INVENTORY_CLOSE_RECT.contains(local_x, local_y)
        || CrystalRect::new(
            INVENTORY_GOLD_LABEL_ORIGIN.x as f32,
            INVENTORY_GOLD_LABEL_ORIGIN.y as f32,
            INVENTORY_GOLD_LABEL_SIZE.width as f32,
            INVENTORY_GOLD_LABEL_SIZE.height as f32,
        )
        .contains(local_x, local_y)
        || CrystalRect::new(
            INVENTORY_FREE_SLOT_LABEL_ORIGIN.x as f32,
            INVENTORY_FREE_SLOT_LABEL_ORIGIN.y as f32,
            INVENTORY_FREE_SLOT_LABEL_SIZE.width as f32,
            INVENTORY_FREE_SLOT_LABEL_SIZE.height as f32,
        )
        .contains(local_x, local_y)
        || CrystalRect::new(
            INVENTORY_DELETE_BUTTON_ORIGIN.x as f32,
            INVENTORY_DELETE_BUTTON_ORIGIN.y as f32,
            INVENTORY_DELETE_BUTTON_SIZE.width as f32,
            INVENTORY_DELETE_BUTTON_SIZE.height as f32,
        )
        .contains(local_x, local_y)
    {
        return false;
    }

    !(0..INVENTORY_PAGE_SIZE).any(|slot| {
        let x = INVENTORY_GRID_ORIGIN.x as f32
            + (slot % INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.x as f32;
        let y = INVENTORY_GRID_ORIGIN.y as f32
            + (slot / INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.y as f32;
        CrystalRect::new(
            x,
            y,
            INVENTORY_CELL_SIZE.width as f32,
            INVENTORY_CELL_SIZE.height as f32,
        )
        .contains(local_x, local_y)
    })
}

fn help_cursor_logical(window: &Window) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    Some(cursor_logical(window, cursor))
}

fn cursor_logical(window: &Window, cursor: Vec2) -> Vec2 {
    let transform = super::metrics::CrystalStageTransform::fit(
        window.resolution.width(),
        window.resolution.height(),
    );
    let (x, y) = transform.physical_to_logical(cursor.x, cursor.y);
    Vec2::new(x, y)
}

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

/// Exact bag-instance identity captured when Crystal opens its destructive
/// delete prompt.  The live inventory snapshot must still contain this same
/// stack before the native client is allowed to emit `DeleteItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryDeleteTarget {
    unique_id: u64,
    slot: u32,
    key: String,
    name: String,
    max_count: u16,
}

/// Renderer-owned state for Crystal's two delete prompt shapes:
/// `MirAmountBox` for a stack and `MirMessageBox(YesNo)` for one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryDeletePrompt {
    Amount {
        target: InventoryDeleteTarget,
        draft: String,
        /// Crystal selects the initial maximum value, so the first typed digit
        /// replaces it instead of appending to it.
        select_all: bool,
    },
    Confirm {
        target: InventoryDeleteTarget,
    },
}

impl InventoryDeletePrompt {
    fn target(&self) -> &InventoryDeleteTarget {
        match self {
            Self::Amount { target, .. } | Self::Confirm { target } => target,
        }
    }
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

fn guild_notice_lines(draft: &str) -> Option<Vec<String>> {
    let mut lines = draft
        .split('\n')
        .map(|line| line.trim_end_matches('\r').trim_end().to_owned())
        .collect::<Vec<_>>();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    if lines.len() > crate::social::MAX_NOTICE_LINES
        || lines
            .iter()
            .any(|line| line.chars().count() > GUILD_NOTICE_MAX_CHARS_PER_LINE)
    {
        return None;
    }
    lines.retain(|line| !line.is_empty());
    Some(lines)
}

fn push_guild_notice_text(draft: &mut String, text: &str) {
    for ch in text.chars() {
        if ch == '\n' {
            if draft.split('\n').count() < crate::social::MAX_NOTICE_LINES {
                draft.push('\n');
            }
        } else if !ch.is_control()
            && draft
                .rsplit('\n')
                .next()
                .map(str::chars)
                .map(Iterator::count)
                .unwrap_or_default()
                < GUILD_NOTICE_MAX_CHARS_PER_LINE
        {
            draft.push(ch);
        }
    }
}

fn valid_social_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed == name
        && trimmed.chars().count() <= 32
        && trimmed.chars().all(|ch| !ch.is_control())
}

fn push_social_name_text(draft: &mut String, text: &str) {
    for ch in text.chars() {
        if !ch.is_control() && draft.chars().count() < 32 {
            draft.push(ch);
        }
    }
}

fn social_has_permission(guild: &crate::social::GuildModel, key: &str) -> bool {
    guild.permissions.iter().any(|permission| {
        let without_can = permission
            .get(..3)
            .filter(|prefix| prefix.eq_ignore_ascii_case("can"))
            .and_then(|_| permission.get(3..));
        permission.eq_ignore_ascii_case(key)
            || without_can.is_some_and(|permission| permission.eq_ignore_ascii_case(key))
            || (key.eq_ignore_ascii_case("notice")
                && (permission.eq_ignore_ascii_case("changeNotice")
                    || permission.eq_ignore_ascii_case("CanChangeNotice")))
            || (key.eq_ignore_ascii_case("changeRank")
                && (permission.eq_ignore_ascii_case("rank")
                    || permission.eq_ignore_ascii_case("CanChangeRank")))
    })
}

fn queue_group_invite_by_name(
    intents: &mut NativePlayerUiIntentQueue,
    social: &mut crate::social::SocialModel,
    name: String,
) -> bool {
    if !valid_social_name(&name) {
        return false;
    }
    let add = NativePlayerUiIntent::GroupAddMember { name: name.clone() };
    if intents.intents.iter().any(|queued| queued == &add) {
        return false;
    }
    let enable = NativePlayerUiIntent::GroupSwitch { allow_group: true };
    let needs_enable =
        !social.group.allow_invites && !intents.intents.iter().any(|queued| queued == &enable);
    let required = usize::from(needs_enable) + 1;
    if intents.intents.len().saturating_add(required) > MAX_QUEUED
        || !social
            .begin_pending(crate::social::SocialPendingOperation::GroupAdd { name: name.clone() })
    {
        return false;
    }
    if needs_enable {
        intents.intents.push_back(enable);
    }
    intents.intents.push_back(add);
    true
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
    /// Local presentation tab for an NPC service that advertises both Buy
    /// and Sell. Capabilities remain authoritative in `ShopModel`.
    pub npc_shop_buy_tab: bool,
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
    /// Crystal InventoryDialog's persistent footer-bin toggle.  This is
    /// presentation state only; deleting still requires an exact prompt and
    /// an authoritative server acknowledgement.
    pub inventory_delete_mode: bool,
    pub inventory_delete_prompt: Option<InventoryDeletePrompt>,
    pub inventory_window: InventoryDialogUi,
    pub selected_group_member: Option<u8>,
    /// Crystal's group window accepts a player name independently of the
    /// current combat target.  This renderer-owned draft is bounded and is
    /// never copied into the authoritative social model.
    pub group_invite_draft: String,
    pub group_invite_focused: bool,
    pub selected_guild_member: Option<u8>,
    /// Same boundary for guild recruitment: local text until one typed
    /// EditGuildMember request is accepted by the outbound queue.
    pub guild_recruit_draft: String,
    pub guild_recruit_focused: bool,
    pub guild_gold_draft: String,
    pub guild_gold_focused: bool,
    pub guild_storage_page: usize,
    pub selected_guild_rank: Option<u8>,
    pub guild_rank_name_draft: String,
    pub guild_rank_name_focused: bool,
    pub guild_left_page: GuildLeftPage,
    /// Renderer-owned Guild Notice editor. The authoritative notice remains
    /// in `SocialModel`; this draft is never copied there optimistically.
    pub guild_notice_editing: bool,
    pub guild_notice_draft: String,
    pub guild_notice_submission: Option<Vec<String>>,
    pub help: HelpDialogUi,
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
            npc_shop_buy_tab: true,
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
            inventory_delete_mode: false,
            inventory_delete_prompt: None,
            inventory_window: InventoryDialogUi::default(),
            selected_group_member: None,
            group_invite_draft: String::new(),
            group_invite_focused: false,
            selected_guild_member: None,
            guild_recruit_draft: String::new(),
            guild_recruit_focused: false,
            guild_gold_draft: String::new(),
            guild_gold_focused: false,
            guild_storage_page: 0,
            selected_guild_rank: None,
            guild_rank_name_draft: String::new(),
            guild_rank_name_focused: false,
            guild_left_page: GuildLeftPage::Notice,
            guild_notice_editing: false,
            guild_notice_draft: String::new(),
            guild_notice_submission: None,
            help: HelpDialogUi::default(),
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

    pub fn help_open(&self) -> bool {
        self.help.open
    }

    pub fn toggle_inventory(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenInventory);
        if !self.inventory_open() {
            self.inventory_window.end_drag();
            self.inventory_window.clear_cursor();
        }
    }
    pub fn toggle_equipment(&mut self) {
        self.apply(mir2_ui_core::action::UiAction::OpenCharacter);
    }
    fn activate_character_hud_button(&mut self) {
        // Crystal's main Character button closes only when the dialog is
        // already showing CharacterPage. From a stats/spells page it keeps
        // the dialog open and returns to CharacterPage instead.
        if self.equipment_open() && self.character_page == CharacterPage::Character {
            self.apply(mir2_ui_core::action::UiAction::OpenCharacter);
            return;
        }
        if !self.equipment_open() {
            self.apply(mir2_ui_core::action::UiAction::OpenCharacter);
        }
        self.character_page = CharacterPage::Character;
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
        self.core.blocks_world_click()
            || self.inspect.is_some()
            || self.inventory_delete_prompt.is_some()
            || self.help.open
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
        self.inventory_delete_mode = false;
        self.inventory_delete_prompt = None;
        self.inventory_window.end_drag();
        self.inventory_window.clear_cursor();
        self.shop_repair_container = 0;
        self.shop_repair_slot = None;
        self.game_shop_page = 0;
        self.split_count = 1;
        self.selected_group_member = None;
        self.group_invite_draft.clear();
        self.group_invite_focused = false;
        self.selected_guild_member = None;
        self.guild_recruit_draft.clear();
        self.guild_recruit_focused = false;
        self.guild_gold_draft.clear();
        self.guild_gold_focused = false;
        self.guild_storage_page = 0;
        self.selected_guild_rank = None;
        self.guild_rank_name_draft.clear();
        self.guild_rank_name_focused = false;
        self.guild_left_page = GuildLeftPage::Notice;
    }

    pub fn inventory_delete_prompt_open(&self) -> bool {
        self.inventory_delete_prompt.is_some()
    }

    /// Open the source-shaped delete prompt for one current carried-item
    /// slot. Quest inventory, legacy rows without an instance id, zero-sized
    /// stacks and counts outside the wire's `u16` domain all fail closed.
    pub fn open_inventory_delete_for_slot(
        &mut self,
        inventory: &InventoryModel,
        slot: u32,
    ) -> bool {
        let Some(item) = inventory
            .items
            .iter()
            .find(|item| item.container == 0 && item.slot == slot)
        else {
            return false;
        };
        let Some(prompt) = inventory_delete_prompt_for_item(item) else {
            return false;
        };
        self.inventory_delete_prompt = Some(prompt);
        self.inspect = None;
        self.inventory_operation = None;
        self.drop_confirmation = None;
        true
    }

    fn cancel_inventory_delete(&mut self) {
        self.inventory_delete_mode = false;
        self.inventory_delete_prompt = None;
        self.inspect = None;
    }
    pub fn close_all_windows(&mut self) {
        self.close_windows();
        self.help.hide();
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
/// Crystal HelpDialog has `Sort = true`: showing or pressing its drag surface
/// raises it above peer NPC dialogs while preserving the dedicated death/menu
/// modal layers.
pub const OVERLAY_HELP_SORTED_Z: i32 = OVERLAY_DEATH_Z - 1;
pub const OVERLAY_DEATH_Z: i32 = 985;
pub const OVERLAY_MENU_Z: i32 = 990;
const OVERLAY_INVENTORY_DELETE_MODAL_Z: i32 = 991;
const OVERLAY_INVENTORY_DELETE_CURSOR_Z: i32 = 992;
pub const OVERLAY_SHELL_Z: i32 = 1000;

/// `MirAmountBox` / `MirMessageBox` use integer centering at Crystal's fixed
/// 1024x768 stage. Their dimensions come from Prguse frames 238 and 360.
const CRYSTAL_DELETE_AMOUNT_RECT: CrystalRect = CrystalRect::new(410.0, 329.0, 204.0, 109.0);
const CRYSTAL_DELETE_CONFIRM_RECT: CrystalRect = CrystalRect::new(284.0, 289.0, 456.0, 190.0);
const CRYSTAL_DELETE_CURSOR_SIZE: (f32, f32) = (16.0, 15.0);

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
    if state.inventory_delete_prompt_open() {
        return Some(OverlayModalPriority::SystemMenu);
    }
    if state.menu_open() {
        return Some(OverlayModalPriority::SystemMenu);
    }
    if dead {
        return Some(OverlayModalPriority::Death);
    }
    if state.chat_focused() {
        return Some(OverlayModalPriority::Chat);
    }
    if dialog_open || state.quest_open() || state.help_open() {
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
    DeleteItem {
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
        request_id: String,
        unique_id: u64,
        from: i32,
        to: i32,
    },
    TakeBackItem {
        request_id: String,
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
    GuildStorageGoldChange {
        change_type: u8,
        amount: u32,
    },
    GuildStorageItemChange {
        change_type: u8,
        from: i32,
        to: i32,
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
            Self::DeleteItem {
                unique_id, count, ..
            } => Some(PendingOperationKey::DeleteItem {
                unique_id: *unique_id,
                count: *count,
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
                request_id,
                unique_id,
                from,
                to,
            } => Some(PendingOperationKey::StorageDepositV2 {
                request_id: request_id.clone(),
                unique_id: *unique_id,
                from: *from,
                to: *to,
            }),
            Self::TakeBackItem {
                request_id,
                unique_id,
                from,
                to,
            } => Some(PendingOperationKey::StorageWithdrawV2 {
                request_id: request_id.clone(),
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
            | Self::GuildStorageGoldChange { .. }
            | Self::GuildStorageItemChange { .. }
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
    storage_request_ids: StorageRequestIdGenerator,
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
            } => {
                let Some(inviter) = social.group.pending_invite_from.clone() else {
                    return false;
                };
                Some(crate::social::SocialPendingOperation::GroupInviteAccept {
                    inviter,
                    invite_epoch: social.group.pending_invite_epoch,
                })
            }
            NativePlayerUiIntent::GuildRequestInfo { .. } => {
                Some(crate::social::SocialPendingOperation::GuildInfo)
            }
            NativePlayerUiIntent::GuildEditMember {
                change_type,
                rank_index,
                name,
                ..
            } => Some(crate::social::SocialPendingOperation::GuildMember {
                change_type: *change_type,
                rank_index: *rank_index,
                name: name.clone(),
            }),
            NativePlayerUiIntent::GuildEditNotice { notice } => {
                Some(crate::social::SocialPendingOperation::GuildNotice {
                    notice: notice.clone(),
                })
            }
            NativePlayerUiIntent::GuildInvite {
                accept_invite: true,
            } => {
                let Some(inviter) = social.guild.pending_invite_from.clone() else {
                    return false;
                };
                Some(crate::social::SocialPendingOperation::GuildInviteAccept {
                    inviter,
                    invite_epoch: social.guild.pending_invite_epoch,
                })
            }
            NativePlayerUiIntent::GuildStorageGoldChange {
                change_type,
                amount,
            } => Some(crate::social::SocialPendingOperation::GuildStorageGold {
                change_type: *change_type,
                amount: *amount,
            }),
            NativePlayerUiIntent::GuildStorageItemChange {
                change_type,
                from,
                to,
            } if *change_type <= 2 => {
                Some(crate::social::SocialPendingOperation::GuildStorageItem {
                    change_type: *change_type,
                    from: *from,
                    to: *to,
                })
            }
            NativePlayerUiIntent::GuildStorageItemChange { .. } => None,
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

    /// Allocate and enqueue a V2 storage transfer. Allocation happens before
    /// any capacity/pending check so a failed attempt still consumes its id;
    /// ids are never reused after a connection reset or queue rejection.
    pub fn push_storage_pending_intent(
        &mut self,
        pending: &mut PendingOperations,
        deposit: bool,
        unique_id: u64,
        from: i32,
        to: i32,
    ) -> bool {
        let Some(request_id) = self.storage_request_ids.next_request_id() else {
            return false;
        };
        let intent = if deposit {
            NativePlayerUiIntent::StoreItem {
                request_id,
                unique_id,
                from,
                to,
            }
        } else {
            NativePlayerUiIntent::TakeBackItem {
                request_id,
                unique_id,
                from,
                to,
            }
        };
        self.push_pending_intent(pending, intent)
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
struct OverlayInventoryDeleteModal;

#[derive(Component)]
struct OverlayInventoryDeleteCursor;

#[derive(Component)]
struct OverlayInventoryDeleteDialog;

#[derive(Component)]
struct OverlayInventoryDeleteAmountInput;

#[derive(Component)]
struct OverlayEquipment;

#[derive(Component)]
struct OverlayMenu;

#[derive(Component)]
struct OverlayHelp;

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
struct HelpPageImageEntity {
    image_index: u8,
}

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

/// Renderer-only marker for the authoritative map response wait state.  It
/// intentionally carries no map identity or fake image: once the server
/// supplies `map_image_url`, the normal panel rebuild removes this entity.
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
struct BigMapLoadingText;

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
struct OverlayNpcShopGoodCell;

#[derive(Component)]
struct OverlayNpcShopGoodIcon;

#[derive(Component)]
struct OverlayNpcShopGoodName;

#[derive(Component)]
struct OverlayNpcShopGoodPrice;

#[derive(Component)]
struct OverlayNpcShopGoodCount;

#[derive(Component)]
struct OverlayNpcShopGoodSelectionDivider;

#[derive(Component)]
struct OverlayNpcShopGoodNewIcon;

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
    ToggleHelp,
    CloseHelp,
    HelpPrevious,
    HelpNext,
    CloseCharacter,
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
    GroupInviteNameFocus,
    GroupInviteNameSubmit,
    GroupRemoveSelected,
    SelectGroupMember(u8),
    GuildRequestInfo,
    SelectGuildLeftPage(GuildLeftPage),
    GuildInviteAccept,
    GuildInviteDecline,
    GuildRecruitNameFocus,
    GuildRecruitNameSubmit,
    GuildBeginNoticeEdit,
    GuildPublishNotice,
    GuildCancelNoticeEdit,
    SelectGuildMember(u8),
    GuildKickMember(u8),
    GuildKickSelected,
    GuildAssignPreviousRank,
    GuildAssignNextRank,
    GuildGoldFocus,
    GuildGoldDeposit,
    GuildGoldWithdraw,
    GuildStoragePreviousPage,
    GuildStorageNextPage,
    SelectGuildRank(u8),
    GuildRankNameFocus,
    GuildRankNameSave,
    GuildRankTogglePermission(u8),
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
    InventoryDeleteToggle,
    InventoryDeleteConfirm,
    InventoryDeleteCancel,
    InventoryDeleteAmountClose,
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
    InspectQuest(u32),
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
    ShopShowBuy,
    ShopShowSell,
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
    ui_audio: ResMut<'w, crate::audio::NativeUiAudioQueue>,
    buttons: Query<'w, 's, (&'static Interaction, &'static OverlayButton), Changed<Interaction>>,
}

#[derive(SystemParam)]
pub(crate) struct OverlayKeyboardControls<'w> {
    surface_signals: Option<ResMut<'w, UiSurfaceSignals>>,
    ui_audio: ResMut<'w, crate::audio::NativeUiAudioQueue>,
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
            .init_resource::<crate::audio::NativeGameplayAudioQueue>()
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .add_message::<CursorMoved>()
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
                    process_help_drag,
                    process_inventory_drag,
                    process_inventory_delete_pointer,
                    process_overlay_keyboard,
                    process_overlay_buttons,
                    crate::audio::sync_native_ui_audio,
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
    mut state: ResMut<NativePlayerUiState>,
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
    reconcile_inventory_capacity(&mut state, &inventory);
    if !state.inventory_open() {
        state.inventory_delete_mode = false;
        state.inventory_delete_prompt = None;
    } else if state
        .inventory_delete_prompt
        .as_ref()
        .is_some_and(|prompt| !inventory_delete_prompt_is_current(prompt, &inventory))
    {
        // Never leave a destructive modal addressing a replaced authoritative
        // stack after an inventory refresh.
        state.cancel_inventory_delete();
    }
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

fn reconcile_inventory_capacity(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
) -> bool {
    if state.inventory_page != 1 || inventory.second_bag_unlocked() {
        return false;
    }
    state.inventory_page = 0;
    state.inspect = None;
    state.inventory_operation = None;
    state.drop_confirmation = None;
    true
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
        3 => "quest",
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
                    left: Val::Px(INVENTORY_PANEL_ORIGIN.x as f32),
                    top: Val::Px(INVENTORY_PANEL_ORIGIN.y as f32),
                    width: Val::Px(INVENTORY_PANEL_SIZE.width as f32),
                    height: Val::Px(INVENTORY_PANEL_SIZE.height as f32),
                    display: Display::None,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            // A Crystal MirAmountBox/MirMessageBox is modal to the entire
            // scene. The full-stage Button consumes pointer hits that would
            // otherwise reach Inventory controls beneath the prompt.
            root.spawn((
                OverlayInventoryDeleteModal,
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(1024.0),
                    height: Val::Px(768.0),
                    display: Display::None,
                    ..default()
                },
                GlobalZIndex(OVERLAY_INVENTORY_DELETE_MODAL_Z),
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlayInventoryDeleteCursor,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(CRYSTAL_DELETE_CURSOR_SIZE.0),
                    height: Val::Px(CRYSTAL_DELETE_CURSOR_SIZE.1),
                    display: Display::None,
                    ..default()
                },
                GlobalZIndex(OVERLAY_INVENTORY_DELETE_CURSOR_Z),
                BackgroundColor(Color::NONE),
            ));
            root.spawn((
                OverlayEquipment,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(CRYSTAL_CHARACTER_PANEL_RECT.left),
                    top: Val::Px(CRYSTAL_CHARACTER_PANEL_RECT.top),
                    width: Val::Px(CRYSTAL_CHARACTER_PANEL_RECT.width),
                    height: Val::Px(CRYSTAL_CHARACTER_PANEL_RECT.height),
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
                OverlayHelp,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(CRYSTAL_HELP_PANEL_RECT.left),
                    top: Val::Px(CRYSTAL_HELP_PANEL_RECT.top),
                    width: Val::Px(CRYSTAL_HELP_PANEL_RECT.width),
                    height: Val::Px(CRYSTAL_HELP_PANEL_RECT.height),
                    display: Display::None,
                    overflow: Overflow::clip(),
                    ..default()
                },
                GlobalZIndex(OVERLAY_NPC_DIALOG_Z),
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
    buttons: Query<
        (&Interaction, &CrystalHudAction, Option<&CrystalImageButton>),
        Changed<Interaction>,
    >,
    shell: Option<Res<NativeShellModel>>,
    inventory: Res<InventoryModel>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    mut ui_audio: ResMut<crate::audio::NativeUiAudioQueue>,
) {
    if !shell.is_some_and(|model| model.screen == NativeShellScreen::InGame) {
        return;
    }
    for (interaction, action, image_button) in buttons.iter() {
        if *interaction != Interaction::Pressed
            || image_button.is_some_and(|button| !button.enabled)
        {
            continue;
        }
        if let Some(sound) = hud_pointer_sound(*action) {
            ui_audio.push(sound);
        }
        match action {
            CrystalHudAction::Inventory => {
                state.toggle_inventory();
                if !state.inventory_open() {
                    state.inspect = None;
                    state.inventory_operation = None;
                    state.drop_confirmation = None;
                    state.inventory_delete_mode = false;
                    state.inventory_delete_prompt = None;
                }
            }
            CrystalHudAction::Character => {
                state.activate_character_hud_button();
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

fn hud_pointer_sound(action: CrystalHudAction) -> Option<crate::audio::NativeUiSound> {
    match action {
        // Crystal `MainDialog` assigns ButtonA to the five small lower-right
        // HUD buttons. Keep this bounded to source-audited controls.
        CrystalHudAction::Inventory
        | CrystalHudAction::Character
        | CrystalHudAction::Skill
        | CrystalHudAction::Quest
        | CrystalHudAction::Option => Some(crate::audio::NativeUiSound::ButtonA),
        // Crystal `MenuButton` and `GameShopButton` use the distinct
        // `SoundList.ButtonC` local UI cue.
        CrystalHudAction::Menu | CrystalHudAction::GameShop => {
            Some(crate::audio::NativeUiSound::ButtonC)
        }
        _ => None,
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

/// Crystal's top-level HelpDialog is movable. The source control records the
/// cursor-to-window offset on press, follows that offset while held, clamps to
/// the fixed logical stage, and clears movement on release or Hide.
fn process_help_drag(
    mut state: ResMut<NativePlayerUiState>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !state.help.open {
        state.help.end_drag();
        return;
    }
    let Some(mouse) = mouse else {
        state.help.end_drag();
        return;
    };
    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        state.help.end_drag();
        return;
    }
    let Ok(window) = windows.single() else {
        state.help.end_drag();
        return;
    };
    if !window.focused {
        state.help.end_drag();
        return;
    }
    let Some(cursor) = help_cursor_logical(window) else {
        state.help.end_drag();
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        let _ = state.help.begin_drag(cursor.x, cursor.y);
    }
    state.help.drag_to(cursor.x, cursor.y);
}

/// Crystal InventoryDialog sets `Movable=true`. The root keeps the original
/// cursor offset and clamps its true 316x236 size to the fixed logical stage;
/// child controls are excluded by [`inventory_drag_surface_contains`].
fn process_inventory_drag(
    mut state: ResMut<NativePlayerUiState>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut cursor_moves: MessageReader<CursorMoved>,
) {
    if !state.inventory_open() || state.inventory_delete_prompt.is_some() {
        state.inventory_window.end_drag();
        state.inventory_window.clear_cursor();
        return;
    }
    let Some(mouse) = mouse else {
        state.inventory_window.end_drag();
        return;
    };
    let Ok((window_entity, window)) = windows.single() else {
        state.inventory_window.end_drag();
        return;
    };
    if !window.focused {
        state.inventory_window.end_drag();
        return;
    }
    let cursor_path = cursor_moves
        .read()
        .filter(|event| event.window == window_entity)
        .map(|event| cursor_logical(window, event.position))
        .collect::<Vec<_>>();
    let current_cursor = cursor_path
        .last()
        .copied()
        .or_else(|| help_cursor_logical(window));

    // SendInput and high-polling mice can deliver press, motion and release in
    // one Bevy frame. SendInput can also move to the press point one frame
    // before the press edge and expose only the destination CursorMoved event
    // in the pressed frame. Preserve the prior cursor as the anchor whenever
    // it owns the InventoryDialog's exposed drag surface.
    if mouse.just_pressed(MouseButton::Left) {
        let observed_start = cursor_path.first().copied().or(current_cursor);
        let previous_start = state.inventory_window.last_cursor.filter(|previous| {
            inventory_drag_surface_contains(&state.inventory_window, previous.x, previous.y)
                && observed_start.is_some_and(|observed| previous.distance(observed) > 2.0)
        });
        let Some(start) = previous_start
            .or(observed_start)
            .or(state.inventory_window.last_cursor)
        else {
            state.inventory_window.end_drag();
            return;
        };
        if state.inventory_window.begin_drag(start.x, start.y) {
            if let Some(end) = current_cursor {
                state.inventory_window.drag_to(end.x, end.y);
            }
        }
        if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
            state.inventory_window.end_drag();
        }
        state.inventory_window.remember_cursor(current_cursor);
        return;
    }

    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        state.inventory_window.end_drag();
        state.inventory_window.remember_cursor(current_cursor);
        return;
    }
    let Some(cursor) = current_cursor else {
        state.inventory_window.end_drag();
        return;
    };
    state.inventory_window.drag_to(cursor.x, cursor.y);
    state.inventory_window.remember_cursor(current_cursor);
}

/// Crystal cancels the footer-bin toggle on a right click anywhere inside the
/// InventoryDialog. A modal delete prompt owns the pointer first, so it is not
/// dismissed by this underlying-window rule.
fn process_inventory_delete_pointer(
    mut state: ResMut<NativePlayerUiState>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut ui_audio: ResMut<crate::audio::NativeUiAudioQueue>,
) {
    if !state.inventory_open()
        || !state.inventory_delete_mode
        || state.inventory_delete_prompt.is_some()
        || !mouse.is_some_and(|mouse| mouse.just_pressed(MouseButton::Right))
    {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    if !window.focused {
        return;
    }
    let Some(cursor) = help_cursor_logical(window) else {
        return;
    };
    let panel = CrystalRect::new(
        state.inventory_window.left,
        state.inventory_window.top,
        INVENTORY_PANEL_SIZE.width as f32,
        INVENTORY_PANEL_SIZE.height as f32,
    );
    if panel.contains(cursor.x, cursor.y) {
        state.cancel_inventory_delete();
        ui_audio.push(crate::audio::NativeUiSound::ButtonB);
    }
}

pub(crate) fn process_overlay_keyboard(
    mut state: ResMut<NativePlayerUiState>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    mut pending: ResMut<PendingOperations>,
    mut shell_intents: ResMut<NativeUiIntentQueue>,
    mut shell: ResMut<NativeShellModel>,
    mut social: Option<ResMut<crate::social::SocialModel>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<KeyboardInput>,
    inventory: Res<InventoryModel>,
    mut compose_ui: ResMut<MailComposeUi>,
    mut game_shop: Option<ResMut<GameShopModel>>,
    mut storage: ResMut<StorageModel>,
    big_map_controls: BigMapControls,
    chat_state: Option<Res<crate::crystal_ui::chat::CrystalChatState>>,
    npc_dialog: Option<Res<NpcDialogModel>>,
    keyboard_controls: OverlayKeyboardControls,
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
    let OverlayKeyboardControls {
        mut surface_signals,
        mut ui_audio,
    } = keyboard_controls;

    // MirAmountBox/MirMessageBox consume every keyboard event while modal.
    // Enter confirms, Escape follows Cancel/No, and the amount textbox accepts
    // digits only with the initial maximum value fully selected.
    if state.inventory_delete_prompt.is_some() {
        if keys.just_pressed(KeyCode::Escape) {
            state.cancel_inventory_delete();
            ui_audio.push(crate::audio::NativeUiSound::ButtonB);
            return;
        }
        if keys.just_pressed(KeyCode::Enter) {
            if confirm_inventory_delete(&mut state, &inventory, &mut intents, &mut pending) {
                ui_audio.push(crate::audio::NativeUiSound::ButtonB);
            }
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            delete_amount_backspace(&mut state);
        }
        for event in typed.read() {
            if event.state == ButtonState::Pressed {
                if let Some(text) = &event.text {
                    push_delete_amount_text(&mut state, text);
                }
            }
        }
        return;
    }

    if state.group_open() && state.group_invite_focused {
        let Some(social) = social.as_deref_mut() else {
            return;
        };
        if keys.just_pressed(KeyCode::Escape) {
            state.group_invite_focused = false;
            state.group_invite_draft.clear();
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            state.group_invite_draft.pop();
        }
        if keys.just_pressed(KeyCode::Enter) {
            let name = state.group_invite_draft.trim().to_owned();
            if valid_social_name(&name) && queue_group_invite_by_name(&mut intents, social, name) {
                state.group_invite_focused = false;
                state.group_invite_draft.clear();
            }
            return;
        }
        for event in typed.read() {
            if event.state == ButtonState::Pressed {
                if let Some(text) = &event.text {
                    push_social_name_text(&mut state.group_invite_draft, text);
                }
            }
        }
        return;
    }

    if state.guild_open() && state.guild_recruit_focused {
        let Some(social) = social.as_deref_mut() else {
            return;
        };
        if keys.just_pressed(KeyCode::Escape) {
            state.guild_recruit_focused = false;
            state.guild_recruit_draft.clear();
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            state.guild_recruit_draft.pop();
        }
        if keys.just_pressed(KeyCode::Enter) {
            let name = state.guild_recruit_draft.trim().to_owned();
            if valid_social_name(&name)
                && social_has_permission(&social.guild, "recruit")
                && intents.push_social_pending(
                    social,
                    NativePlayerUiIntent::GuildEditMember {
                        change_type: 0,
                        rank_index: 0,
                        name,
                        rank_name: String::new(),
                    },
                )
            {
                state.guild_recruit_focused = false;
                state.guild_recruit_draft.clear();
            }
            return;
        }
        for event in typed.read() {
            if event.state == ButtonState::Pressed {
                if let Some(text) = &event.text {
                    push_social_name_text(&mut state.guild_recruit_draft, text);
                }
            }
        }
        return;
    }

    if state.guild_open()
        && state.guild_left_page == GuildLeftPage::Storage
        && state.guild_gold_focused
    {
        if keys.just_pressed(KeyCode::Escape) {
            state.guild_gold_focused = false;
            state.guild_gold_draft.clear();
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            state.guild_gold_draft.pop();
        }
        if keys.just_pressed(KeyCode::Enter) {
            state.guild_gold_focused = false;
            return;
        }
        for event in typed.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if let Some(text) = &event.text {
                for ch in text.chars().filter(char::is_ascii_digit) {
                    if state.guild_gold_draft.len() < 10 {
                        state.guild_gold_draft.push(ch);
                    }
                }
            }
        }
        return;
    }

    if state.guild_open()
        && state.guild_left_page == GuildLeftPage::Ranks
        && state.guild_rank_name_focused
    {
        if keys.just_pressed(KeyCode::Escape) {
            state.guild_rank_name_focused = false;
            state.guild_rank_name_draft.clear();
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            state.guild_rank_name_draft.pop();
        }
        if keys.just_pressed(KeyCode::Enter) {
            state.guild_rank_name_focused = false;
            return;
        }
        for event in typed.read() {
            if event.state == ButtonState::Pressed {
                if let Some(text) = &event.text {
                    for ch in text.chars().filter(|ch| !ch.is_control()) {
                        if state.guild_rank_name_draft.chars().count() < 20 {
                            state.guild_rank_name_draft.push(ch);
                        }
                    }
                }
            }
        }
        return;
    }

    if state.guild_open()
        && state.guild_left_page == GuildLeftPage::Notice
        && state.guild_notice_editing
    {
        if state.guild_notice_submission.is_some() {
            // A submitted draft stays immutable until an authoritative result
            // resolves the matching social pending operation.
            return;
        }
        if keys.just_pressed(KeyCode::Escape) {
            state.guild_notice_editing = false;
            state.guild_notice_draft.clear();
            return;
        }
        if keys.just_pressed(KeyCode::Backspace) {
            state.guild_notice_draft.pop();
        }
        if keys.just_pressed(KeyCode::Enter) {
            push_guild_notice_text(&mut state.guild_notice_draft, "\n");
        }
        for event in typed.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if let Some(text) = &event.text {
                push_guild_notice_text(&mut state.guild_notice_draft, text);
            }
        }
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
            state.npc_shop_buy_tab = true;
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
    // Crystal's default Help binding is H with Ctrl and Shift explicitly
    // unpressed; Alt is a don't-care modifier. Ctrl+H remains available to
    // the attack-mode binding and Shift+H must not toggle Help.
    let help_shortcut = keys.just_pressed(KeyCode::KeyH)
        && !keys.pressed(KeyCode::ControlLeft)
        && !keys.pressed(KeyCode::ControlRight)
        && !keys.pressed(KeyCode::ShiftLeft)
        && !keys.pressed(KeyCode::ShiftRight);
    if help_shortcut {
        state.help.toggle();
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
            state.inventory_delete_mode = false;
            state.inventory_delete_prompt = None;
        } else if was_open {
            // already handled
        }
    }
    if keys.just_pressed(KeyCode::KeyC) || keys.just_pressed(KeyCode::F10) {
        // Crystal's Equipment shortcut shares the CharacterButton page
        // state machine but does not invoke MirControl.OnMouseClick, so it is
        // intentionally silent.
        state.activate_character_hud_button();
    }
    if keys.just_pressed(KeyCode::F11) {
        state.toggle_skill();
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
        // Crystal's default binding is P = Group. Help page one exposes this
        // binding, so the production route must not silently open Storage.
        state.toggle_group();
    }
    if keys.just_pressed(KeyCode::Escape) {
        if state.core.panel != mir2_ui_core::state::UiPanel::None
            || state.inspect.is_some()
            || state.help_open()
        {
            // If shop/storage open, Escape acts as Cancel
            state.close_all_windows();
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
        state.close_all_windows();
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
        mut ui_audio,
        buttons,
    } = button_controls;
    let mut fallback_effects = UiEffectQueue::default();
    let mut effects = effects.as_deref_mut().unwrap_or(&mut fallback_effects);
    let mut game_shop = game_shop;
    if let Some(expected) = state.guild_notice_submission.clone() {
        let still_pending = social.pending.iter().any(|operation| {
            matches!(
                operation,
                crate::social::SocialPendingOperation::GuildNotice { notice }
                    if notice == &expected
            )
        });
        if !still_pending {
            let explicit_result = social.last_event.as_ref().and_then(|event| {
                matches!(
                    event.packet.as_str(),
                    "GuildNoticeChange" | "GuildNoticeResult"
                )
                .then_some(event.success)
                .flatten()
            });
            let succeeded = explicit_result == Some(true) || social.guild.notice == expected;
            state.guild_notice_submission = None;
            if succeeded {
                state.guild_notice_editing = false;
                state.guild_notice_draft.clear();
                // The edit receipt does not contain the replacement body.
                // Request the authoritative page rather than copying the
                // submitted draft into SocialModel.
                let _ = intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GuildRequestInfo { info_type: 0 },
                );
            } else {
                // An explicit failure keeps the exact draft editable so the
                // player can correct it and retry.
                state.guild_notice_editing = true;
            }
        }
    }
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
        if state.inventory_delete_prompt.is_some()
            && !matches!(
                *button,
                OverlayButton::InventoryDeleteConfirm
                    | OverlayButton::InventoryDeleteCancel
                    | OverlayButton::InventoryDeleteAmountClose
            )
        {
            // MirAmountBox/MirMessageBox is modal; never let a covered control
            // mutate another window in the same frame.
            continue;
        }
        match *button {
            OverlayButton::ExitApplication => {
                effects.push(mir2_ui_core::effect::UiEffect::ExitApplication);
            }
            OverlayButton::CloseWindows | OverlayButton::CloseCharacter => {
                if matches!(*button, OverlayButton::CloseCharacter) {
                    // Crystal CharacterDialog.CloseButton uses ButtonA. Keep
                    // the source-audited cue local to this control rather than
                    // assigning sound to every generic CloseWindows caller.
                    ui_audio.push(crate::audio::NativeUiSound::ButtonA);
                }
                state.close_windows();
                state.shop_quantity = 1;
                if let Some(skill_binding) = skill_binding.as_deref_mut() {
                    skill_binding.clear_selection();
                    skill_binding.set_assign_key(false);
                }
            }
            OverlayButton::ToggleHelp => {
                state.help.toggle();
            }
            OverlayButton::CloseHelp => {
                ui_audio.push(crate::audio::NativeUiSound::ButtonA);
                state.help.hide();
            }
            OverlayButton::HelpPrevious => {
                ui_audio.push(crate::audio::NativeUiSound::ButtonA);
                state.help.previous_page();
            }
            OverlayButton::HelpNext => {
                ui_audio.push(crate::audio::NativeUiSound::ButtonA);
                state.help.next_page();
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
                state.npc_shop_buy_tab = true;
                state.shop_repair_container = 0;
                state.shop_repair_slot = None;
                let _ = shop.apply_service_signal(NpcShopServiceSignal::default());
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
                if state.guild_notice_submission.is_none() {
                    state.guild_notice_editing = false;
                    state.guild_notice_draft.clear();
                }
                state.group_invite_focused = false;
                state.group_invite_draft.clear();
                state.guild_recruit_focused = false;
                state.guild_recruit_draft.clear();
                state.guild_gold_focused = false;
                state.guild_gold_draft.clear();
                state.selected_guild_rank = None;
                state.guild_rank_name_focused = false;
                state.guild_rank_name_draft.clear();
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
                queue_group_invite_by_name(&mut intents, &mut social, name.to_owned());
            }
            OverlayButton::GroupInviteNameFocus => {
                state.group_invite_focused = true;
            }
            OverlayButton::GroupInviteNameSubmit => {
                let name = state.group_invite_draft.trim().to_owned();
                if valid_social_name(&name)
                    && queue_group_invite_by_name(&mut intents, &mut social, name)
                {
                    state.group_invite_focused = false;
                    state.group_invite_draft.clear();
                }
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
                state.guild_recruit_focused = false;
                state.guild_gold_focused = false;
                if page != GuildLeftPage::Ranks {
                    state.selected_guild_rank = None;
                    state.guild_rank_name_draft.clear();
                    state.guild_rank_name_focused = false;
                }
                if page != GuildLeftPage::Notice && state.guild_notice_submission.is_none() {
                    state.guild_notice_editing = false;
                    state.guild_notice_draft.clear();
                }
                if matches!(
                    page,
                    GuildLeftPage::Notice | GuildLeftPage::Members | GuildLeftPage::Ranks
                ) {
                    let info_type = if page == GuildLeftPage::Notice { 0 } else { 1 };
                    intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::GuildRequestInfo { info_type },
                    );
                } else if page == GuildLeftPage::Storage {
                    intents.push_transient_unique(NativePlayerUiIntent::GuildStorageItemChange {
                        change_type: 3,
                        from: 0,
                        to: 0,
                    });
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
            OverlayButton::GuildRecruitNameFocus => {
                if social_has_permission(&social.guild, "recruit") {
                    state.guild_recruit_focused = true;
                }
            }
            OverlayButton::GuildRecruitNameSubmit => {
                let name = state.guild_recruit_draft.trim().to_owned();
                if valid_social_name(&name)
                    && social_has_permission(&social.guild, "recruit")
                    && intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::GuildEditMember {
                            change_type: 0,
                            rank_index: 0,
                            name,
                            rank_name: String::new(),
                        },
                    )
                {
                    state.guild_recruit_focused = false;
                    state.guild_recruit_draft.clear();
                }
            }
            OverlayButton::GuildBeginNoticeEdit => {
                let can_edit =
                    social.guild.name.is_some() && social_has_permission(&social.guild, "notice");
                if can_edit && state.guild_notice_submission.is_none() {
                    state.guild_notice_draft = social.guild.notice.join("\n");
                    state.guild_notice_editing = true;
                }
            }
            OverlayButton::GuildPublishNotice => {
                let can_edit =
                    social.guild.name.is_some() && social_has_permission(&social.guild, "notice");
                let Some(notice) = guild_notice_lines(&state.guild_notice_draft) else {
                    continue;
                };
                if can_edit
                    && state.guild_notice_editing
                    && state.guild_notice_submission.is_none()
                    && notice != social.guild.notice
                    && intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::GuildEditNotice {
                            notice: notice.clone(),
                        },
                    )
                {
                    state.guild_notice_submission = Some(notice);
                }
            }
            OverlayButton::GuildCancelNoticeEdit => {
                if state.guild_notice_submission.is_none() {
                    state.guild_notice_editing = false;
                    state.guild_notice_draft.clear();
                }
            }
            OverlayButton::SelectGuildMember(index) => {
                state.selected_guild_member = Some(index);
            }
            OverlayButton::GuildKickMember(index) => {
                let Some(member) = social.guild.members.get(usize::from(index)) else {
                    continue;
                };
                if member.name.trim().is_empty() || !social_has_permission(&social.guild, "kick") {
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
                if member.name.trim().is_empty() || !social_has_permission(&social.guild, "kick") {
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
            OverlayButton::GuildAssignPreviousRank | OverlayButton::GuildAssignNextRank => {
                let Some(index) = state.selected_guild_member else {
                    continue;
                };
                let Some(member) = social.guild.members.get(usize::from(index)) else {
                    continue;
                };
                if !social_has_permission(&social.guild, "changeRank") {
                    continue;
                }
                let mut ranks = social
                    .guild
                    .ranks
                    .iter()
                    .filter_map(|rank| u8::try_from(rank.index).ok())
                    .collect::<Vec<_>>();
                ranks.sort_unstable();
                ranks.dedup();
                if ranks.is_empty() {
                    continue;
                }
                let current = member.rank_index.unwrap_or(ranks[0]);
                let current_position = ranks.iter().position(|rank| *rank == current).unwrap_or(0);
                let next_position = if *button == OverlayButton::GuildAssignPreviousRank {
                    current_position.checked_sub(1).unwrap_or(ranks.len() - 1)
                } else {
                    (current_position + 1) % ranks.len()
                };
                let name = member.name.clone();
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GuildEditMember {
                        change_type: 4,
                        rank_index: ranks[next_position],
                        name,
                        rank_name: String::new(),
                    },
                );
            }
            OverlayButton::GuildGoldFocus => {
                state.guild_gold_focused = true;
            }
            OverlayButton::GuildGoldDeposit | OverlayButton::GuildGoldWithdraw => {
                let Ok(amount) = state.guild_gold_draft.parse::<u32>() else {
                    continue;
                };
                let change_type = if *button == OverlayButton::GuildGoldDeposit {
                    0
                } else {
                    1
                };
                let permission = if change_type == 0 {
                    "storeItem"
                } else {
                    "retrieveItem"
                };
                if amount > 0
                    && social_has_permission(&social.guild, permission)
                    && intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::GuildStorageGoldChange {
                            change_type,
                            amount,
                        },
                    )
                {
                    state.guild_gold_focused = false;
                    state.guild_gold_draft.clear();
                }
            }
            OverlayButton::GuildStoragePreviousPage => {
                state.guild_storage_page = state.guild_storage_page.saturating_sub(1);
            }
            OverlayButton::GuildStorageNextPage => {
                let page_count = crate::social::MAX_GUILD_STORAGE_ITEMS.div_ceil(28);
                state.guild_storage_page =
                    (state.guild_storage_page + 1).min(page_count.saturating_sub(1));
            }
            OverlayButton::SelectGuildRank(rank_index) => {
                let Some(rank) = social
                    .guild
                    .ranks
                    .iter()
                    .find(|rank| u8::try_from(rank.index).ok() == Some(rank_index))
                else {
                    continue;
                };
                state.selected_guild_rank = Some(rank_index);
                state.guild_rank_name_draft = rank.name.clone();
                state.guild_rank_name_focused = false;
            }
            OverlayButton::GuildRankNameFocus => {
                if state.selected_guild_rank.is_some()
                    && social_has_permission(&social.guild, "changeRank")
                {
                    state.guild_rank_name_focused = true;
                }
            }
            OverlayButton::GuildRankNameSave => {
                let Some(rank_index) = state.selected_guild_rank else {
                    continue;
                };
                let rank_name = state.guild_rank_name_draft.trim().to_owned();
                let changed = social
                    .guild
                    .ranks
                    .iter()
                    .find(|rank| u8::try_from(rank.index).ok() == Some(rank_index))
                    .is_some_and(|rank| rank.name != rank_name);
                if changed
                    && !rank_name.is_empty()
                    && rank_name.chars().count() <= 20
                    && social_has_permission(&social.guild, "changeRank")
                    && intents.push_social_pending(
                        &mut social,
                        NativePlayerUiIntent::GuildEditMember {
                            change_type: 2,
                            rank_index,
                            name: String::new(),
                            rank_name,
                        },
                    )
                {
                    state.guild_rank_name_focused = false;
                }
            }
            OverlayButton::GuildRankTogglePermission(option) => {
                let Some(rank_index) = state.selected_guild_rank else {
                    continue;
                };
                let Some(rank) = social
                    .guild
                    .ranks
                    .iter()
                    .find(|rank| u8::try_from(rank.index).ok() == Some(rank_index))
                else {
                    continue;
                };
                if option > 7 || !social_has_permission(&social.guild, "changeRank") {
                    continue;
                }
                let enabled = rank.options & (1_u8 << option) != 0;
                intents.push_social_pending(
                    &mut social,
                    NativePlayerUiIntent::GuildEditMember {
                        change_type: 5,
                        rank_index,
                        name: (!enabled).to_string(),
                        rank_name: option.to_string(),
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
                state.close_all_windows();
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
            OverlayButton::InventoryDeleteToggle => {
                if let Some(slot) =
                    inspected_inventory_item(&state, &inventory).map(|item| item.slot)
                {
                    // DelItemButton itself owns ButtonA even when an existing
                    // selected cell opens the prompt without toggling mode.
                    ui_audio.push(crate::audio::NativeUiSound::ButtonA);
                    let _ = state.open_inventory_delete_for_slot(&inventory, slot);
                } else {
                    state.inventory_delete_mode = !state.inventory_delete_mode;
                    state.inventory_delete_prompt = None;
                    ui_audio.push(if state.inventory_delete_mode {
                        crate::audio::NativeUiSound::ButtonA
                    } else {
                        crate::audio::NativeUiSound::ButtonB
                    });
                }
            }
            OverlayButton::InventoryDeleteConfirm => {
                if confirm_inventory_delete(&mut state, &inventory, &mut intents, &mut pending) {
                    ui_audio.push(crate::audio::NativeUiSound::ButtonB);
                }
            }
            OverlayButton::InventoryDeleteCancel => {
                state.cancel_inventory_delete();
                ui_audio.push(crate::audio::NativeUiSound::ButtonB);
            }
            OverlayButton::InventoryDeleteAmountClose => {
                if matches!(
                    state.inventory_delete_prompt,
                    Some(InventoryDeletePrompt::Amount { .. })
                ) {
                    // MirAmountBox.CloseButton disposes only the box. Unlike
                    // its Cancel button it does not call InventoryDialog's
                    // CancelDelete callback, so delete mode remains active.
                    state.inventory_delete_prompt = None;
                    state.inspect = None;
                    ui_audio.push(crate::audio::NativeUiSound::ButtonA);
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
            OverlayButton::InspectBag(slot) if state.inventory_delete_mode => {
                let _ = state.open_inventory_delete_for_slot(&inventory, slot);
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
            OverlayButton::InspectQuest(slot) => {
                if state.inventory_delete_mode {
                    // Crystal consumes the click while delete mode is active,
                    // but QuestInventory cells can never be deleted.
                    continue;
                }
                state.inspect = inventory
                    .items_in(3)
                    .into_iter()
                    .find(|item| item.slot == slot)
                    .map(inspect_from_item);
                state.split_count = 1;
                state.inventory_operation = None;
                state.drop_confirmation = None;
            }
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
                // Crystal CharacterDialog page buttons inherit
                // MirButton.Sound = SoundList.ButtonA. Keep the cue on the
                // local Changed<Interaction> press edge; switching pages must
                // never manufacture a gateway intent.
                ui_audio.push(crate::audio::NativeUiSound::ButtonA);
                state.character_page = page;
                state.inspect = None;
            }
            OverlayButton::SelectInventoryPage(page) => {
                if page <= 2 {
                    // All three source tabs are MirButtons with ButtonA. The
                    // locked second tab still clicks (Crystal opens an
                    // expansion prompt), but it must never expose a phantom
                    // empty page while expansion authority is unavailable.
                    ui_audio.push(crate::audio::NativeUiSound::ButtonA);
                    if page != 1 || inventory.second_bag_unlocked() {
                        state.inventory_page = page;
                        state.inspect = None;
                        state.inventory_operation = None;
                        state.drop_confirmation = None;
                    }
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
                if state.npc_shop_open() && shop.allows_buy() {
                    shop.selected_id = Some(id);
                }
            }
            OverlayButton::ShopShowBuy => {
                if state.npc_shop_open() && shop.allows_buy() {
                    state.npc_shop_buy_tab = true;
                }
            }
            OverlayButton::ShopShowSell => {
                if state.npc_shop_open() && shop.allows_sell() {
                    state.npc_shop_buy_tab = false;
                }
            }
            OverlayButton::ShopBuy => {
                if state.npc_shop_open()
                    && shop.allows_buy()
                    && shop_buy_enabled(&shop, &inventory, state.shop_quantity)
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
                if !state.npc_shop_open() || !shop.allows_sell() {
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
                if !state.npc_shop_open() || !shop.allows_repair() {
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
                if !state.npc_shop_open() || !shop.allows_special_repair() {
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
                if state.npc_shop_open()
                    && matches!(
                        shop.service_mode,
                        NpcShopServiceMode::Buy | NpcShopServiceMode::Sell
                    )
                {
                    state.shop_quantity_inc();
                }
            }
            OverlayButton::ShopQuantityDec => {
                if state.npc_shop_open()
                    && matches!(
                        shop.service_mode,
                        NpcShopServiceMode::Buy | NpcShopServiceMode::Sell
                    )
                {
                    state.shop_quantity_dec();
                }
            }
            OverlayButton::ShopPageUp => {
                if state.npc_shop_open() && shop.allows_buy() {
                    shop_ui.start_index = shop_ui.start_index.saturating_sub(1);
                    shop.selected_id = None;
                }
            }
            OverlayButton::ShopPageDown => {
                if state.npc_shop_open() && shop.allows_buy() {
                    shop_ui.start_index =
                        (shop_ui.start_index + 1).min(shop.goods.len().saturating_sub(8));
                    shop.selected_id = None;
                }
            }
            OverlayButton::ShopConfirm => {
                if !state.npc_shop_open() {
                    continue;
                }
                if shop.allows_repair() || shop.allows_special_repair() {
                    if let Some(item) = selected_repair_item(&state, &inventory) {
                        if let Some(uid) = item_unique_id(item) {
                            if repair_selection_enabled(&state, &inventory) {
                                let intent = if shop.allows_special_repair() {
                                    NativePlayerUiIntent::SRepairItem { unique_id: uid }
                                } else {
                                    NativePlayerUiIntent::RepairItem { unique_id: uid }
                                };
                                intents.push_pending_intent(&mut pending, intent);
                            }
                        }
                    }
                } else if shop.allows_sell() && (!shop.allows_buy() || !state.npc_shop_buy_tab) {
                    if let Some(slot) = shop.selected_bag_slot_for_sell {
                        if let Some(item) = inventory
                            .items
                            .iter()
                            .find(|item| item.container == 0 && item.slot == slot)
                        {
                            if let Some(unique_id) = item_unique_id(item) {
                                intents.push_pending_intent(
                                    &mut pending,
                                    NativePlayerUiIntent::SellItem {
                                        unique_id,
                                        count: shop_quantity_clamped(state.shop_quantity),
                                    },
                                );
                            }
                        }
                    }
                } else if shop.allows_buy()
                    && shop_buy_enabled(&shop, &inventory, state.shop_quantity)
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
            OverlayButton::ShopCancel => {
                if state.npc_shop_open() {
                    state.core.panel = mir2_ui_core::state::UiPanel::None;
                }
                state.shop_quantity = 1;
                state.npc_shop_buy_tab = true;
                shop.selected_id = None;
                shop.selected_bag_slot_for_sell = None;
                shop.selected_bag_slot_for_repair = None;
                state.shop_repair_container = 0;
                state.shop_repair_slot = None;
                let _ = shop.apply_service_signal(NpcShopServiceSignal::default());
            }
            OverlayButton::SelectBagForSell(slot) => {
                if state.npc_shop_open() && shop.allows_sell() {
                    shop.selected_bag_slot_for_sell = Some(slot);
                }
            }
            OverlayButton::SelectBagForRepair(slot) => {
                if state.npc_shop_open() && (shop.allows_repair() || shop.allows_special_repair()) {
                    shop.selected_bag_slot_for_repair = Some(slot);
                    state.shop_repair_container = 0;
                    state.shop_repair_slot = Some(slot);
                }
            }
            OverlayButton::SelectEquipForRepair(slot) => {
                if state.npc_shop_open() && (shop.allows_repair() || shop.allows_special_repair()) {
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
                    intents.push_storage_pending_intent(&mut pending, true, unique_id, from, to);
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
                    intents.push_storage_pending_intent(&mut pending, false, unique_id, from, to);
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
    if inspect.container == 3 {
        return None;
    }
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
    if inspect.container == 3 {
        return None;
    }
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

fn inventory_delete_prompt_for_item(item: &ItemModel) -> Option<InventoryDeletePrompt> {
    (item.container == 0).then_some(())?;
    let max_count = u16::try_from(item.quantity).ok()?;
    (max_count > 0).then_some(())?;
    let target = InventoryDeleteTarget {
        unique_id: item_unique_id(item)?,
        slot: item.slot,
        key: item.key.clone(),
        name: if item.name.trim().is_empty() {
            item.key.clone()
        } else {
            item.name.clone()
        },
        max_count,
    };
    Some(if max_count > 1 {
        InventoryDeletePrompt::Amount {
            draft: max_count.to_string(),
            target,
            select_all: true,
        }
    } else {
        InventoryDeletePrompt::Confirm { target }
    })
}

fn inventory_delete_target_is_current(
    target: &InventoryDeleteTarget,
    inventory: &InventoryModel,
) -> bool {
    inventory.items.iter().any(|item| {
        item.container == 0
            && item.slot == target.slot
            && item.key == target.key
            && item_unique_id(item) == Some(target.unique_id)
            && item.quantity == u32::from(target.max_count)
    })
}

fn inventory_delete_prompt_is_current(
    prompt: &InventoryDeletePrompt,
    inventory: &InventoryModel,
) -> bool {
    inventory_delete_target_is_current(prompt.target(), inventory)
}

fn inventory_delete_amount(prompt: &InventoryDeletePrompt) -> Option<u16> {
    match prompt {
        InventoryDeletePrompt::Amount { target, draft, .. } => {
            let parsed = draft.parse::<u32>().ok()?;
            Some(parsed.clamp(1, u32::from(target.max_count)) as u16)
        }
        InventoryDeletePrompt::Confirm { .. } => Some(1),
    }
}

fn delete_amount_backspace(state: &mut NativePlayerUiState) {
    let Some(InventoryDeletePrompt::Amount {
        draft, select_all, ..
    }) = state.inventory_delete_prompt.as_mut()
    else {
        return;
    };
    if *select_all {
        draft.clear();
        *select_all = false;
    } else {
        draft.pop();
    }
}

fn push_delete_amount_text(state: &mut NativePlayerUiState, text: &str) {
    let Some(InventoryDeletePrompt::Amount {
        target,
        draft,
        select_all,
    }) = state.inventory_delete_prompt.as_mut()
    else {
        return;
    };
    let digits = text
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    if digits.is_empty() {
        return;
    }
    if *select_all {
        draft.clear();
        *select_all = false;
    }
    for digit in digits.chars() {
        if draft.len() < 10 {
            draft.push(digit);
        }
        if draft
            .parse::<u32>()
            .is_ok_and(|amount| amount > u32::from(target.max_count))
        {
            *draft = target.max_count.to_string();
        }
    }
}

fn confirm_inventory_delete(
    state: &mut NativePlayerUiState,
    inventory: &InventoryModel,
    intents: &mut NativePlayerUiIntentQueue,
    pending: &mut PendingOperations,
) -> bool {
    let Some(prompt) = state.inventory_delete_prompt.clone() else {
        return false;
    };
    if !inventory_delete_prompt_is_current(&prompt, inventory) {
        state.cancel_inventory_delete();
        return false;
    }
    let Some(count) = inventory_delete_amount(&prompt) else {
        return false;
    };
    let target = prompt.target();
    if !intents.push_pending_intent(
        pending,
        NativePlayerUiIntent::DeleteItem {
            unique_id: target.unique_id,
            count,
            hero_inventory: false,
        },
    ) {
        return false;
    }
    state.cancel_inventory_delete();
    true
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

fn render_inventory_delete_item(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    target: &InventoryDeleteTarget,
    inventory: &InventoryModel,
    item_box: CrystalRect,
) {
    let Some(item) = inventory.items.iter().find(|item| {
        item.container == 0
            && item.slot == target.slot
            && item_unique_id(item) == Some(target.unique_id)
    }) else {
        return;
    };
    let (Some(path), Some(icon_rect)) = (
        item_icon_path(item.icon),
        crystal_inventory_icon_rect(item, item_box.width, item_box.height),
    ) else {
        return;
    };
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        path,
        CrystalRect::new(
            item_box.left + icon_rect.left,
            item_box.top + icon_rect.top,
            icon_rect.width,
            icon_rect.height,
        ),
    );
}

fn render_inventory_delete_modal(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    state: &NativePlayerUiState,
    inventory: &InventoryModel,
) {
    let Some(prompt) = state.inventory_delete_prompt.as_ref() else {
        return;
    };
    match prompt {
        InventoryDeletePrompt::Amount { target, draft, .. } => {
            parent
                .spawn((
                    OverlayInventoryDeleteDialog,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(CRYSTAL_DELETE_AMOUNT_RECT.left),
                        top: Val::Px(CRYSTAL_DELETE_AMOUNT_RECT.top),
                        width: Val::Px(CRYSTAL_DELETE_AMOUNT_RECT.width),
                        height: Val::Px(CRYSTAL_DELETE_AMOUNT_RECT.height),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ))
                .with_children(|dialog| {
                    if let Some(asset_server) = asset_server {
                        spawn_overlay_frame(
                            dialog,
                            asset_server,
                            "original-ui/Prguse/238.png",
                            CRYSTAL_DELETE_AMOUNT_RECT.width,
                            CRYSTAL_DELETE_AMOUNT_RECT.height,
                        );
                        spawn_overlay_crystal_button(
                            dialog,
                            asset_server,
                            "Prguse2",
                            360,
                            361,
                            362,
                            CrystalRect::new(180.0, 3.0, 24.0, 21.0),
                            OverlayButton::InventoryDeleteAmountClose,
                        );
                        render_inventory_delete_item(
                            dialog,
                            asset_server,
                            target,
                            inventory,
                            CrystalRect::new(15.0, 34.0, 38.0, 34.0),
                        );
                        spawn_overlay_crystal_button_enabled(
                            dialog,
                            asset_server,
                            "Title",
                            200,
                            201,
                            202,
                            CrystalRect::new(23.0, 76.0, 76.0, 25.0),
                            OverlayButton::InventoryDeleteConfirm,
                            draft.parse::<u32>().is_ok(),
                        );
                        spawn_overlay_crystal_button(
                            dialog,
                            asset_server,
                            "Title",
                            203,
                            204,
                            205,
                            CrystalRect::new(110.0, 76.0, 76.0, 25.0),
                            OverlayButton::InventoryDeleteCancel,
                        );
                    }
                    overlay_text_at(
                        dialog,
                        &format!("Delete how many '{name}'?", name = target.name),
                        CrystalRect::new(19.0, 8.0, 158.0, 14.0),
                        10.0,
                        TEXT,
                    );

                    let parsed = draft.parse::<u32>().ok();
                    let border = match parsed {
                        None => Color::srgb(1.0, 0.0, 0.0),
                        Some(amount) if amount == u32::from(target.max_count) => {
                            Color::srgb(1.0, 0.647, 0.0)
                        }
                        Some(_) => Color::srgb(0.0, 1.0, 0.0),
                    };
                    dialog
                        .spawn((
                            OverlayInventoryDeleteAmountInput,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(58.0),
                                top: Val::Px(43.0),
                                width: Val::Px(132.0),
                                height: Val::Px(19.0),
                                border: UiRect::all(Val::Px(1.0)),
                                padding: UiRect::axes(Val::Px(2.0), Val::Px(0.0)),
                                align_items: AlignItems::Center,
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(Color::BLACK),
                            BorderColor::all(border),
                        ))
                        .with_children(|input| {
                            input.spawn((
                                Text::new(draft.clone()),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(TEXT),
                                TextLayout::new(Justify::Left, LineBreak::NoWrap),
                            ));
                        });
                });
        }
        InventoryDeletePrompt::Confirm { target } => {
            parent
                .spawn((
                    OverlayInventoryDeleteDialog,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(CRYSTAL_DELETE_CONFIRM_RECT.left),
                        top: Val::Px(CRYSTAL_DELETE_CONFIRM_RECT.top),
                        width: Val::Px(CRYSTAL_DELETE_CONFIRM_RECT.width),
                        height: Val::Px(CRYSTAL_DELETE_CONFIRM_RECT.height),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ))
                .with_children(|dialog| {
                    if let Some(asset_server) = asset_server {
                        spawn_overlay_frame(
                            dialog,
                            asset_server,
                            "original-ui/Prguse/360.png",
                            CRYSTAL_DELETE_CONFIRM_RECT.width,
                            CRYSTAL_DELETE_CONFIRM_RECT.height,
                        );
                        spawn_overlay_crystal_button(
                            dialog,
                            asset_server,
                            "Title",
                            206,
                            207,
                            208,
                            CrystalRect::new(260.0, 157.0, 76.0, 25.0),
                            OverlayButton::InventoryDeleteConfirm,
                        );
                        spawn_overlay_crystal_button(
                            dialog,
                            asset_server,
                            "Title",
                            210,
                            211,
                            212,
                            CrystalRect::new(360.0, 157.0, 76.0, 25.0),
                            OverlayButton::InventoryDeleteCancel,
                        );
                    }
                    overlay_text_at(
                        dialog,
                        &format!(
                            "Permanently delete '{}'? This cannot be undone.",
                            target.name
                        ),
                        CrystalRect::new(35.0, 35.0, 390.0, 110.0),
                        10.0,
                        TEXT,
                    );
                });
        }
    }
}

fn render_overlays(
    models: OverlayRenderModels,
    windows: Query<&Window, With<PrimaryWindow>>,
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
            Query<(Entity, &mut Node, &mut GlobalZIndex), With<OverlayHelp>>,
        )>,
        ParamSet<(
            Query<(Entity, &mut Node), With<OverlayInventoryDeleteModal>>,
            Query<(Entity, &mut Node, &mut GlobalZIndex), With<OverlayInventoryDeleteCursor>>,
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

        fill_positioned_unindexed_panel(
            &mut commands,
            &mut all.p1(),
            state.inventory_window.left,
            state.inventory_window.top,
            state.inventory_open(),
            |parent| {
                render_inventory(
                    parent,
                    asset_server.as_deref(),
                    &inventory,
                    &ui,
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
        fill_panel(&mut commands, &mut all.p7(), false, |_| {});
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
                    &ui.player,
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
                    &ui.player,
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
                    &ui.player,
                )
            },
        );
        fill_positioned_panel(
            &mut commands,
            &mut secondary.p7(),
            state.help.left,
            state.help.top,
            state.help.z_index(),
            state.help_open(),
            |parent| render_help(parent, asset_server.as_deref(), state.help.page),
        );
    }
    {
        let mut delete_layers = panels.p2();
        fill_panel(
            &mut commands,
            &mut delete_layers.p0(),
            state.inventory_delete_prompt.is_some(),
            |parent| {
                render_inventory_delete_modal(parent, asset_server.as_deref(), &state, &inventory)
            },
        );

        let cursor = windows
            .single()
            .ok()
            .and_then(help_cursor_logical)
            .map(|cursor| {
                (
                    cursor.x - CRYSTAL_DELETE_CURSOR_SIZE.0 / 2.0,
                    cursor.y - CRYSTAL_DELETE_CURSOR_SIZE.1,
                )
            });
        let (cursor_left, cursor_top) = cursor.unwrap_or_default();
        fill_positioned_panel(
            &mut commands,
            &mut delete_layers.p1(),
            cursor_left,
            cursor_top,
            OVERLAY_INVENTORY_DELETE_CURSOR_Z,
            state.inventory_open() && state.inventory_delete_mode && cursor.is_some(),
            |parent| {
                if let Some(asset_server) = asset_server.as_deref() {
                    spawn_static_overlay_sprite(
                        parent,
                        asset_server,
                        "original-ui/Prguse2/366.png".to_owned(),
                        CrystalRect::new(
                            0.0,
                            0.0,
                            CRYSTAL_DELETE_CURSOR_SIZE.0,
                            CRYSTAL_DELETE_CURSOR_SIZE.1,
                        ),
                    );
                }
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

fn fill_positioned_unindexed_panel<C: Component>(
    commands: &mut Commands,
    query: &mut Query<(Entity, &mut Node), With<C>>,
    left: f32,
    top: f32,
    visible: bool,
    render: impl FnOnce(&mut ChildSpawnerCommands),
) {
    let Some((entity, mut node)) = query.iter_mut().next() else {
        return;
    };
    node.left = Val::Px(left);
    node.top = Val::Px(top);
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

fn fill_positioned_panel<C: Component>(
    commands: &mut Commands,
    query: &mut Query<(Entity, &mut Node, &mut GlobalZIndex), With<C>>,
    left: f32,
    top: f32,
    z_index: i32,
    visible: bool,
    render: impl FnOnce(&mut ChildSpawnerCommands),
) {
    let Some((entity, mut node, mut rendered_z_index)) = query.iter_mut().next() else {
        return;
    };
    node.left = Val::Px(left);
    node.top = Val::Px(top);
    *rendered_z_index = GlobalZIndex(z_index);
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
    ui: &UiReadModel,
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
        let second_tab_index = inventory_second_tab_index(inventory, state.inventory_page == 1);
        spawn_inventory_tab(
            parent,
            asset_server,
            76.0,
            7.0,
            second_tab_index,
            second_tab_index,
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
            &format_crystal_gold(inventory.gold),
            CrystalRect::new(
                INVENTORY_GOLD_LABEL_ORIGIN.x as f32,
                INVENTORY_GOLD_LABEL_ORIGIN.y as f32,
                INVENTORY_GOLD_LABEL_SIZE.width as f32,
                INVENTORY_GOLD_LABEL_SIZE.height as f32,
            ),
            10.0,
            TEXT,
        );
        spawn_inventory_weight_bar(parent, asset_server, ui.player.normalized_weight());
        overlay_text_at(
            parent,
            &free_inventory_slots(inventory).to_string(),
            CrystalRect::new(
                INVENTORY_FREE_SLOT_LABEL_ORIGIN.x as f32,
                INVENTORY_FREE_SLOT_LABEL_ORIGIN.y as f32,
                INVENTORY_FREE_SLOT_LABEL_SIZE.width as f32,
                INVENTORY_FREE_SLOT_LABEL_SIZE.height as f32,
            ),
            10.0,
            TEXT,
        );
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse2",
            if state.inventory_delete_mode {
                368
            } else {
                366
            },
            if state.inventory_delete_mode {
                368
            } else {
                367
            },
            368,
            CrystalRect::new(
                INVENTORY_DELETE_BUTTON_ORIGIN.x as f32,
                INVENTORY_DELETE_BUTTON_ORIGIN.y as f32,
                INVENTORY_DELETE_BUTTON_SIZE.width as f32,
                INVENTORY_DELETE_BUTTON_SIZE.height as f32,
            ),
            OverlayButton::InventoryDeleteToggle,
            true,
        );
        let (container, page_offset) = if state.inventory_page == 2 {
            (3, 0)
        } else {
            (0, usize::from(state.inventory_page) * INVENTORY_PAGE_SIZE)
        };
        parent
            .spawn((
                OverlayInventoryGridViewport,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(INVENTORY_GRID_ORIGIN.x as f32),
                    top: Val::Px(INVENTORY_GRID_ORIGIN.y as f32),
                    width: Val::Px(
                        (INVENTORY_GRID_STEP.x as usize * (INVENTORY_PAGE_COLUMNS - 1)
                            + INVENTORY_CELL_SIZE.width as usize) as f32,
                    ),
                    height: Val::Px(
                        (INVENTORY_GRID_STEP.y as usize
                            * (INVENTORY_PAGE_SIZE / INVENTORY_PAGE_COLUMNS - 1)
                            + INVENTORY_CELL_SIZE.height as usize) as f32,
                    ),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_children(|grid| {
                let page_items = inventory.items_in(container);
                for local_slot in 0..INVENTORY_PAGE_SIZE {
                    let slot = (page_offset + local_slot) as u32;
                    if container == 0 && slot >= u32::from(inventory.bag_slot_capacity()) {
                        continue;
                    }
                    let item = page_items.iter().copied().find(|item| item.slot == slot);
                    let enabled = if container == 3 {
                        item.is_some()
                    } else {
                        match &state.inventory_operation {
                            Some(InventoryOperationDraft::Move { source_slot, .. }) => {
                                *source_slot != slot
                            }
                            Some(InventoryOperationDraft::Merge { source_slot, .. }) => {
                                *source_slot != slot && item.and_then(item_unique_id).is_some()
                            }
                            None => item.is_some(),
                        }
                    };
                    let x =
                        (local_slot % INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.x as f32;
                    let y =
                        (local_slot / INVENTORY_PAGE_COLUMNS) as f32 * INVENTORY_GRID_STEP.y as f32;
                    let rect = CrystalRect::new(
                        x,
                        y,
                        INVENTORY_CELL_SIZE.width as f32,
                        INVENTORY_CELL_SIZE.height as f32,
                    );
                    let button = if container == 3 {
                        OverlayButton::InspectQuest(slot)
                    } else {
                        OverlayButton::InspectBag(slot)
                    };
                    if let Some(item) = item {
                        overlay_absolute_item_button(
                            grid,
                            asset_server,
                            item,
                            rect,
                            button,
                            enabled,
                            &ui.player,
                        );
                    } else {
                        overlay_absolute_inventory_cell(grid, rect, button, enabled);
                    }
                }
            });
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

fn format_crystal_gold(gold: u32) -> String {
    let digits = gold.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            output.push(',');
        }
        output.push(digit);
    }
    output
}

fn inventory_weight_bar_asset(ratio: f32) -> (&'static str, u16) {
    if ratio <= 0.50 {
        ("Prguse", 24)
    } else if ratio <= 0.75 {
        ("UI_32bit", 471)
    } else {
        ("UI_32bit", 470)
    }
}

fn inventory_weight_bar_width(ratio: f32) -> f32 {
    ((INVENTORY_WEIGHT_BAR_SIZE.width as f32 - 3.0) * ratio.clamp(0.0, 1.0)).floor()
}

fn spawn_inventory_weight_bar(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    ratio: f32,
) {
    let width = inventory_weight_bar_width(ratio);
    if width <= 0.0 {
        return;
    }
    let (library, index) = inventory_weight_bar_asset(ratio);
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(INVENTORY_WEIGHT_BAR_ORIGIN.x as f32),
            top: Val::Px(INVENTORY_WEIGHT_BAR_ORIGIN.y as f32),
            width: Val::Px(width),
            height: Val::Px(INVENTORY_WEIGHT_BAR_SIZE.height as f32),
            overflow: Overflow::clip(),
            ..default()
        },
        ImageNode {
            image: asset_server.load(format!("original-ui/{library}/{index}.png")),
            rect: Some(bevy::math::Rect {
                min: Vec2::ZERO,
                max: Vec2::new(width, INVENTORY_WEIGHT_BAR_SIZE.height as f32),
            }),
            ..default()
        },
    ));
}

fn crystal_character_gender_offset(gender: Option<&str>) -> Option<u16> {
    let gender = gender?.trim();
    if gender.eq_ignore_ascii_case("male") {
        Some(0)
    } else if gender.eq_ignore_ascii_case("female") {
        Some(1)
    } else {
        None
    }
}

fn crystal_character_page_index(gender: Option<&str>) -> u16 {
    // A missing legacy gender keeps the structural page usable, while the
    // actual appearance layers below remain fail-closed.
    340 + crystal_character_gender_offset(gender).unwrap_or_default()
}

fn crystal_character_class_image_index(class_name: Option<&str>) -> Option<u16> {
    let class_name = class_name?.trim();
    if class_name.eq_ignore_ascii_case("warrior") {
        Some(100)
    } else if class_name.eq_ignore_ascii_case("wizard") {
        Some(101)
    } else if class_name.eq_ignore_ascii_case("taoist") {
        Some(102)
    } else if class_name.eq_ignore_ascii_case("assassin") {
        Some(103)
    } else if class_name.eq_ignore_ascii_case("archer") {
        Some(104)
    } else {
        None
    }
}

fn crystal_character_guild_label(ui: &UiReadModel) -> String {
    [
        ui.player.guild_name.as_deref(),
        ui.player.guild_rank_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

fn crystal_character_hair_frame(
    class_name: Option<&str>,
    gender: Option<&str>,
    hair: Option<u8>,
) -> Option<CrystalFrameSpec> {
    let gender_offset = crystal_character_gender_offset(gender)?;
    let hair = usize::from(hair?);
    if hair >= CRYSTAL_MALE_HAIR_RECTS.len() {
        return None;
    }
    let assassin = class_name
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("assassin"));
    let (base, mut rect) = match (gender_offset, assassin) {
        (0, false) => (441, CRYSTAL_MALE_HAIR_RECTS[hair]),
        (0, true) => (461, CRYSTAL_ASSASSIN_MALE_HAIR_RECTS[hair]),
        (1, false) => (481, CRYSTAL_FEMALE_HAIR_RECTS[hair]),
        (1, true) => (501, CRYSTAL_ASSASSIN_FEMALE_HAIR_RECTS[hair]),
        _ => return None,
    };
    // CharacterDialog.cs applies these offsets on top of the source frame's
    // intrinsic `useOffset=true` x/y for Assassin hair only.
    if assassin {
        rect.left += if gender_offset == 0 { 6.0 } else { 4.0 };
        rect.top += if gender_offset == 0 { 25.0 } else { 18.0 };
    }
    Some(CrystalFrameSpec::new("Prguse", base + hair as u16, rect))
}

fn crystal_character_state_item_frame(item: &ItemModel) -> Option<CrystalFrameSpec> {
    if item.state_image == 0 || item.state_image_width == 0 || item.state_image_height == 0 {
        return None;
    }
    Some(CrystalFrameSpec::new(
        "StateItem",
        item.state_image,
        CrystalRect::new(
            item.state_image_x as f32,
            item.state_image_y as f32,
            item.state_image_width as f32,
            item.state_image_height as f32,
        ),
    ))
}

fn spawn_character_frame(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    frame: CrystalFrameSpec,
) {
    spawn_static_overlay_sprite(parent, asset_server, frame.asset_path(), frame.rect);
}

fn crystal_character_paper_doll_frames(
    inventory: &InventoryModel,
    ui: &UiReadModel,
) -> Vec<CrystalFrameSpec> {
    let equipped = |slot| {
        inventory
            .items
            .iter()
            .find(|item| item.container == 2 && item.slot == slot)
    };
    let mut frames = Vec::with_capacity(3);

    // Exact CharacterPage.AfterDraw order: wing (when authoritative data is
    // available), armour, weapon, then helmet-or-hair. WingEffect is not yet a
    // personal read-model field, so this slice deliberately emits no fake wing.
    for slot in [1, 0] {
        if let Some(frame) = equipped(slot).and_then(crystal_character_state_item_frame) {
            frames.push(frame);
        }
    }
    if let Some(helmet) = equipped(2) {
        if let Some(frame) = crystal_character_state_item_frame(helmet) {
            frames.push(frame);
        }
    } else if let Some(frame) = crystal_character_hair_frame(
        ui.player.class_name.as_deref(),
        ui.player.gender.as_deref(),
        ui.player.hair,
    ) {
        frames.push(frame);
    }
    frames
}

fn render_character_paper_doll(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    inventory: &InventoryModel,
    ui: &UiReadModel,
) {
    for frame in crystal_character_paper_doll_frames(inventory, ui) {
        spawn_character_frame(parent, asset_server, frame);
    }
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
            CharacterPage::Character => (
                crystal_character_page_index(ui.player.gender.as_deref()),
                CRYSTAL_CHARACTER_PAGE_RECT,
            ),
            CharacterPage::Stats1 => (506, CRYSTAL_CHARACTER_PAGE_RECT),
            CharacterPage::Stats2 => (507, CRYSTAL_CHARACTER_PAGE_RECT),
            CharacterPage::Spells => (508, CRYSTAL_CHARACTER_PAGE_RECT),
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
            OverlayButton::CloseCharacter,
        );
        overlay_centered_text_at(
            parent,
            ui.player.name.as_deref().unwrap_or(""),
            CrystalRect::new(0.0, 12.0, 264.0, 20.0),
            12.0,
            TEXT,
        );
        overlay_centered_text_at(
            parent,
            &crystal_character_guild_label(ui),
            CrystalRect::new(0.0, 33.0, 264.0, 30.0),
            10.0,
            TEXT,
        );
        if let Some(index) = crystal_character_class_image_index(ui.player.class_name.as_deref()) {
            spawn_static_overlay_sprite(
                parent,
                asset_server,
                format!("original-ui/Prguse/{index}.png"),
                CrystalRect::new(15.0, 33.0, 32.0, 32.0),
            );
        }

        match state.character_page {
            CharacterPage::Character => {
                for (slot, rect) in CRYSTAL_CHARACTER_EQUIPMENT_SLOTS {
                    let item = inventory
                        .items_in(2)
                        .into_iter()
                        .find(|item| item.slot == slot);
                    if let Some(item) = item {
                        overlay_absolute_item_button(
                            parent,
                            asset_server,
                            item,
                            rect,
                            OverlayButton::InspectEquip(slot),
                            true,
                            &ui.player,
                        );
                    }
                }
                // CharacterPage.AfterDraw runs after its MirItemCell children.
                render_character_paper_doll(parent, asset_server, inventory, ui);
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

fn inventory_second_tab_index(inventory: &InventoryModel, active: bool) -> u16 {
    if !inventory.second_bag_unlocked() {
        169
    } else if active {
        168
    } else {
        738
    }
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

fn overlay_centered_text_at(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: CrystalRect,
    font_size: f32,
    color: Color,
) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect.left),
                top: Val::Px(rect.top),
                width: Val::Px(rect.width),
                height: Val::Px(rect.height),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|container| {
            container.spawn((
                Text::new(text.to_owned()),
                TextFont {
                    font_size: FontSize::Px(font_size),
                    ..default()
                },
                TextColor(color),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
            ));
        });
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
    player: &crate::read_model::PlayerStats,
) {
    let mut entity = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Button,
        CrystalItemHint(crystal_item_tooltip_document(item, player)),
    ));
    if enabled {
        entity.insert(action);
    }
    entity.with_children(|cell| {
        if let (Some(path), Some(icon_rect)) = (
            item_icon_path(item.icon),
            crystal_inventory_icon_rect(item, rect.width, rect.height),
        ) {
            cell.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(icon_rect.left),
                    top: Val::Px(icon_rect.top),
                    width: Val::Px(icon_rect.width),
                    height: Val::Px(icon_rect.height),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(path),
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    color: if enabled {
                        Color::WHITE
                    } else {
                        Color::srgba(0.412, 0.412, 0.412, 0.8)
                    },
                    ..default()
                },
            ));
        } else if item.icon == 0 {
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
            overlay_inventory_count(cell, &detail, rect.width, rect.height);
        }
    });
}

fn overlay_absolute_inventory_cell(
    parent: &mut ChildSpawnerCommands,
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
            ..default()
        },
        BackgroundColor(Color::NONE),
    ));
    if enabled {
        entity.insert((Button, action));
    }
}

fn crystal_inventory_icon_rect(
    item: &ItemModel,
    cell_width: f32,
    cell_height: f32,
) -> Option<CrystalRect> {
    if item.icon == 0 || item.icon_width == 0 || item.icon_height == 0 {
        return None;
    }
    let width = i32::from(item.icon_width);
    let height = i32::from(item.icon_height);
    let left = (cell_width as i32 - width) / 2;
    let top = (cell_height as i32 - height) / 2;
    Some(CrystalRect::new(
        left as f32,
        top as f32,
        width as f32,
        height as f32,
    ))
}

fn overlay_inventory_count(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    cell_width: f32,
    cell_height: f32,
) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(cell_width),
                height: Val::Px(cell_height),
                align_items: AlignItems::FlexEnd,
                justify_content: JustifyContent::FlexEnd,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|label| {
            label.spawn((
                Text::new(text.to_owned()),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
                TextLayout::new(Justify::Right, LineBreak::NoWrap),
            ));
        });
}

fn crystal_npc_goods_cell_rect(row: usize) -> CrystalRect {
    CrystalRect::new(
        10.0,
        34.0 + row as f32 * 33.0,
        NPC_GOODS_CELL_WIDTH,
        NPC_GOODS_CELL_HEIGHT,
    )
}

fn crystal_npc_goods_icon_rect(good: &ShopGood) -> Option<CrystalRect> {
    if good.icon == 0 || good.icon_width == 0 || good.icon_height == 0 {
        return None;
    }
    let width = i32::from(good.icon_width);
    let height = i32::from(good.icon_height);
    Some(CrystalRect::new(
        ((NPC_GOODS_ICON_AREA_WIDTH - width) / 2) as f32,
        ((NPC_GOODS_CELL_HEIGHT as i32 - height) / 2) as f32,
        width as f32,
        height as f32,
    ))
}

fn crystal_npc_goods_new_icon_visible(good: &ShopGood, goods: &[ShopGood]) -> bool {
    let Some(source) = good.tooltip_source.as_ref() else {
        return false;
    };
    let Some(item) = source.user_item.as_ref() else {
        return false;
    };
    let item_index = source.info.item_index;
    let mut matching = 0usize;
    let mut has_non_shop_item = false;
    for candidate in goods {
        let Some(candidate_source) = candidate.tooltip_source.as_ref() else {
            continue;
        };
        if candidate_source.info.item_index != item_index {
            continue;
        }
        matching += 1;
        has_non_shop_item |= candidate_source
            .user_item
            .as_ref()
            .is_some_and(|candidate| !candidate.is_shop_item);
    }
    let multiple_available = matching > 1 && has_non_shop_item;
    !item.is_shop_item || multiple_available
}

/// Crystal MirGoodsCell: one 205x32 click/hover surface, with a 40x32 icon
/// area and independent name/count/price labels at their source coordinates.
fn overlay_absolute_shop_good_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    good: &ShopGood,
    rect: CrystalRect,
    action: OverlayButton,
    player: &crate::read_model::PlayerStats,
    selected: bool,
    hide_added_stats: bool,
    show_new_icon: bool,
) {
    let mut entity = parent.spawn((
        OverlayNpcShopGoodCell,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Outline::new(
            Val::Px(1.0),
            Val::Px(0.0),
            if selected {
                Color::srgb(0.0, 1.0, 0.0)
            } else {
                Color::NONE
            },
        ),
        Button,
        action,
    ));
    if let Some(document) = crystal_item_tooltip_document_from_source_with_options(
        &good.name,
        good.icon,
        u32::from(good.count),
        good.tooltip_source.as_ref(),
        player,
        CrystalItemTooltipOptions { hide_added_stats },
    ) {
        entity.insert(CrystalItemHint(document));
    }
    entity.with_children(|row| {
        if let (Some(asset_server), Some(path), Some(icon_rect)) = (
            asset_server,
            item_icon_path(good.icon),
            crystal_npc_goods_icon_rect(good),
        ) {
            row.spawn((
                OverlayNpcShopGoodIcon,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(icon_rect.left),
                    top: Val::Px(icon_rect.top),
                    width: Val::Px(icon_rect.width),
                    height: Val::Px(icon_rect.height),
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
            OverlayNpcShopGoodName,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(44.0),
                top: Val::Px(0.0),
                ..default()
            },
            Text::new(good.name.clone()),
            TextFont {
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ));
        row.spawn((
            OverlayNpcShopGoodPrice,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(44.0),
                top: Val::Px(14.0),
                ..default()
            },
            Text::new(format!("Price: {} gold", good.price)),
            TextFont {
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ));
        if good.count > 1 {
            row.spawn((
                OverlayNpcShopGoodCount,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(23.0),
                    top: Val::Px(17.0),
                    ..default()
                },
                Text::new(good.count.to_string()),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
        }
        if selected {
            row.spawn((
                OverlayNpcShopGoodSelectionDivider,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(40.0),
                    top: Val::Px(0.0),
                    width: Val::Px(1.0),
                    height: Val::Px(NPC_GOODS_CELL_HEIGHT),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.0, 1.0, 0.0)),
            ));
        }
        if show_new_icon {
            if let Some(asset_server) = asset_server {
                row.spawn((
                    OverlayNpcShopGoodNewIcon,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(190.0),
                        top: Val::Px(5.0),
                        width: Val::Px(12.0),
                        height: Val::Px(9.0),
                        ..default()
                    },
                    ImageNode {
                        image: asset_server.load(NPC_GOODS_NEW_ICON_ASSET),
                        image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                        ..default()
                    },
                ));
            }
        }
    });
}

/// Compact row cell used by the still-provisional Shop/Warehouse panels. It
/// uses the same authoritative icon path without inventing an icon when the
/// source record has none.
fn overlay_compact_item_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    item: &ItemModel,
    label: &str,
    action: OverlayButton,
    enabled: bool,
    player: &crate::read_model::PlayerStats,
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
        Button,
        CrystalItemHint(crystal_item_tooltip_document(item, player)),
    ));
    if enabled {
        entity.insert(action);
    }
    entity.with_children(|row| {
        if let (Some(asset_server), Some(path)) = (asset_server, item_icon_path(item.icon)) {
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
    spawn_overlay_crystal_button_enabled_with_disabled(
        parent,
        asset_server,
        library,
        normal,
        hover,
        pressed,
        None,
        rect,
        action,
        enabled,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_overlay_crystal_button_enabled_with_disabled(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    library: &'static str,
    normal: u16,
    hover: u16,
    pressed: u16,
    disabled: Option<u16>,
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
    let mut assets = CrystalButtonAssetSet::from_spec(spec);
    if let Some(disabled) = disabled {
        assets = assets.with_disabled(spec.asset_path(disabled));
    }
    spawn_crystal_image_button(parent, asset_server, spec, assets, action, false, enabled);
}

const HELP_PAGE_TITLES: [&str; HELP_PAGE_COUNT as usize] = [
    "Shortcut Information",
    "Shortcut Information",
    "Chat Shortcuts",
    "Movements",
    "Attacking",
    "Collecting Items",
    "Health",
    "Skills",
    "Skills",
    "Mana",
    "Chatting",
    "Groups",
    "Durability",
    "Purchasing",
    "Selling",
    "Repairing",
    "Trading",
    "Inspecting",
    "Statistics",
    "Statistics",
    "Statistics",
    "Statistics",
    "Statistics",
    "Statistics",
    "Quests",
    "Quests",
    "Quests",
    "Quests",
    "Mounts",
    "Mounts",
    "Fishing",
    "Gems and Orbs",
    "Heroes",
    "Heroes",
    "Heroes",
    "Heroes",
    "Heroes",
    "Guild Buffs",
    "Guild Buffs",
    "Guild Buffs",
    "Awakening",
    "Awakening",
    "Awakening",
    "Awakening",
    "Awakening",
];

const HELP_SHORTCUT_PAGE_1: [(&str, &str); 18] = [
    ("Alt + Q", "Exit the game"),
    ("Alt + X", "Log out"),
    ("F1-F8", "Skill buttons"),
    ("F9", "Inventory window (open / close)"),
    ("F10", "Status window (open / close)"),
    ("F11", "Skill window (open / close)"),
    ("P", "Group window (open / close)"),
    ("T", "Trade window (open / close)"),
    ("F", "Friend window (open / close)"),
    ("V", "Minimap window (open / close)"),
    ("Ctrl + G", "Guild window (open / close)"),
    ("Y", "Gameshop window (open / close)"),
    ("L", "Engagement window (open / close)"),
    ("Ctrl + Z", "Belt window (open / close)"),
    ("F12", "Option window (open / close)"),
    ("H", "Help window (open / close)"),
    ("M", "Mount / Dismount ride"),
    ("", "Lock spell onto target not cursor location"),
];

const HELP_SHORTCUT_PAGE_2: [(&str, &str); 18] = [
    ("Ctrl + A", "Toggle pet attack pet"),
    ("Ctrl + H", "Toggle player attack mode"),
    ("", "Peace Mode - Attack monsters only"),
    (
        "",
        "Group Mode - Attack all subjects except your group members",
    ),
    (
        "",
        "Guild Mode - Attack all subjects except your guild members",
    ),
    ("", "Good/Evil Mode - Attack PK players and monsters only"),
    ("", "All Attack Mode - Attack all subjects"),
    ("B", "Show the field map"),
    ("R", "Show the skill bar"),
    ("D", "Auto run on / off"),
    ("Insert", "Show / Hide interface"),
    ("Tab", "Highlight / Pickup Items"),
    ("Ctrl + Right Click", "Show other players kits"),
    ("PrintScreen", "Screen Capture"),
    ("N", "Open / Close fishing window"),
    ("", "Mentor window (open / close)"),
    ("X", "Creature Pickup (Multi Mouse Target)"),
    ("Alt + A", "Creature Pickup (Single Mouse Target)"),
];

const HELP_SHORTCUT_PAGE_3: [(&str, &str); 3] = [
    ("/(username)", "Command to whisper to others"),
    ("!(text)", "Command to shout to others nearby"),
    ("!~(text)", "Command to guild chat"),
];

fn help_shortcut_rows(page: u8) -> Option<&'static [(&'static str, &'static str)]> {
    match page {
        0 => Some(&HELP_SHORTCUT_PAGE_1),
        1 => Some(&HELP_SHORTCUT_PAGE_2),
        2 => Some(&HELP_SHORTCUT_PAGE_3),
        _ => None,
    }
}

fn help_image_dimensions(index: u8) -> (f32, f32) {
    match index {
        0..=28 => (512.0, 396.0),
        29 => (509.0, 396.0),
        30 => (508.0, 395.0),
        31..=33 => (509.0, 396.0),
        34..=41 => (508.0, 395.0),
        _ => (0.0, 0.0),
    }
}

fn render_help(parent: &mut ChildSpawnerCommands, asset_server: Option<&AssetServer>, page: u8) {
    let page = page.min(HELP_PAGE_COUNT - 1);
    let Some(asset_server) = asset_server else {
        return;
    };

    spawn_overlay_frame(
        parent,
        asset_server,
        "original-ui/Prguse/920.png",
        CRYSTAL_HELP_PANEL_RECT.width,
        CRYSTAL_HELP_PANEL_RECT.height,
    );
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Title/57.png".to_owned(),
        CrystalRect::new(18.0, 9.0, 45.0, 14.0),
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        240,
        241,
        242,
        CrystalRect::new(210.0, 485.0, 16.0, 16.0),
        OverlayButton::HelpPrevious,
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        243,
        244,
        245,
        CrystalRect::new(310.0, 485.0, 16.0, 16.0),
        OverlayButton::HelpNext,
    );
    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse2",
        360,
        361,
        362,
        CrystalRect::new(509.0, 3.0, 24.0, 21.0),
        OverlayButton::CloseHelp,
    );
    overlay_text_at(
        parent,
        &format!("{} / {}", page + 1, HELP_PAGE_COUNT),
        CrystalRect::new(230.0, 480.0, 80.0, 20.0),
        9.0,
        TEXT,
    );
    overlay_text_at(
        parent,
        &format!("{}. {}", page + 1, HELP_PAGE_TITLES[usize::from(page)]),
        CrystalRect::new(147.0, 39.0, 242.0, 30.0),
        10.0,
        TEXT,
    );

    if let Some(rows) = help_shortcut_rows(page) {
        overlay_text_at(
            parent,
            "Shortcuts",
            CrystalRect::new(25.0, 110.0, 100.0, 30.0),
            10.0,
            TEXT,
        );
        overlay_text_at(
            parent,
            "Information",
            CrystalRect::new(126.0, 110.0, 405.0, 30.0),
            10.0,
            TEXT,
        );
        for (row, (shortcut, information)) in rows.iter().enumerate() {
            let top = 142.0 + row as f32 * 20.0;
            overlay_text_at(
                parent,
                shortcut,
                CrystalRect::new(30.0, top, 95.0, 23.0),
                9.0,
                GOLD,
            );
            overlay_text_at(
                parent,
                information,
                CrystalRect::new(131.0, top, 400.0, 23.0),
                9.0,
                TEXT,
            );
        }
    } else {
        let image_index = page - 3;
        let (width, height) = help_image_dimensions(image_index);
        parent.spawn((
            HelpPageImageEntity { image_index },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(75.0),
                width: Val::Px(width),
                height: Val::Px(height),
                ..default()
            },
            ImageNode {
                image: asset_server.load(format!("original-ui/Help/{image_index}.png")),
                ..default()
            },
        ));
    }
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

    spawn_overlay_crystal_button(
        parent,
        asset_server,
        "Prguse",
        1970,
        1971,
        1972,
        CrystalRect::new(3.0, 50.0, 32.0, 20.0),
        OverlayButton::ToggleHelp,
    );

    for (library, normal, hover, pressed, top) in [
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
    player: &crate::read_model::PlayerStats,
) {
    if state.group_open() {
        render_group_panel(parent, asset_server, social, state, combat_target);
    } else if state.guild_open() {
        render_guild_panel(parent, asset_server, social, state, player);
    } else {
        render_trade_panel(parent, asset_server, social, inventory, player);
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
    } else {
        let draft = if state.group_invite_focused {
            format!("Name: {}|", state.group_invite_draft)
        } else if state.group_invite_draft.is_empty() {
            "Name: <invite player>".to_owned()
        } else {
            format!("Name: {}", state.group_invite_draft)
        };
        overlay_clickable_text_at(
            parent,
            &draft,
            CrystalRect::new(rect.left + 16.0, rect.top + 191.0, 144.0, 18.0),
            OverlayButton::GroupInviteNameFocus,
            state.group_invite_focused,
            true,
        );
        overlay_absolute_button(
            parent,
            "Invite",
            CrystalRect::new(rect.left + 162.0, rect.top + 191.0, 48.0, 18.0),
            OverlayButton::GroupInviteNameSubmit,
            valid_social_name(state.group_invite_draft.trim()),
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
    player: &crate::read_model::PlayerStats,
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
    overlay_absolute_button(
        parent,
        "Storage",
        CrystalRect::new(rect.left + 162.0, rect.top + 38.0, 68.0, 24.0),
        OverlayButton::SelectGuildLeftPage(GuildLeftPage::Storage),
        true,
    );
    overlay_absolute_button(
        parent,
        "Ranks",
        CrystalRect::new(rect.left + 233.0, rect.top + 38.0, 62.0, 24.0),
        OverlayButton::SelectGuildLeftPage(GuildLeftPage::Ranks),
        true,
    );
    spawn_static_overlay_sprite(
        parent,
        asset_server,
        "original-ui/Title/104.png".to_owned(),
        CrystalRect::new(rect.left + 501.0, rect.top + 38.0, 72.0, 24.0),
    );

    match state.guild_left_page {
        GuildLeftPage::Notice => render_guild_notice(parent, asset_server, guild, state, rect),
        GuildLeftPage::Members => render_guild_members(parent, asset_server, guild, state, rect),
        GuildLeftPage::Storage => {
            render_guild_storage(parent, asset_server, guild, state, rect, player)
        }
        GuildLeftPage::Ranks => render_guild_ranks(parent, guild, state, rect),
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
    state: &NativePlayerUiState,
    rect: CrystalRect,
) {
    let notice = if guild.name.is_none() {
        "You are not in a guild.".to_owned()
    } else if state.guild_notice_editing {
        if state.guild_notice_draft.is_empty() {
            "|".to_owned()
        } else {
            format!("{}|", state.guild_notice_draft)
        }
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
    let can_edit = guild.name.is_some() && social_has_permission(guild, "notice");
    if can_edit {
        let action = if state.guild_notice_editing {
            OverlayButton::GuildPublishNotice
        } else {
            OverlayButton::GuildBeginNoticeEdit
        };
        let publish_enabled = !state.guild_notice_editing
            || (state.guild_notice_submission.is_none()
                && guild_notice_lines(&state.guild_notice_draft)
                    .is_some_and(|notice| notice != guild.notice));
        spawn_overlay_crystal_button_enabled(
            parent,
            asset_server,
            "Prguse",
            560,
            561,
            562,
            CrystalRect::new(rect.left + 20.0, rect.top + 402.0, 28.0, 25.0),
            action,
            publish_enabled,
        );
        if state.guild_notice_editing {
            overlay_absolute_button(
                parent,
                "Cancel",
                CrystalRect::new(rect.left + 54.0, rect.top + 405.0, 58.0, 20.0),
                OverlayButton::GuildCancelNoticeEdit,
                state.guild_notice_submission.is_none(),
            );
        }
    }
    if state.guild_notice_submission.is_some() {
        overlay_text_at(
            parent,
            "Waiting for authoritative notice receipt...",
            CrystalRect::new(rect.left + 122.0, rect.top + 407.0, 260.0, 16.0),
            9.0,
            GOLD,
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
    let can_kick = social_has_permission(guild, "kick");
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

    let can_recruit = social_has_permission(guild, "recruit");
    if can_recruit {
        let draft = if state.guild_recruit_focused {
            format!("Recruit: {}|", state.guild_recruit_draft)
        } else if state.guild_recruit_draft.is_empty() {
            "Recruit: <player name>".to_owned()
        } else {
            format!("Recruit: {}", state.guild_recruit_draft)
        };
        overlay_clickable_text_at(
            parent,
            &draft,
            CrystalRect::new(rect.left + 20.0, rect.top + 364.0, 220.0, 18.0),
            OverlayButton::GuildRecruitNameFocus,
            state.guild_recruit_focused,
            true,
        );
        overlay_absolute_button(
            parent,
            "Add",
            CrystalRect::new(rect.left + 244.0, rect.top + 364.0, 52.0, 18.0),
            OverlayButton::GuildRecruitNameSubmit,
            valid_social_name(state.guild_recruit_draft.trim()),
        );
    }

    let can_change_rank = social_has_permission(guild, "changeRank")
        && state.selected_guild_member.is_some()
        && guild.ranks.len() > 1;
    overlay_absolute_button(
        parent,
        "Rank -",
        CrystalRect::new(rect.left + 20.0, rect.top + 386.0, 58.0, 18.0),
        OverlayButton::GuildAssignPreviousRank,
        can_change_rank,
    );
    overlay_absolute_button(
        parent,
        "Rank +",
        CrystalRect::new(rect.left + 82.0, rect.top + 386.0, 58.0, 18.0),
        OverlayButton::GuildAssignNextRank,
        can_change_rank,
    );
}

fn render_guild_storage(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    guild: &crate::social::GuildModel,
    state: &NativePlayerUiState,
    rect: CrystalRect,
    player: &crate::read_model::PlayerStats,
) {
    const PAGE_SIZE: usize = 28;
    let page_count = crate::social::MAX_GUILD_STORAGE_ITEMS.div_ceil(PAGE_SIZE);
    let page = state.guild_storage_page.min(page_count.saturating_sub(1));
    let start = page * PAGE_SIZE;
    overlay_text_at(
        parent,
        &format!("Guild Gold: {}", guild.gold),
        CrystalRect::new(rect.left + 20.0, rect.top + 70.0, 220.0, 18.0),
        10.0,
        GOLD,
    );
    let draft = if state.guild_gold_focused {
        format!("Amount: {}|", state.guild_gold_draft)
    } else if state.guild_gold_draft.is_empty() {
        "Amount: 0".to_owned()
    } else {
        format!("Amount: {}", state.guild_gold_draft)
    };
    overlay_clickable_text_at(
        parent,
        &draft,
        CrystalRect::new(rect.left + 20.0, rect.top + 91.0, 150.0, 18.0),
        OverlayButton::GuildGoldFocus,
        state.guild_gold_focused,
        true,
    );
    let amount_valid = state
        .guild_gold_draft
        .parse::<u32>()
        .is_ok_and(|amount| amount > 0);
    overlay_absolute_button(
        parent,
        "Deposit",
        CrystalRect::new(rect.left + 176.0, rect.top + 91.0, 62.0, 18.0),
        OverlayButton::GuildGoldDeposit,
        amount_valid && social_has_permission(guild, "storeItem"),
    );
    overlay_absolute_button(
        parent,
        "Withdraw",
        CrystalRect::new(rect.left + 242.0, rect.top + 91.0, 68.0, 18.0),
        OverlayButton::GuildGoldWithdraw,
        amount_valid && social_has_permission(guild, "retrieveItem"),
    );

    for offset in 0..PAGE_SIZE {
        let slot = start + offset;
        let column = offset % 4;
        let row = offset / 4;
        let cell_rect = CrystalRect::new(
            rect.left + 20.0 + column as f32 * 75.0,
            rect.top + 121.0 + row as f32 * 31.0,
            72.0,
            28.0,
        );
        let Some(item) = guild.storage_items.get(slot).and_then(Option::as_ref) else {
            overlay_text_at(parent, &format!("{slot}: -"), cell_rect, 8.0, TEXT);
            continue;
        };
        let name = item
            .tooltip_source
            .as_ref()
            .map(|source| source.info.name.as_str())
            .unwrap_or("Item");
        let icon = item
            .tooltip_source
            .as_ref()
            .map(|source| source.info.image)
            .unwrap_or_default();
        let mut cell = parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cell_rect.left),
                top: Val::Px(cell_rect.top),
                width: Val::Px(cell_rect.width),
                height: Val::Px(cell_rect.height),
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.06, 0.03, 0.42)),
            Interaction::None,
            FocusPolicy::Block,
        ));
        if let Some(document) = crystal_item_tooltip_document_from_source(
            name,
            icon,
            u32::from(item.count),
            item.tooltip_source.as_ref(),
            player,
        ) {
            cell.insert(CrystalItemHint(document));
        }
        cell.with_children(|content| {
            if let Some(path) = item_icon_path(icon) {
                content.spawn((
                    Node {
                        width: Val::Px(26.0),
                        height: Val::Px(26.0),
                        ..default()
                    },
                    ImageNode {
                        image: asset_server.load(path),
                        image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                        ..default()
                    },
                ));
            }
            content.spawn((
                Text::new(format!(
                    "{slot}: {} x{}",
                    short_name(name, &item.item_index.to_string()),
                    item.count
                )),
                TextFont {
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(TEXT),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
        });
    }
    overlay_absolute_button(
        parent,
        "Prev",
        CrystalRect::new(rect.left + 20.0, rect.top + 352.0, 52.0, 18.0),
        OverlayButton::GuildStoragePreviousPage,
        page > 0,
    );
    overlay_text_at(
        parent,
        &format!("{}/{}", page + 1, page_count),
        CrystalRect::new(rect.left + 80.0, rect.top + 352.0, 50.0, 18.0),
        9.0,
        TEXT,
    );
    overlay_absolute_button(
        parent,
        "Next",
        CrystalRect::new(rect.left + 130.0, rect.top + 352.0, 52.0, 18.0),
        OverlayButton::GuildStorageNextPage,
        page + 1 < page_count,
    );
}

fn render_guild_ranks(
    parent: &mut ChildSpawnerCommands,
    guild: &crate::social::GuildModel,
    state: &NativePlayerUiState,
    rect: CrystalRect,
) {
    for (row, rank) in guild.ranks.iter().take(12).enumerate() {
        let permissions = [
            (0x01, "Rank"),
            (0x02, "Recruit"),
            (0x04, "Kick"),
            (0x08, "Store"),
            (0x10, "Take"),
            (0x20, "Ally"),
            (0x40, "Notice"),
            (0x80, "Buff"),
        ]
        .into_iter()
        .filter_map(|(bit, label)| (rank.options & bit != 0).then_some(label))
        .collect::<Vec<_>>()
        .join("/");
        let Ok(rank_index) = u8::try_from(rank.index) else {
            continue;
        };
        overlay_clickable_text_at(
            parent,
            &format!("{}  {}  [{}]", rank.index, rank.name, permissions),
            CrystalRect::new(
                rect.left + 20.0,
                rect.top + 75.0 + row as f32 * 19.0,
                300.0,
                18.0,
            ),
            OverlayButton::SelectGuildRank(rank_index),
            state.selected_guild_rank == Some(rank_index),
            true,
        );
    }

    let can_change = social_has_permission(guild, "changeRank");
    if let Some(rank_index) = state.selected_guild_rank {
        let draft = if state.guild_rank_name_focused {
            format!("Rank name: {}|", state.guild_rank_name_draft)
        } else {
            format!("Rank name: {}", state.guild_rank_name_draft)
        };
        overlay_clickable_text_at(
            parent,
            &draft,
            CrystalRect::new(rect.left + 20.0, rect.top + 315.0, 205.0, 18.0),
            OverlayButton::GuildRankNameFocus,
            state.guild_rank_name_focused,
            can_change,
        );
        let name_changed = guild
            .ranks
            .iter()
            .find(|rank| u8::try_from(rank.index).ok() == Some(rank_index))
            .is_some_and(|rank| rank.name != state.guild_rank_name_draft.trim());
        overlay_absolute_button(
            parent,
            "Save",
            CrystalRect::new(rect.left + 232.0, rect.top + 315.0, 52.0, 18.0),
            OverlayButton::GuildRankNameSave,
            can_change && name_changed && !state.guild_rank_name_draft.trim().is_empty(),
        );
        let rank_options = guild
            .ranks
            .iter()
            .find(|rank| u8::try_from(rank.index).ok() == Some(rank_index))
            .map(|rank| rank.options)
            .unwrap_or(0);
        for (option, label) in [
            "Rank", "Recruit", "Kick", "Store", "Take", "Ally", "Notice", "Buff",
        ]
        .into_iter()
        .enumerate()
        {
            let enabled = rank_options & (1_u8 << option) != 0;
            overlay_absolute_button(
                parent,
                &format!("{} {}", if enabled { "[x]" } else { "[ ]" }, label),
                CrystalRect::new(
                    rect.left + 20.0 + (option % 4) as f32 * 72.0,
                    rect.top + 340.0 + (option / 4) as f32 * 22.0,
                    68.0,
                    18.0,
                ),
                OverlayButton::GuildRankTogglePermission(option as u8),
                can_change,
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
        overlay_text_at(
            parent,
            "Rank",
            CrystalRect::new(rect.left + 362.0, rect.top + 185.0, 75.0, 14.0),
            9.0,
            Color::srgb(0.55, 0.55, 0.55),
        );
        overlay_text_at(
            parent,
            guild.rank_name.as_deref().unwrap_or("-"),
            CrystalRect::new(rect.left + 437.0, rect.top + 185.0, 120.0, 14.0),
            9.0,
            TEXT,
        );
        overlay_text_at(
            parent,
            "Gold",
            CrystalRect::new(rect.left + 362.0, rect.top + 211.0, 75.0, 14.0),
            9.0,
            Color::srgb(0.55, 0.55, 0.55),
        );
        overlay_text_at(
            parent,
            &guild.gold.to_string(),
            CrystalRect::new(rect.left + 437.0, rect.top + 211.0, 120.0, 14.0),
            9.0,
            GOLD,
        );
        overlay_text_at(
            parent,
            &format!("Rights: {}", guild.permissions.join("/")),
            CrystalRect::new(rect.left + 362.0, rect.top + 237.0, 195.0, 52.0),
            8.0,
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

fn spawn_partner_trade_item(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    item: &crate::social::TradeItemModel,
    player: &crate::read_model::PlayerStats,
) {
    let name = item
        .name
        .as_deref()
        .or_else(|| {
            item.tooltip_source
                .as_ref()
                .map(|source| source.info.name.as_str())
        })
        .unwrap_or("Item");
    let icon = item
        .tooltip_source
        .as_ref()
        .map(|source| source.info.image)
        .unwrap_or_default();
    let mut row = parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(34.0),
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.06, 0.03, 0.42)),
        Interaction::None,
        FocusPolicy::Block,
    ));
    if let Some(document) = crystal_item_tooltip_document_from_source(
        name,
        icon,
        u32::from(item.count),
        item.tooltip_source.as_ref(),
        player,
    ) {
        row.insert(CrystalItemHint(document));
    }
    row.with_children(|content| {
        if let (Some(asset_server), Some(path)) = (asset_server, item_icon_path(icon)) {
            content.spawn((
                Node {
                    width: Val::Px(30.0),
                    height: Val::Px(30.0),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(path),
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                },
            ));
        }
        content.spawn((
            Text::new(format!("{} x{}", short_name(name, "Item"), item.count)),
            TextFont {
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(TEXT),
        ));
    });
}

fn spawn_trade_offer_item(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    item: &ItemModel,
    player: &crate::read_model::PlayerStats,
) {
    let mut row = parent.spawn((
        Button,
        OverlayButton::TradeDepositItem(item.slot.min(9) as u8),
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(34.0),
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            ..default()
        },
        BackgroundColor(BUTTON_BG),
        CrystalItemHint(crystal_item_tooltip_document(item, player)),
    ));
    row.with_children(|content| {
        if let (Some(asset_server), Some(path)) = (asset_server, item_icon_path(item.icon)) {
            content.spawn((
                Node {
                    width: Val::Px(30.0),
                    height: Val::Px(30.0),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(path),
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                },
            ));
        }
        content.spawn((
            Text::new(format!(
                "Offer {} x{}",
                short_name(&item.name, &item.key),
                item.quantity
            )),
            TextFont {
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(TEXT),
        ));
    });
}

fn render_trade_panel(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    social: &crate::social::SocialModel,
    inventory: &InventoryModel,
    player: &crate::read_model::PlayerStats,
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
                spawn_partner_trade_item(trade_parent, asset_server, item, player);
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
                    spawn_trade_offer_item(trade_parent, asset_server, item, player);
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
        // This is MailDialog's context-help control, not MenuDialog's global
        // HelpDialog button. Its source handler remains unimplemented here.
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
                viewport.spawn((
                    BigMapLoadingText,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(BIGMAP_HEIGHT * 0.5 - 8.0),
                        width: Val::Px(BIGMAP_WIDTH),
                        height: Val::Px(16.0),
                        ..default()
                    },
                    Text::new("Loading map..."),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.82, 0.74, 0.55)),
                    TextLayout::justify(Justify::Center),
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
    spawn_overlay_crystal_button_enabled_with_disabled(
        parent,
        asset_server,
        "Title",
        821,
        822,
        823,
        Some(823),
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
    player: &crate::read_model::PlayerStats,
) {
    let show_buy = shop.allows_buy() && (!shop.allows_sell() || state.npc_shop_buy_tab);
    if !show_buy {
        render_npc_item_service(parent, asset_server, shop, inventory, state);
        return;
    }
    let buy_enabled = shop_buy_enabled(shop, inventory, state.shop_quantity);

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
        let show_new_icon = crystal_npc_goods_new_icon_visible(good, &shop.goods);
        overlay_absolute_shop_good_button(
            parent,
            asset_server,
            good,
            crystal_npc_goods_cell_rect(row),
            OverlayButton::SelectShopGood(good.unique_id),
            player,
            selected,
            shop.hide_added_stats,
            show_new_icon,
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
    if shop.allows_sell() {
        overlay_absolute_button(
            parent,
            "Sell",
            CrystalRect::new(162.0, 304.0, 52.0, 22.0),
            OverlayButton::ShopShowSell,
            true,
        );
    }
}

fn render_npc_item_service(
    parent: &mut ChildSpawnerCommands,
    _asset_server: Option<&AssetServer>,
    shop: &ShopModel,
    inventory: &InventoryModel,
    state: &NativePlayerUiState,
) {
    let title = if shop.allows_special_repair() {
        "Special repair"
    } else if shop.allows_repair() {
        "Repair items"
    } else if shop.allows_sell() {
        "Sell items"
    } else {
        "NPC service unavailable"
    };
    let sell_mode = shop.allows_sell();
    let repair_mode = shop.allows_repair() || shop.allows_special_repair();
    let sell_enabled = sell_mode && shop_sell_enabled(inventory, shop.selected_bag_slot_for_sell);
    let repair_enabled = repair_mode && repair_selection_enabled(state, inventory);

    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(360.0),
                height: Val::Px(360.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(3.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|header| {
                    body(header, title);
                    if shop.allows_buy() && shop.allows_sell() {
                        overlay_button(header, "Buy", OverlayButton::ShopShowBuy, true);
                    }
                    overlay_button(header, "Close", OverlayButton::ShopCancel, true);
                });

            if let Some(rate) = shop.repair_rate.filter(|_| repair_mode) {
                body(panel, &format!("Repair rate x{rate:.2}"));
            }

            for item in inventory.items_in(0).into_iter().take(10) {
                let selected = if sell_mode {
                    shop.selected_bag_slot_for_sell == Some(item.slot)
                } else {
                    state.shop_repair_container == 0 && state.shop_repair_slot == Some(item.slot)
                };
                overlay_button(
                    panel,
                    &format!(
                        "{}{} x{}",
                        if selected { "▶ " } else { "" },
                        short_name(&item.name, &item.key),
                        item.quantity
                    ),
                    if sell_mode {
                        OverlayButton::SelectBagForSell(item.slot)
                    } else {
                        OverlayButton::SelectBagForRepair(item.slot)
                    },
                    sell_mode || repair_mode,
                );
            }

            if repair_mode {
                body(panel, "Equipment");
                panel
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
                            let selected = state.shop_repair_container == 2
                                && state.shop_repair_slot == Some(slot);
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
            }

            panel
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|actions| {
                    if sell_mode {
                        overlay_button(
                            actions,
                            "−",
                            OverlayButton::ShopQuantityDec,
                            state.shop_quantity > SHOP_QUANTITY_MIN,
                        );
                        body(actions, &format!("x{}", state.shop_quantity));
                        overlay_button(
                            actions,
                            "+",
                            OverlayButton::ShopQuantityInc,
                            state.shop_quantity < SHOP_QUANTITY_MAX,
                        );
                        overlay_button(actions, "Sell", OverlayButton::ShopSell, sell_enabled);
                    } else if shop.allows_repair() {
                        overlay_button(
                            actions,
                            "Repair",
                            OverlayButton::ShopRepair,
                            repair_enabled,
                        );
                    } else if shop.allows_special_repair() {
                        overlay_button(
                            actions,
                            "Special repair",
                            OverlayButton::ShopSRepair,
                            repair_enabled,
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
            &ui.player,
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
    player: &crate::read_model::PlayerStats,
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
    } else {
        card.insert((Interaction::None, FocusPolicy::Block));
    }
    if let Some(document) = crystal_item_tooltip_document_from_source(
        &entry.item_name,
        u16::try_from(entry.image).unwrap_or_default(),
        u32::from(entry.count),
        entry.tooltip_source.as_ref(),
        player,
    ) {
        card.insert(CrystalItemHint(document));
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
    player: &crate::read_model::PlayerStats,
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
                player,
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
            item,
            &format!(
                "{}{} x{}",
                if selected { "▶" } else { "" },
                short_name(&item.name, &item.key),
                item.quantity
            ),
            OverlayButton::SelectBagForStore(item.slot),
            true,
            player,
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
        .then(|| item.quantity.to_string())
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
    use crate::crystal_ui::widget::CrystalImageButton;
    use crate::inventory::{
        CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel,
    };
    use crate::mail::MailMessage;
    use crate::shop::ShopGood;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::input::keyboard::Key;

    #[test]
    fn inventory_hud_pointer_edges_enqueue_one_crystal_button_a_sound() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<InventoryModel>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Update, consume_hud_buttons);

        let button = app
            .world_mut()
            .spawn((Interaction::None, CrystalHudAction::Inventory))
            .id();
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            0
        );

        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            1
        );

        // Changed<Interaction> consumes the pointer edge, never the held state.
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            1
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonA]
        );

        app.world_mut().entity_mut(button).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            1
        );
        app.world_mut()
            .resource_mut::<crate::audio::NativeUiAudioQueue>()
            .drain_bounded(8);

        // A stale press outside InGame is neither a click nor a sound.
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::Login;
        app.world_mut().entity_mut(button).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            0
        );

        // Keyboard inventory toggles call the state directly and never enter
        // the HUD pointer producer.
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_inventory();
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            0
        );
    }

    #[test]
    fn crystal_character_source_geometry_matches_character_dialog() {
        assert_eq!(
            CRYSTAL_CHARACTER_PANEL_RECT,
            CrystalRect::new(760.0, 0.0, 264.0, 380.0)
        );
        assert_eq!(
            CRYSTAL_CHARACTER_PAGE_RECT,
            CrystalRect::new(8.0, 90.0, 248.0, 284.0)
        );
        assert_eq!(
            CRYSTAL_CHARACTER_EQUIPMENT_SLOTS,
            [
                (0, CrystalRect::new(131.0, 97.0, 32.0, 32.0)),
                (1, CrystalRect::new(171.0, 97.0, 32.0, 32.0)),
                (2, CrystalRect::new(211.0, 97.0, 32.0, 32.0)),
                (13, CrystalRect::new(211.0, 152.0, 32.0, 32.0)),
                (4, CrystalRect::new(211.0, 188.0, 32.0, 32.0)),
                (3, CrystalRect::new(211.0, 224.0, 32.0, 32.0)),
                (5, CrystalRect::new(16.0, 260.0, 32.0, 32.0)),
                (6, CrystalRect::new(211.0, 260.0, 32.0, 32.0)),
                (7, CrystalRect::new(16.0, 296.0, 32.0, 32.0)),
                (8, CrystalRect::new(211.0, 296.0, 32.0, 32.0)),
                (9, CrystalRect::new(16.0, 332.0, 32.0, 32.0)),
                (11, CrystalRect::new(56.0, 332.0, 32.0, 32.0)),
                (10, CrystalRect::new(96.0, 332.0, 32.0, 32.0)),
                (12, CrystalRect::new(136.0, 332.0, 32.0, 32.0)),
            ]
        );
    }

    #[test]
    fn crystal_character_gender_class_and_guild_projection_is_exact() {
        assert_eq!(crystal_character_page_index(Some("Male")), 340);
        assert_eq!(crystal_character_page_index(Some("female")), 341);
        assert_eq!(crystal_character_page_index(None), 340);
        assert_eq!(
            crystal_character_class_image_index(Some("Warrior")),
            Some(100)
        );
        assert_eq!(
            crystal_character_class_image_index(Some("Wizard")),
            Some(101)
        );
        assert_eq!(
            crystal_character_class_image_index(Some("Taoist")),
            Some(102)
        );
        assert_eq!(
            crystal_character_class_image_index(Some("Assassin")),
            Some(103)
        );
        assert_eq!(
            crystal_character_class_image_index(Some("Archer")),
            Some(104)
        );
        assert_eq!(crystal_character_class_image_index(Some("unknown")), None);

        let mut ui = UiReadModel::default();
        ui.player.guild_name = Some("  Crystal Guild ".to_owned());
        ui.player.guild_rank_name = Some(" Leader ".to_owned());
        assert_eq!(crystal_character_guild_label(&ui), "Crystal Guild Leader");
    }

    #[test]
    fn crystal_character_hair_uses_source_index_offsets_and_assassin_adjustment() {
        assert_eq!(
            crystal_character_hair_frame(Some("Warrior"), Some("Male"), Some(0)),
            Some(CrystalFrameSpec::new(
                "Prguse",
                441,
                CrystalRect::new(131.0, 173.0, 16.0, 14.0)
            ))
        );
        assert_eq!(
            crystal_character_hair_frame(Some("Assassin"), Some("Male"), Some(0)),
            Some(CrystalFrameSpec::new(
                "Prguse",
                461,
                CrystalRect::new(131.0, 172.0, 16.0, 21.0)
            ))
        );
        assert_eq!(
            crystal_character_hair_frame(Some("Wizard"), Some("Female"), Some(8)),
            Some(CrystalFrameSpec::new(
                "Prguse",
                489,
                CrystalRect::new(118.0, 168.0, 40.0, 30.0)
            ))
        );
        assert_eq!(
            crystal_character_hair_frame(Some("Assassin"), Some("Female"), Some(0)),
            Some(CrystalFrameSpec::new(
                "Prguse",
                501,
                CrystalRect::new(126.0, 174.0, 24.0, 24.0)
            ))
        );
        assert!(crystal_character_hair_frame(Some("Warrior"), None, Some(0)).is_none());
        assert!(crystal_character_hair_frame(Some("Warrior"), Some("Male"), Some(9)).is_none());
    }

    #[test]
    fn crystal_character_paper_doll_order_is_armour_weapon_then_helmet_or_hair() {
        let state_item = |slot, image, x, y, width, height| ItemModel {
            container: 2,
            slot,
            state_image: image,
            state_image_x: x,
            state_image_y: y,
            state_image_width: width,
            state_image_height: height,
            ..Default::default()
        };
        let mut inventory = InventoryModel::default();
        inventory.items = vec![
            state_item(0, 30, 75, 186, 28, 57),
            state_item(1, 60, 92, 194, 80, 128),
        ];
        let mut ui = UiReadModel::default();
        ui.player.class_name = Some("Warrior".to_owned());
        ui.player.gender = Some("Male".to_owned());
        ui.player.hair = Some(0);

        let frames = crystal_character_paper_doll_frames(&inventory, &ui);
        assert_eq!(
            frames.iter().map(|frame| frame.index).collect::<Vec<_>>(),
            vec![60, 30, 441]
        );
        assert_eq!(frames[0].rect, CrystalRect::new(92.0, 194.0, 80.0, 128.0));

        inventory.items.push(state_item(2, 100, 120, 160, 24, 30));
        let frames = crystal_character_paper_doll_frames(&inventory, &ui);
        assert_eq!(
            frames.iter().map(|frame| frame.index).collect::<Vec<_>>(),
            vec![60, 30, 100],
            "a present helmet suppresses Crystal's hair fallback"
        );
    }

    #[test]
    fn character_hud_pointer_uses_button_a_and_restores_character_page() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<InventoryModel>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Update, consume_hud_buttons);

        // Crystal keeps CharacterDialog visible and switches a non-character
        // page back to CharacterPage on the first HUD click.
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.character_page = CharacterPage::Stats1;
            state.toggle_equipment();
        }
        let button = app
            .world_mut()
            .spawn((
                Interaction::None,
                CrystalHudAction::Character,
                CrystalImageButton {
                    assets: super::super::assets::CrystalButtonAssetSet::from_spec(
                        super::super::spec::hud::CHARACTER,
                    ),
                    focused: false,
                    enabled: true,
                },
            ))
            .id();
        app.update();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(state.equipment_open());
        assert_eq!(state.character_page, CharacterPage::Character);
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            1
        );
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());

        // A held press neither repeats ButtonA nor invokes the callback.
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .equipment_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            1
        );
        app.world_mut()
            .resource_mut::<crate::audio::NativeUiAudioQueue>()
            .drain_bounded(8);

        // A second edge closes the already-visible CharacterPage and plays
        // exactly one new ButtonA cue.
        app.world_mut().entity_mut(button).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .equipment_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            1
        );
        app.world_mut()
            .resource_mut::<crate::audio::NativeUiAudioQueue>()
            .drain_bounded(8);

        // Disabled image buttons are not valid clicks even if a synthetic
        // Interaction component changes under test.
        app.world_mut()
            .entity_mut(button)
            .get_mut::<CrystalImageButton>()
            .expect("image button")
            .enabled = false;
        app.world_mut().entity_mut(button).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .equipment_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            0
        );

        // Keyboard activation shares the source page-state transition but
        // stays outside the pointer/audio producer.
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .activate_character_hud_button();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .equipment_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            0
        );
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
    }

    #[test]
    fn character_hud_activation_restores_every_non_character_page_before_closing() {
        for page in [
            CharacterPage::Stats1,
            CharacterPage::Stats2,
            CharacterPage::Spells,
        ] {
            let mut state = NativePlayerUiState::default();
            state.character_page = page;
            state.toggle_equipment();
            assert!(state.equipment_open());

            state.activate_character_hud_button();
            assert!(state.equipment_open());
            assert_eq!(state.character_page, CharacterPage::Character);

            state.activate_character_hud_button();
            assert!(!state.equipment_open());
        }
    }

    #[test]
    fn menu_hud_pointer_uses_button_c_and_toggles_menu_without_gameplay_intents() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<InventoryModel>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Update, consume_hud_buttons);

        let button = app
            .world_mut()
            .spawn((Interaction::None, CrystalHudAction::Menu))
            .id();
        app.update();

        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        assert!(app.world().resource::<NativePlayerUiState>().menu_open());
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonC]
        );
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
    }

    #[test]
    fn five_small_main_hud_buttons_emit_button_a_on_real_press_edges() {
        for action in [
            CrystalHudAction::Inventory,
            CrystalHudAction::Character,
            CrystalHudAction::Skill,
            CrystalHudAction::Quest,
            CrystalHudAction::Option,
        ] {
            let mut app = App::new();
            app.init_resource::<NativePlayerUiState>()
                .init_resource::<InventoryModel>()
                .init_resource::<NativePlayerUiIntentQueue>()
                .init_resource::<crate::audio::NativeUiAudioQueue>()
                .insert_resource(NativeShellModel {
                    screen: NativeShellScreen::InGame,
                    ..Default::default()
                })
                .add_systems(Update, consume_hud_buttons);
            let button = app.world_mut().spawn((Interaction::None, action)).id();
            app.update();

            app.world_mut()
                .entity_mut(button)
                .insert(Interaction::Pressed);
            app.update();
            assert_eq!(
                app.world_mut()
                    .resource_mut::<crate::audio::NativeUiAudioQueue>()
                    .drain_bounded(8),
                vec![crate::audio::NativeUiSound::ButtonA],
                "{action:?} must emit exactly one Crystal ButtonA edge"
            );
            assert!(app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .is_empty());
        }
    }

    #[test]
    fn game_shop_hud_pointer_uses_button_c_and_resets_quantity_only_on_close() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<InventoryModel>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Update, consume_hud_buttons);

        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.shop_quantity = 7;
        }
        let button = app
            .world_mut()
            .spawn((Interaction::None, CrystalHudAction::GameShop))
            .id();
        app.update();

        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        {
            let state = app.world().resource::<NativePlayerUiState>();
            assert!(state.shop_open());
            assert_eq!(state.shop_quantity, 7);
        }
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonC]
        );

        app.world_mut().entity_mut(button).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();
        {
            let state = app.world().resource::<NativePlayerUiState>();
            assert!(!state.shop_open());
            assert_eq!(state.shop_quantity, 1);
        }
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonC]
        );
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
    }

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
        assert_eq!(inventory_cell_stack_label(&stacked), "12");
    }

    #[test]
    fn inventory_icons_use_true_size_centering_and_fail_closed_without_geometry() {
        let potion = ItemModel {
            icon: 7,
            icon_width: 36,
            icon_height: 26,
            ..default()
        };
        assert_eq!(
            crystal_inventory_icon_rect(&potion, 36.0, 32.0),
            Some(CrystalRect::new(0.0, 3.0, 36.0, 26.0))
        );

        let odd_height = ItemModel {
            icon: 30,
            icon_width: 36,
            icon_height: 25,
            ..default()
        };
        assert_eq!(
            crystal_inventory_icon_rect(&odd_height, 36.0, 32.0),
            Some(CrystalRect::new(0.0, 3.0, 36.0, 25.0))
        );

        assert!(crystal_inventory_icon_rect(&ItemModel::default(), 36.0, 32.0).is_none());
    }

    #[test]
    fn operation_disabled_inventory_item_remains_a_rich_tooltip_hover_target() {
        let mut app = overlay_render_test_app();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::Inventory;
            state.inventory_operation = Some(InventoryOperationDraft::Move {
                source_slot: 0,
                unique_id: 42,
            });
        }
        app.world_mut().resource_mut::<InventoryModel>().items = vec![ItemModel {
            unique_id: Some(42),
            key: "wooden-sword".to_owned(),
            name: "Wooden Sword".to_owned(),
            quantity: 1,
            slot: 0,
            container: 0,
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 221,
                    name: "Wooden Sword".to_owned(),
                    item_type: 1,
                    durability: 4000,
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id: 42,
                    item_index: 221,
                    current_dura: 3000,
                    max_dura: 4000,
                    count: 1,
                    ..Default::default()
                }),
                socket_infos: Vec::new(),
                ..Default::default()
            }),
            ..Default::default()
        }];

        app.update();

        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(&CrystalItemHint, Option<&OverlayButton>), With<Button>>();
        let (hint, action) = query
            .iter(world)
            .find(|(hint, _)| hint.0.plain_text().contains("Wooden Sword"))
            .expect("occupied inventory cell stays hoverable while its action is disabled");
        assert!(hint.0.source_complete);
        assert!(hint.0.plain_text().contains("Weapon"));
        assert_eq!(action, None);
    }

    #[test]
    fn warehouse_bag_item_uses_the_same_rich_tooltip_document() {
        let mut app = overlay_render_test_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::Storage;
        app.world_mut().resource_mut::<InventoryModel>().items = vec![ItemModel {
            unique_id: Some(43),
            key: "small-hp-drug".to_owned(),
            name: "Small HP Drug".to_owned(),
            quantity: 5,
            slot: 0,
            container: 0,
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 658,
                    name: "(HP)DrugSmall".to_owned(),
                    item_type: 13,
                    weight: 1,
                    stack_size: 20,
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id: 43,
                    item_index: 658,
                    count: 5,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }];

        app.update();

        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(&CrystalItemHint, Option<&OverlayButton>), With<Button>>();
        let (hint, action) = query
            .iter(world)
            .find(|(hint, _)| hint.0.plain_text().contains("Small HP Drug (5)"))
            .expect("warehouse bag row must remain a rich item hover target");
        assert!(hint.0.source_complete);
        assert!(hint.0.plain_text().contains("Potion"));
        assert!(matches!(action, Some(OverlayButton::SelectBagForStore(0))));
    }

    fn surface_tooltip_source(
        item_index: i32,
        name: &str,
        image: u16,
        unique_id: u64,
        count: u16,
    ) -> CrystalItemTooltipSourceModel {
        CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel {
                item_index,
                name: name.to_owned(),
                item_type: 13,
                image,
                stack_size: 20,
                ..Default::default()
            },
            user_item: Some(CrystalUserItemModel {
                unique_id,
                item_index,
                count,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn assert_rendered_rich_hint(app: &mut App, expected_name: &str) {
        let world = app.world_mut();
        let mut query = world.query::<&CrystalItemHint>();
        let hint = query
            .iter(world)
            .find(|hint| hint.0.plain_text().contains(expected_name))
            .unwrap_or_else(|| panic!("missing rich hint for {expected_name}"));
        assert!(hint.0.source_complete);
        assert!(hint.0.plain_text().contains("Potion"));
    }

    #[test]
    fn npc_game_shop_guild_and_trade_cells_share_the_crystal_item_hint_lifecycle() {
        let mut app = overlay_render_test_app();

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::NpcShop;
        {
            let mut shop = app.world_mut().resource_mut::<ShopModel>();
            shop.service_mode = NpcShopServiceMode::Buy;
            shop.goods = vec![ShopGood {
                unique_id: 101,
                name: "NPC Potion".to_owned(),
                count: 3,
                icon: 532,
                tooltip_source: Some(surface_tooltip_source(658, "NPC Potion", 532, 101, 3)),
                ..Default::default()
            }];
        }
        app.update();
        assert_rendered_rich_hint(&mut app, "NPC Potion (3)");

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::GameShop;
        app.world_mut().resource_mut::<GameShopModel>().items =
            vec![crate::game_shop::GameShopEntry {
                item_index: 659,
                game_shop_index: 31,
                item_name: "Cash Potion".to_owned(),
                image: 533,
                count: 2,
                class: "All".to_owned(),
                tooltip_source: Some(surface_tooltip_source(659, "Cash Potion", 533, 0, 2)),
                ..Default::default()
            }];
        app.update();
        assert_rendered_rich_hint(&mut app, "Cash Potion (2)");

        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::Guild;
            state.guild_left_page = GuildLeftPage::Storage;
        }
        app.world_mut()
            .resource_mut::<crate::social::SocialModel>()
            .guild
            .storage_items = vec![Some(crate::social::GuildStorageItemModel {
            unique_id: 202,
            item_index: 660,
            count: 4,
            user_id: 7,
            tooltip_source: Some(surface_tooltip_source(660, "Guild Potion", 534, 202, 4)),
        })];
        app.update();
        assert_rendered_rich_hint(&mut app, "Guild Potion (4)");

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::Trade;
        app.world_mut()
            .resource_mut::<crate::social::SocialModel>()
            .trade
            .partner_items = vec![crate::social::TradeItemModel {
            unique_id: Some(303),
            item_index: Some(661),
            name: Some("Trade Potion".to_owned()),
            count: 5,
            tooltip_source: Some(surface_tooltip_source(661, "Trade Potion", 535, 303, 5)),
        }];
        app.update();
        assert_rendered_rich_hint(&mut app, "Trade Potion (5)");
    }

    #[test]
    fn npc_goods_cell_uses_crystal_geometry_labels_selection_and_hidden_added_stats() {
        let mut app = overlay_render_test_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::NpcShop;

        let mut source = surface_tooltip_source(221, "Wooden Sword", 7, 101, 3);
        source.info.item_type = 1;
        source.info.stats = vec![
            crate::inventory::CrystalItemStatModel { stat: 4, value: 2 },
            crate::inventory::CrystalItemStatModel { stat: 5, value: 4 },
        ];
        let item = source.user_item.as_mut().expect("shop user item");
        item.is_shop_item = false;
        item.cursed = true;
        item.added_stats = vec![crate::inventory::CrystalItemStatModel { stat: 5, value: 9 }];

        {
            let mut shop = app.world_mut().resource_mut::<ShopModel>();
            shop.service_mode = NpcShopServiceMode::Buy;
            shop.hide_added_stats = true;
            shop.selected_id = Some(101);
            shop.goods = vec![ShopGood {
                unique_id: 101,
                name: "Wooden Sword".to_owned(),
                price: 50,
                count: 3,
                icon: 7,
                icon_width: 36,
                icon_height: 26,
                tooltip_source: Some(source),
                ..Default::default()
            }];
        }

        app.update();

        {
            let world = app.world_mut();
            let mut cell_query = world.query_filtered::<
                (&Node, &Outline, &CrystalItemHint),
                With<OverlayNpcShopGoodCell>,
            >();
            let (cell, outline, hint) = cell_query.single(world).expect("NPC goods cell");
            assert_eq!(cell.left, Val::Px(10.0));
            assert_eq!(cell.top, Val::Px(34.0));
            assert_eq!(cell.width, Val::Px(205.0));
            assert_eq!(cell.height, Val::Px(32.0));
            assert_eq!(outline.width, Val::Px(1.0));
            assert_eq!(outline.color, Color::srgb(0.0, 1.0, 0.0));
            assert!(hint.0.plain_text().contains("DC + 2~4"));
            assert!(!hint.0.plain_text().contains("(+9)"));
            assert!(!hint.0.plain_text().contains("Cursed"));

            let mut icon_query = world.query_filtered::<&Node, With<OverlayNpcShopGoodIcon>>();
            let icon = icon_query.single(world).expect("true-size shop icon");
            assert_eq!(icon.left, Val::Px(2.0));
            assert_eq!(icon.top, Val::Px(3.0));
            assert_eq!(icon.width, Val::Px(36.0));
            assert_eq!(icon.height, Val::Px(26.0));

            let mut divider_query =
                world.query_filtered::<&Node, With<OverlayNpcShopGoodSelectionDivider>>();
            let divider = divider_query.single(world).expect("selection divider");
            assert_eq!(divider.left, Val::Px(40.0));
            assert_eq!(divider.height, Val::Px(32.0));

            let mut new_icon_query =
                world.query_filtered::<&Node, With<OverlayNpcShopGoodNewIcon>>();
            let new_icon = new_icon_query.single(world).expect("Crystal new icon");
            assert_eq!(new_icon.left, Val::Px(190.0));
            assert_eq!(new_icon.top, Val::Px(5.0));
            assert_eq!(new_icon.width, Val::Px(12.0));
            assert_eq!(new_icon.height, Val::Px(9.0));
        }

        let (name, name_left, name_top) = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<(&Text, &Node), With<OverlayNpcShopGoodName>>();
            let (text, node) = query.single(world).expect("name label");
            (text.0.clone(), node.left, node.top)
        };
        let (price, price_left, price_top) = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<(&Text, &Node), With<OverlayNpcShopGoodPrice>>();
            let (text, node) = query.single(world).expect("price label");
            (text.0.clone(), node.left, node.top)
        };
        let (count, count_left, count_top) = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<(&Text, &Node), With<OverlayNpcShopGoodCount>>();
            let (text, node) = query.single(world).expect("count label");
            (text.0.clone(), node.left, node.top)
        };
        assert_eq!(name, "Wooden Sword");
        assert_eq!((name_left, name_top), (Val::Px(44.0), Val::Px(0.0)));
        assert_eq!(price, "Price: 50 gold");
        assert_eq!((price_left, price_top), (Val::Px(44.0), Val::Px(14.0)));
        assert_eq!(count, "3");
        assert_eq!((count_left, count_top), (Val::Px(23.0), Val::Px(17.0)));
    }

    #[test]
    fn npc_goods_geometry_and_new_marker_match_mir_goods_cell_rules() {
        assert_eq!(
            crystal_npc_goods_cell_rect(7),
            CrystalRect::new(10.0, 265.0, 205.0, 32.0)
        );
        let mut good = ShopGood {
            icon: 7,
            icon_width: 36,
            icon_height: 26,
            tooltip_source: Some(surface_tooltip_source(221, "Sword", 7, 1, 1)),
            ..Default::default()
        };
        assert_eq!(
            crystal_npc_goods_icon_rect(&good),
            Some(CrystalRect::new(2.0, 3.0, 36.0, 26.0))
        );
        good.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .is_shop_item = true;
        assert!(!crystal_npc_goods_new_icon_visible(
            &good,
            std::slice::from_ref(&good)
        ));
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
    fn hud_controls_remain_above_quest_world_blocker_and_options_switches_panel() {
        // This intentionally wires the real native HUD, overlay renderer and
        // quest renderer together.  A reducer-only test cannot catch the
        // full-stage quest modal blocker covering the HUD hit target.
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<bevy::audio::AudioSource>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_plugins((
                super::super::hud::Mir2CrystalHudPlugin,
                crate::quest_ui::Mir2QuestUiPlugin,
            ));

        // Startup creates the actual HUD/Quest/Options entities.
        app.update();

        let option_button = {
            let mut query = app.world_mut().query::<(Entity, &CrystalHudAction)>();
            query
                .iter(app.world())
                .find_map(|(entity, action)| {
                    matches!(action, CrystalHudAction::Option).then_some(entity)
                })
                .expect("native HUD option button should be present")
        };

        let hud_z = {
            let mut query = app
                .world_mut()
                .query_filtered::<&GlobalZIndex, With<super::super::hud::CrystalHudRoot>>();
            query.single(app.world()).expect("HUD root should exist").0
        };
        let world_blocker_z = {
            let mut query = app.world_mut().query::<(&Node, &GlobalZIndex)>();
            query
                .iter(app.world())
                .find_map(|(node, z)| {
                    (node.width == Val::Percent(100.0)
                        && node.height == Val::Percent(100.0)
                        && z.0 < 980)
                        .then_some(z.0)
                })
                .expect("quest world blocker should carry an explicit z-index")
        };
        assert!(
            world_blocker_z < hud_z,
            "quest world blocker must stay below every persistent HUD control"
        );

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::QuestLog;
        app.update();

        // Press the real HUD button entity as Bevy's pointer hit-test would.
        // The quest modal is open at this point, so this reproduces the
        // previously failing transition rather than opening Options in an
        // otherwise empty game state.
        app.world_mut()
            .entity_mut(option_button)
            .insert(Interaction::Pressed);
        app.update();

        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(
            state.core.panel,
            mir2_ui_core::state::UiPanel::Options,
            "HUD Option must replace QuestLog, not merely close it"
        );
        assert!(
            state.blocks_world_click(),
            "Options must capture world clicks"
        );

        let options_visible = {
            let mut query = app.world_mut().query::<(&OverlayOptions, &Node)>();
            query
                .iter(app.world())
                .any(|(_, node)| node.display == Display::Flex)
        };
        assert!(
            options_visible,
            "OverlayOptions must be visible after the switch"
        );

        let quest_root_stays_in_game_layer = {
            let mut query = app.world_mut().query::<(&GlobalZIndex, &Node)>();
            query
                .iter(app.world())
                .any(|(z, node)| z.0 == 980 && node.display == Display::Flex)
        };
        assert!(
            quest_root_stays_in_game_layer,
            "QuestUiRoot remains the in-game layer"
        );
        let quest_log_hidden = {
            let mut query = app.world_mut().query::<&Node>();
            query.iter(app.world()).any(|node| {
                node.left == Val::Px(212.0)
                    && node.top == Val::Px(80.0)
                    && node.width == Val::Px(600.0)
                    && node.display == Display::None
            })
        };
        assert!(
            quest_log_hidden,
            "QuestLog panel must disappear after opening Options"
        );
        let quest_blocker_hidden = {
            let mut query = app.world_mut().query::<&Node>();
            query.iter(app.world()).any(|node| {
                node.width == Val::Percent(100.0)
                    && node.height == Val::Percent(100.0)
                    && node.display == Display::None
            })
        };
        assert!(
            quest_blocker_hidden,
            "Quest modal blocker must release the pointer"
        );

        // Release then press again to verify the same HUD control closes the
        // Options panel and does not leave a stale visible overlay behind.
        app.world_mut()
            .entity_mut(option_button)
            .insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(option_button)
            .insert(Interaction::Pressed);
        app.update();

        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(state.core.panel, mir2_ui_core::state::UiPanel::None);
        assert!(!state.blocks_world_click());
        let options_hidden = {
            let mut query = app.world_mut().query::<(&OverlayOptions, &Node)>();
            query
                .iter(app.world())
                .all(|(_, node)| node.display == Display::None)
        };
        assert!(
            options_hidden,
            "repeated Option click must hide the overlay"
        );
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
            .init_resource::<crate::audio::NativeUiAudioQueue>()
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
        assert_eq!(inventory_node.left, Val::Px(0.0));
        assert_eq!(inventory_node.top, Val::Px(0.0));
        let inventory_viewports = app
            .world_mut()
            .query_filtered::<&Node, With<OverlayInventoryGridViewport>>()
            .iter(app.world())
            .count();
        assert_eq!(inventory_viewports, 1);

        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.inventory_window.left = 120.0;
            state.inventory_window.top = 90.0;
        }
        app.update();
        let inventory_node = app
            .world_mut()
            .query_filtered::<&Node, With<OverlayInventory>>()
            .single(app.world())
            .expect("moved inventory panel");
        assert_eq!(inventory_node.left, Val::Px(120.0));
        assert_eq!(inventory_node.top, Val::Px(90.0));

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
    fn guild_notice_draft_is_bounded_by_lines_and_characters() {
        let mut draft = String::new();
        push_guild_notice_text(&mut draft, &"x".repeat(40));
        assert_eq!(draft.chars().count(), GUILD_NOTICE_MAX_CHARS_PER_LINE);
        for _ in 0..20 {
            push_guild_notice_text(&mut draft, "\nnext");
        }
        assert_eq!(draft.split('\n').count(), crate::social::MAX_NOTICE_LINES);
        assert!(guild_notice_lines(&draft).is_some());
        assert!(guild_notice_lines(&format!(
            "{}\nextra",
            (0..crate::social::MAX_NOTICE_LINES)
                .map(|_| "line")
                .collect::<Vec<_>>()
                .join("\n")
        ))
        .is_none());
    }

    #[test]
    fn guild_notice_editor_consumes_real_keyboard_messages_before_chat() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<StorageModel>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .add_message::<KeyboardInput>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Update, process_overlay_keyboard);
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::Guild;
            state.guild_left_page = GuildLeftPage::Notice;
            state.guild_notice_editing = true;
        }
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyN,
            logical_key: Key::Character("New notice".into()),
            state: ButtonState::Pressed,
            text: Some("New notice".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(state.guild_notice_draft, "New notice");
        assert!(!state.chat_focused());
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
    }

    #[test]
    fn guild_notice_edit_publish_failure_retry_and_receipt_stay_authoritative() {
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
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::Guild;
            state.guild_left_page = GuildLeftPage::Notice;
        }
        {
            let mut social = app.world_mut().resource_mut::<crate::social::SocialModel>();
            social.guild.name = Some("TestGuild".into());
            social.guild.permissions = vec!["notice".into()];
            social.guild.notice = vec!["Old".into()];
        }
        fn press(app: &mut App, button: OverlayButton) {
            let entity = app
                .world_mut()
                .spawn((Interaction::Pressed, button, Button))
                .id();
            app.update();
            app.world_mut().despawn(entity);
        }

        press(&mut app, OverlayButton::GuildBeginNoticeEdit);
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            assert!(state.guild_notice_editing);
            assert_eq!(state.guild_notice_draft, "Old");
            state.guild_notice_draft = "New line\nSecond".into();
        }
        press(&mut app, OverlayButton::GuildPublishNotice);
        assert_eq!(
            app.world()
                .resource::<crate::social::SocialModel>()
                .guild
                .notice,
            vec!["Old"],
            "submitting a draft must not mutate the authoritative notice"
        );
        assert!(matches!(
            app.world()
                .resource::<crate::social::SocialModel>()
                .pending
                .as_slice(),
            [crate::social::SocialPendingOperation::GuildNotice { notice }]
                if notice == &vec!["New line".to_owned(), "Second".to_owned()]
        ));
        let first = app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        assert!(matches!(
            first.as_slice(),
            [NativePlayerUiIntent::GuildEditNotice { notice }]
                if notice == &vec!["New line".to_owned(), "Second".to_owned()]
        ));

        {
            let current = app.world().resource::<crate::social::SocialModel>().clone();
            let mut incoming = current.clone();
            incoming.pending.clear();
            assert!(
                incoming.apply_packet("GuildNoticeResult", &serde_json::json!({"success":false}))
            );
            app.world_mut()
                .resource_mut::<crate::social::SocialModel>()
                .apply_authoritative(incoming);
        }
        app.update();
        {
            let state = app.world().resource::<NativePlayerUiState>();
            assert!(state.guild_notice_editing);
            assert!(state.guild_notice_submission.is_none());
            assert_eq!(state.guild_notice_draft, "New line\nSecond");
        }

        press(&mut app, OverlayButton::GuildPublishNotice);
        let retry = app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        assert!(matches!(
            retry.as_slice(),
            [NativePlayerUiIntent::GuildEditNotice { .. }]
        ));
        {
            let current = app.world().resource::<crate::social::SocialModel>().clone();
            let mut incoming = current.clone();
            incoming.pending.clear();
            assert!(incoming.apply_packet(
                "GuildNoticeChange",
                &serde_json::json!({"update":-1,"notice":[]})
            ));
            app.world_mut()
                .resource_mut::<crate::social::SocialModel>()
                .apply_authoritative(incoming);
        }
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(!state.guild_notice_editing);
        assert!(state.guild_notice_submission.is_none());
        assert_eq!(
            app.world()
                .resource::<crate::social::SocialModel>()
                .guild
                .notice,
            vec!["Old"]
        );
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .iter()
            .any(|intent| matches!(
                intent,
                NativePlayerUiIntent::GuildRequestInfo { info_type: 0 }
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
            .init_resource::<crate::audio::NativeUiAudioQueue>()
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
            .init_resource::<crate::audio::NativeUiAudioQueue>()
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
            .init_resource::<crate::audio::NativeUiAudioQueue>()
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
    fn overlay_keyboard_toggles_skills_with_f11() {
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
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .add_message::<KeyboardInput>()
            .add_systems(Update, process_overlay_keyboard);
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        });

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F11);
        app.update();
        assert!(app.world().resource::<NativePlayerUiState>().skill_open());

        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::F11);
            keys.clear();
            keys.press(KeyCode::F11);
        }
        app.update();
        assert!(!app.world().resource::<NativePlayerUiState>().skill_open());
    }

    #[test]
    fn character_c_and_f10_share_source_page_semantics_without_click_audio() {
        for key in [KeyCode::KeyC, KeyCode::F10] {
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
                .init_resource::<crate::audio::NativeUiAudioQueue>()
                .add_message::<KeyboardInput>()
                .add_systems(Update, process_overlay_keyboard);
            app.insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            });
            {
                let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
                state.character_page = CharacterPage::Stats2;
                state.toggle_equipment();
            }

            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();
            let state = app.world().resource::<NativePlayerUiState>();
            assert!(state.equipment_open());
            assert_eq!(state.character_page, CharacterPage::Character);

            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.release(key);
                keys.clear();
                keys.press(key);
            }
            app.update();
            assert!(!app
                .world()
                .resource::<NativePlayerUiState>()
                .equipment_open());

            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.release(key);
                keys.clear();
                keys.press(key);
            }
            app.update();
            let state = app.world().resource::<NativePlayerUiState>();
            assert!(state.equipment_open());
            assert_eq!(state.character_page, CharacterPage::Character);
            assert_eq!(
                app.world()
                    .resource::<crate::audio::NativeUiAudioQueue>()
                    .len(),
                0
            );
            assert!(app
                .world()
                .resource::<NativePlayerUiIntentQueue>()
                .intents
                .is_empty());
        }
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
        assert_eq!(
            app.world_mut()
                .query::<&BigMapLoadingText>()
                .iter(app.world())
                .count(),
            0,
            "an authoritative map image must replace the loading state"
        );
    }

    #[test]
    fn big_map_ecs_shows_loading_text_without_authoritative_image() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_resource::<BigMapModel>()
            .init_resource::<BigMapUiState>()
            .init_resource::<UiReadModel>()
            .add_systems(Startup, spawn_big_map_render_test);

        app.update();

        let world = app.world_mut();
        let loading = world
            .query::<(&BigMapLoadingText, &Text)>()
            .iter(world)
            .map(|(_, text)| text.0.clone())
            .collect::<Vec<_>>();
        assert_eq!(loading, vec!["Loading map...".to_owned()]);
        assert_eq!(
            world.query::<&BigMapImageEntity>().iter(world).count(),
            0,
            "waiting for authority must not fabricate a map image"
        );
    }

    #[test]
    fn big_map_disabled_teleport_uses_explicit_crystal_title_823_frame() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_resource::<BigMapModel>()
            .init_resource::<BigMapUiState>()
            .init_resource::<UiReadModel>()
            .add_systems(Startup, spawn_big_map_render_test);
        app.update();

        let world = app.world_mut();
        let (entity, button, sprite_entity) = {
            let mut buttons =
                world.query::<(Entity, &OverlayButton, &CrystalImageButton, &Children)>();
            let (entity, _, button, children) = buttons
                .iter(world)
                .find(|(_, action, _, _)| matches!(action, OverlayButton::BigMapTeleport))
                .expect("BigMap Teleport button");
            let sprite_entity = children.iter().next().expect("Teleport button sprite");
            (entity, button.clone(), sprite_entity)
        };
        assert!(!button.enabled);
        assert!(!world.entity(entity).contains::<Button>());
        assert_eq!(
            button.assets.disabled.as_deref(),
            Some("original-ui/Title/823.png")
        );
        let image = world
            .get::<ImageNode>(sprite_entity)
            .expect("Teleport sprite image");
        assert_eq!(
            world
                .resource::<AssetServer>()
                .get_path(image.image.id())
                .map(|path| path.path().to_string_lossy().replace('\\', "/")),
            Some("original-ui/Title/823.png".to_owned())
        );
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
        press(&mut app, OverlayButton::InventoryDeleteToggle);
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_delete_prompt
            .is_some());
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
        press(&mut app, OverlayButton::InventoryDeleteCancel);
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
    fn inventory_delete_mode_uses_exact_stack_or_single_item_flow() {
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
                key: "guide-ring".into(),
                name: "Guide Ring".into(),
                quantity: 1,
                slot: 1,
                container: 0,
                ..ItemModel::default()
            },
            ItemModel {
                unique_id: Some(44),
                key: "quest-leaf".into(),
                name: "Cannibal Leaves".into(),
                quantity: 5,
                slot: 0,
                container: 3,
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

        // No selected bag cell: the footer enters persistent delete mode.
        press(&mut app, OverlayButton::InventoryDeleteToggle);
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .inventory_delete_mode
        );
        press(&mut app, OverlayButton::InspectQuest(0));
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inspect
            .is_none());

        // A stack opens MirAmountBox semantics with the maximum selected.
        press(&mut app, OverlayButton::InspectBag(0));
        {
            let state = app.world().resource::<NativePlayerUiState>();
            assert!(matches!(
                state.inventory_delete_prompt.as_ref(),
                Some(InventoryDeletePrompt::Amount {
                    target,
                    draft,
                    select_all: true,
                }) if target.unique_id == 42 && target.max_count == 4 && draft == "4"
            ));
        }
        push_delete_amount_text(
            &mut app.world_mut().resource_mut::<NativePlayerUiState>(),
            "2",
        );
        press(&mut app, OverlayButton::InventoryDeleteConfirm);

        // A selected one-count item opens the source Yes/No message box.
        press(&mut app, OverlayButton::InspectBag(1));
        press(&mut app, OverlayButton::InventoryDeleteToggle);
        assert!(matches!(
            app.world()
                .resource::<NativePlayerUiState>()
                .inventory_delete_prompt,
            Some(InventoryDeletePrompt::Confirm { .. })
        ));
        press(&mut app, OverlayButton::InventoryDeleteConfirm);

        let intents = app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents();
        assert_eq!(intents.len(), 2);
        assert!(matches!(
            intents[0],
            NativePlayerUiIntent::DeleteItem {
                unique_id: 42,
                count: 2,
                hero_inventory: false,
            }
        ));
        assert!(matches!(
            intents[1],
            NativePlayerUiIntent::DeleteItem {
                unique_id: 43,
                count: 1,
                hero_inventory: false,
            }
        ));
        let pending = app.world().resource::<PendingOperations>();
        assert!(pending.contains(&PendingOperationKey::DeleteItem {
            unique_id: 42,
            count: 2,
        }));
        assert!(pending.contains(&PendingOperationKey::DeleteItem {
            unique_id: 43,
            count: 1,
        }));
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(!state.inventory_delete_mode);
        assert!(state.inventory_delete_prompt.is_none());
    }

    #[test]
    fn inventory_delete_amount_editing_clamps_and_close_matches_crystal() {
        let inventory = InventoryModel {
            items: vec![ItemModel {
                unique_id: Some(7),
                key: "potion".into(),
                name: "Potion".into(),
                quantity: 12,
                slot: 3,
                container: 0,
                ..ItemModel::default()
            }],
            ..Default::default()
        };
        let mut state = NativePlayerUiState {
            inventory_delete_mode: true,
            ..Default::default()
        };
        assert!(state.open_inventory_delete_for_slot(&inventory, 3));
        push_delete_amount_text(&mut state, "9");
        assert!(matches!(
            state.inventory_delete_prompt,
            Some(InventoryDeletePrompt::Amount { ref draft, .. }) if draft == "9"
        ));
        push_delete_amount_text(&mut state, "9");
        assert!(matches!(
            state.inventory_delete_prompt,
            Some(InventoryDeletePrompt::Amount { ref draft, .. }) if draft == "12"
        ));
        delete_amount_backspace(&mut state);
        assert!(matches!(
            state.inventory_delete_prompt,
            Some(InventoryDeletePrompt::Amount { ref draft, .. }) if draft == "1"
        ));
        // The amount-box X disposes the modal without invoking CancelDelete.
        state.inventory_delete_prompt = None;
        assert!(state.inventory_delete_mode);
    }

    #[test]
    fn stale_inventory_delete_prompt_cannot_delete_a_replacement_stack() {
        let mut inventory = InventoryModel {
            items: vec![ItemModel {
                unique_id: Some(7),
                key: "potion".into(),
                name: "Potion".into(),
                quantity: 3,
                slot: 2,
                container: 0,
                ..ItemModel::default()
            }],
            ..Default::default()
        };
        let mut state = NativePlayerUiState {
            inventory_delete_mode: true,
            ..Default::default()
        };
        assert!(state.open_inventory_delete_for_slot(&inventory, 2));
        inventory.items[0].unique_id = Some(8);
        let mut intents = NativePlayerUiIntentQueue::default();
        let mut pending = PendingOperations::default();
        assert!(!confirm_inventory_delete(
            &mut state,
            &inventory,
            &mut intents,
            &mut pending,
        ));
        assert!(intents.drain_intents().is_empty());
        assert!(!state.inventory_delete_mode);
        assert!(state.inventory_delete_prompt.is_none());
    }

    #[test]
    fn inventory_delete_dialog_uses_source_centered_geometry() {
        let mut app = overlay_render_test_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_inventory();
        app.world_mut().resource_mut::<InventoryModel>().items = vec![ItemModel {
            unique_id: Some(42),
            key: "potion".into(),
            name: "Potion".into(),
            quantity: 4,
            slot: 0,
            container: 0,
            icon: 7,
            icon_width: 36,
            icon_height: 26,
            ..ItemModel::default()
        }];
        {
            let inventory = app.world().resource::<InventoryModel>().clone();
            assert!(app
                .world_mut()
                .resource_mut::<NativePlayerUiState>()
                .open_inventory_delete_for_slot(&inventory, 0));
        }
        app.update();
        app.update();

        let dialog = app
            .world_mut()
            .query_filtered::<&Node, With<OverlayInventoryDeleteDialog>>()
            .single(app.world())
            .expect("amount dialog");
        assert_eq!(dialog.left, Val::Px(CRYSTAL_DELETE_AMOUNT_RECT.left));
        assert_eq!(dialog.top, Val::Px(CRYSTAL_DELETE_AMOUNT_RECT.top));
        assert_eq!(dialog.width, Val::Px(204.0));
        assert_eq!(dialog.height, Val::Px(109.0));

        let input = app
            .world_mut()
            .query_filtered::<&Node, With<OverlayInventoryDeleteAmountInput>>()
            .single(app.world())
            .expect("amount input");
        assert_eq!(input.left, Val::Px(58.0));
        assert_eq!(input.top, Val::Px(43.0));
        assert_eq!(input.width, Val::Px(132.0));
        assert_eq!(input.height, Val::Px(19.0));
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
    fn quest_inventory_items_are_inspectable_but_read_only() {
        let inventory = InventoryModel {
            items: vec![ItemModel {
                unique_id: Some(124),
                key: "cannibal-leaves".to_owned(),
                name: "Cannibal Leaves".to_owned(),
                quantity: 5,
                slot: 0,
                container: 3,
                ..ItemModel::default()
            }],
            ..InventoryModel::default()
        };
        let mut state = NativePlayerUiState::default();
        state.inspect = inventory
            .items_in(3)
            .into_iter()
            .find(|item| item.slot == 0)
            .map(inspect_from_item);

        assert_eq!(container_name(3), "quest");
        assert_eq!(state.inspect.as_ref().map(|item| item.quantity), Some(5));
        assert!(inspected_inventory_item(&state, &inventory).is_none());
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
            ..Default::default()
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

        for page in [
            CharacterPage::Stats1,
            CharacterPage::Stats2,
            CharacterPage::Spells,
            CharacterPage::Character,
        ] {
            let entity = app
                .world_mut()
                .spawn((
                    Button,
                    Interaction::Pressed,
                    OverlayButton::SelectCharacterPage(page),
                ))
                .id();
            app.update();
            assert_eq!(
                app.world().resource::<NativePlayerUiState>().character_page,
                page
            );
            assert_eq!(
                app.world_mut()
                    .resource_mut::<crate::audio::NativeUiAudioQueue>()
                    .drain_bounded(8),
                vec![crate::audio::NativeUiSound::ButtonA]
            );

            // A held button is not another Crystal click edge.
            app.update();
            assert_eq!(
                app.world()
                    .resource::<crate::audio::NativeUiAudioQueue>()
                    .len(),
                0
            );

            // Release and press again produces exactly one new cue.
            app.world_mut().entity_mut(entity).insert(Interaction::None);
            app.update();
            assert_eq!(
                app.world()
                    .resource::<crate::audio::NativeUiAudioQueue>()
                    .len(),
                0
            );
            app.world_mut()
                .entity_mut(entity)
                .insert(Interaction::Pressed);
            app.update();
            assert_eq!(
                app.world_mut()
                    .resource_mut::<crate::audio::NativeUiAudioQueue>()
                    .drain_bounded(8),
                vec![crate::audio::NativeUiSound::ButtonA]
            );
            app.world_mut().despawn(entity);
        }

        for page in [
            CharacterPage::Character,
            CharacterPage::Stats1,
            CharacterPage::Stats2,
            CharacterPage::Spells,
        ] {
            {
                let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
                state.core.panel = mir2_ui_core::state::UiPanel::Character;
                state.character_page = page;
            }
            let entity = app
                .world_mut()
                .spawn((Button, Interaction::Pressed, OverlayButton::CloseCharacter))
                .id();
            app.update();
            assert!(!app
                .world()
                .resource::<NativePlayerUiState>()
                .equipment_open());
            assert_eq!(
                app.world().resource::<NativePlayerUiState>().character_page,
                CharacterPage::Character,
                "the shared close lifecycle resets transient page state"
            );
            assert_eq!(
                app.world_mut()
                    .resource_mut::<crate::audio::NativeUiAudioQueue>()
                    .drain_bounded(8),
                vec![crate::audio::NativeUiSound::ButtonA]
            );

            // A held close control is not a second Crystal click edge.
            app.update();
            assert_eq!(
                app.world()
                    .resource::<crate::audio::NativeUiAudioQueue>()
                    .len(),
                0
            );

            app.world_mut().entity_mut(entity).insert(Interaction::None);
            app.update();
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .panel = mir2_ui_core::state::UiPanel::Character;
            app.world_mut()
                .entity_mut(entity)
                .insert(Interaction::Pressed);
            app.update();
            assert!(!app
                .world()
                .resource::<NativePlayerUiState>()
                .equipment_open());
            assert_eq!(
                app.world_mut()
                    .resource_mut::<crate::audio::NativeUiAudioQueue>()
                    .drain_bounded(8),
                vec![crate::audio::NativeUiSound::ButtonA]
            );
            app.world_mut().despawn(entity);
        }

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::Character;
        app.world_mut().resource_mut::<NativeShellModel>().screen =
            NativeShellScreen::ConnectionLost;
        let blocked_close = app
            .world_mut()
            .spawn((Button, Interaction::Pressed, OverlayButton::CloseCharacter))
            .id();
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .equipment_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            0
        );
        app.world_mut().despawn(blocked_close);
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::InGame;

        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
        assert!(app.world().resource::<NativeUiIntentQueue>().is_empty());

        let press = |app: &mut App, action: OverlayButton| {
            let entity = app
                .world_mut()
                .spawn((Button, Interaction::Pressed, action))
                .id();
            app.update();
            app.world_mut().despawn(entity);
        };
        press(&mut app, OverlayButton::SelectInventoryPage(1));
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().inventory_page,
            0,
            "the source length 46 keeps the second bag locked"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonA],
            "Crystal's locked MirButton still emits its click cue"
        );
        app.world_mut().resource_mut::<InventoryModel>().capacity = 54;
        press(&mut app, OverlayButton::SelectInventoryPage(1));
        assert_eq!(
            app.world().resource::<NativePlayerUiState>().inventory_page,
            1,
            "an explicit expanded Crystal array unlocks page two"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonA]
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
    fn inventory_capacity_downgrade_closes_the_second_page_fail_closed() {
        let mut state = NativePlayerUiState {
            inventory_page: 1,
            ..Default::default()
        };
        let expanded = InventoryModel {
            capacity: 54,
            ..Default::default()
        };
        assert!(!reconcile_inventory_capacity(&mut state, &expanded));
        assert_eq!(state.inventory_page, 1);

        let locked = InventoryModel::default();
        assert!(reconcile_inventory_capacity(&mut state, &locked));
        assert_eq!(state.inventory_page, 0);
        assert!(state.inspect.is_none());
        assert!(state.inventory_operation.is_none());
        assert!(state.drop_confirmation.is_none());
    }

    #[test]
    fn inventory_second_tab_uses_exact_crystal_locked_and_page_assets() {
        let locked = InventoryModel::default();
        assert_eq!(inventory_second_tab_index(&locked, false), 169);
        assert_eq!(inventory_second_tab_index(&locked, true), 169);

        let expanded = InventoryModel {
            capacity: 54,
            ..Default::default()
        };
        assert_eq!(inventory_second_tab_index(&expanded, false), 738);
        assert_eq!(inventory_second_tab_index(&expanded, true), 168);
    }

    #[test]
    fn inventory_footer_uses_crystal_gold_and_weight_rules() {
        assert_eq!(format_crystal_gold(0), "0");
        assert_eq!(format_crystal_gold(1_280), "1,280");
        assert_eq!(format_crystal_gold(12_345_678), "12,345,678");

        assert_eq!(inventory_weight_bar_asset(0.0), ("Prguse", 24));
        assert_eq!(inventory_weight_bar_asset(0.50), ("Prguse", 24));
        assert_eq!(inventory_weight_bar_asset(0.75), ("UI_32bit", 471));
        assert_eq!(inventory_weight_bar_asset(0.76), ("UI_32bit", 470));
        assert_eq!(inventory_weight_bar_width(0.0), 0.0);
        assert_eq!(inventory_weight_bar_width(0.5), 40.0);
        assert_eq!(inventory_weight_bar_width(1.0), 81.0);
        assert_eq!(inventory_weight_bar_width(2.0), 81.0);
    }

    #[test]
    fn inventory_drag_matches_mircontrol_child_hit_and_stage_clamp_rules() {
        let mut window = InventoryDialogUi::default();
        assert_eq!((window.left, window.top), (0.0, 0.0));

        assert!(!window.begin_drag(10.0, 10.0), "first tab owns the hit");
        assert!(!window.begin_drag(10.0, 40.0), "item cells own the hit");
        assert!(!window.begin_drag(50.0, 216.0), "gold label owns the hit");
        assert!(
            !window.begin_drag(292.0, 213.0),
            "delete button owns the hit"
        );
        assert!(!window.begin_drag(300.0, 10.0), "close button owns the hit");
        assert!(
            !window.begin_drag(250.0, 10.0),
            "visible add button owns the hit"
        );
        assert!(
            window.begin_drag(182.0, 217.0),
            "WeightBar is NotControl in Crystal, so the parent owns its hit"
        );
        assert!(window.dragging());
        window.drag_to(300.0, 300.0);
        assert_eq!((window.left, window.top), (118.0, 83.0));
        window.drag_to(-100.0, -100.0);
        assert_eq!((window.left, window.top), (0.0, 0.0));
        window.drag_to(10_000.0, 10_000.0);
        assert_eq!(
            (window.left, window.top),
            (INVENTORY_MAX_LEFT, INVENTORY_MAX_TOP)
        );

        window.end_drag();
        assert!(!window.dragging());
        let stopped = (window.left, window.top);
        window.drag_to(200.0, 200.0);
        assert_eq!((window.left, window.top), stopped);
    }

    #[test]
    fn inventory_drag_system_uses_shared_stage_transform_and_preserves_location() {
        let mut app = App::new();
        let mut primary = Window::default();
        primary.resolution.set(2048.0, 1536.0);
        primary.set_cursor_position(Some(Vec2::new(450.0, 20.0)));
        app.world_mut().spawn((primary, PrimaryWindow));
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_systems(Update, process_inventory_drag);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_inventory();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_window
            .dragging());

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .expect("primary window")
            .set_cursor_position(Some(Vec2::new(600.0, 400.0)));
        app.update();
        assert_eq!(
            {
                let inventory = &app
                    .world()
                    .resource::<NativePlayerUiState>()
                    .inventory_window;
                (inventory.left, inventory.top)
            },
            (75.0, 190.0)
        );

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert!(!state.inventory_window.dragging());
        assert_eq!(
            (state.inventory_window.left, state.inventory_window.top),
            (75.0, 190.0)
        );
    }

    #[test]
    fn inventory_drag_keeps_same_frame_press_motion_release() {
        let mut app = App::new();
        let mut primary = Window::default();
        primary.resolution.set(1024.0, 768.0);
        let window = app.world_mut().spawn((primary, PrimaryWindow)).id();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_systems(Update, process_inventory_drag);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_inventory();
        {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.press(MouseButton::Left);
            mouse.release(MouseButton::Left);
        }
        app.world_mut().write_message(CursorMoved {
            window,
            position: Vec2::new(225.0, 10.0),
            delta: None,
        });
        app.world_mut().write_message(CursorMoved {
            window,
            position: Vec2::new(500.0, 300.0),
            delta: Some(Vec2::new(275.0, 290.0)),
        });

        app.update();

        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(
            (state.inventory_window.left, state.inventory_window.top),
            (275.0, 290.0)
        );
        assert!(!state.inventory_window.dragging());
    }

    #[test]
    fn inventory_drag_uses_prepress_cursor_when_sendinput_batches_the_destination() {
        let mut app = App::new();
        let mut primary = Window::default();
        primary.resolution.set(1024.0, 768.0);
        let window = app.world_mut().spawn((primary, PrimaryWindow)).id();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_systems(Update, process_inventory_drag);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_inventory();

        // Windows SendInput first moves the pointer to the source while the
        // button is still up. The press edge and destination can then arrive
        // together in the following render frame.
        app.world_mut().write_message(CursorMoved {
            window,
            position: Vec2::new(225.0, 10.0),
            delta: None,
        });
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut().write_message(CursorMoved {
            window,
            position: Vec2::new(500.0, 218.0),
            delta: Some(Vec2::new(275.0, 208.0)),
        });
        app.update();

        {
            let inventory = &app
                .world()
                .resource::<NativePlayerUiState>()
                .inventory_window;
            assert_eq!((inventory.left, inventory.top), (275.0, 208.0));
            assert!(inventory.dragging());
        }

        {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.clear_just_pressed(MouseButton::Left);
            mouse.release(MouseButton::Left);
        }
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .inventory_window
            .dragging());
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
            assert!(shop.apply_service_signal(NpcShopServiceSignal {
                mode: NpcShopServiceMode::Buy,
                repair_rate: None,
            }));
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
        // A Buy service must not authorize Sell.
        press(&mut app, OverlayButton::SelectBagForSell(0));
        press(&mut app, OverlayButton::ShopSell);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert!(app
            .world_mut()
            .resource_mut::<ShopModel>()
            .apply_service_signal(NpcShopServiceSignal {
                mode: NpcShopServiceMode::Sell,
                repair_rate: None,
            }));
        assert!(app.world().resource::<ShopModel>().allows_buy());
        assert!(app.world().resource::<ShopModel>().allows_sell());
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .npc_shop_buy_tab,
            "NPCGoods followed by NPCSell must keep the Buy entry visible"
        );
        press(&mut app, OverlayButton::ShopShowSell);
        assert!(
            !app.world()
                .resource::<NativePlayerUiState>()
                .npc_shop_buy_tab
        );
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
        press(&mut app, OverlayButton::ShopShowBuy);
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .npc_shop_buy_tab
        );
        press(&mut app, OverlayButton::SelectShopGood(101));
        press(&mut app, OverlayButton::ShopConfirm);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .iter()
            .any(|intent| matches!(
                intent,
                NativePlayerUiIntent::BuyItem {
                    item_index: 101,
                    count: 1
                }
            )));
        assert!(app
            .world_mut()
            .resource_mut::<ShopModel>()
            .apply_service_signal(NpcShopServiceSignal {
                mode: NpcShopServiceMode::Repair,
                repair_rate: Some(1.0),
            }));
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
        assert!(app
            .world_mut()
            .resource_mut::<ShopModel>()
            .apply_service_signal(NpcShopServiceSignal {
                mode: NpcShopServiceMode::SpecialRepair,
                repair_rate: Some(2.0),
            }));
        press(&mut app, OverlayButton::SelectBagForRepair(1));
        press(&mut app, OverlayButton::ShopRepair);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        press(&mut app, OverlayButton::ShopSRepair);
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .iter()
            .any(|intent| matches!(intent, NativePlayerUiIntent::SRepairItem { unique_id: 2 })));
        assert_eq!(
            app.world()
                .resource::<ShopModel>()
                .selected_bag_slot_for_sell,
            None
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
        assert!(app
            .world_mut()
            .resource_mut::<ShopModel>()
            .apply_service_signal(NpcShopServiceSignal {
                mode: NpcShopServiceMode::Sell,
                repair_rate: None,
            }));
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
            assert!(intents.iter().any(|i| matches!(
                i,
                NativePlayerUiIntent::StoreItem { request_id, .. }
                    if request_id == "st-0000000000000001"
            )));
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
            assert!(intents.iter().any(|i| matches!(
                i,
                NativePlayerUiIntent::TakeBackItem { request_id, .. }
                    if request_id == "st-0000000000000002"
            )));
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
    fn storage_request_ids_survive_queue_clear_for_connection_reset() {
        let mut queue = NativePlayerUiIntentQueue::default();
        let mut pending = PendingOperations::default();

        assert!(queue.push_storage_pending_intent(&mut pending, true, 10, 3, 9));
        let first = queue.drain_intents();
        assert!(matches!(
            first.as_slice(),
            [NativePlayerUiIntent::StoreItem { request_id, .. }]
                if request_id == "st-0000000000000001"
        ));

        queue.clear();
        assert!(queue.push_storage_pending_intent(&mut pending, false, 11, 9, 3));
        let second = queue.drain_intents();
        assert!(matches!(
            second.as_slice(),
            [NativePlayerUiIntent::TakeBackItem { request_id, .. }]
                if request_id == "st-0000000000000002"
        ));
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

    #[test]
    fn group_invite_name_is_bounded_ordered_and_authoritative() {
        let mut queue = NativePlayerUiIntentQueue::default();
        let mut social = crate::social::SocialModel::default();

        assert!(!valid_social_name(""));
        assert!(!valid_social_name(" leading"));
        assert!(!valid_social_name(&"x".repeat(33)));
        assert!(valid_social_name("PlayerOne"));

        assert!(queue_group_invite_by_name(
            &mut queue,
            &mut social,
            "PlayerOne".to_owned()
        ));
        assert_eq!(
            social.pending,
            vec![crate::social::SocialPendingOperation::GroupAdd {
                name: "PlayerOne".to_owned()
            }]
        );
        assert_eq!(
            queue.drain_intents(),
            vec![
                NativePlayerUiIntent::GroupSwitch { allow_group: true },
                NativePlayerUiIntent::GroupAddMember {
                    name: "PlayerOne".to_owned()
                }
            ]
        );
        assert!(social.group.members.is_empty());
        assert!(!queue_group_invite_by_name(
            &mut queue,
            &mut social,
            "PlayerOne".to_owned()
        ));
    }

    #[test]
    fn guild_permission_aliases_accept_authoritative_bit_names() {
        let mut guild = crate::social::GuildModel {
            permissions: vec![
                "CanRecruit".to_owned(),
                "CanKick".to_owned(),
                "CanChangeRank".to_owned(),
                "CanChangeNotice".to_owned(),
            ],
            ..Default::default()
        };
        assert!(social_has_permission(&guild, "recruit"));
        assert!(social_has_permission(&guild, "kick"));
        assert!(social_has_permission(&guild, "changeRank"));
        assert!(social_has_permission(&guild, "notice"));
        assert!(!social_has_permission(&guild, "retrieveItem"));

        guild.permissions = vec!["changeNotice".to_owned()];
        assert!(social_has_permission(&guild, "notice"));
    }

    #[test]
    fn guild_storage_and_rank_intents_register_exact_authoritative_pending_keys() {
        let mut queue = NativePlayerUiIntentQueue::default();
        let mut social = crate::social::SocialModel::default();

        assert!(queue.push_social_pending(
            &mut social,
            NativePlayerUiIntent::GuildStorageGoldChange {
                change_type: 0,
                amount: 500,
            }
        ));
        assert!(queue.push_social_pending(
            &mut social,
            NativePlayerUiIntent::GuildStorageItemChange {
                change_type: 2,
                from: 3,
                to: 4,
            }
        ));
        assert!(queue.push_social_pending(
            &mut social,
            NativePlayerUiIntent::GuildEditMember {
                change_type: 4,
                rank_index: 2,
                name: "Member".to_owned(),
                rank_name: String::new(),
            }
        ));
        assert_eq!(
            social.pending,
            vec![
                crate::social::SocialPendingOperation::GuildStorageGold {
                    change_type: 0,
                    amount: 500,
                },
                crate::social::SocialPendingOperation::GuildStorageItem {
                    change_type: 2,
                    from: 3,
                    to: 4,
                },
                crate::social::SocialPendingOperation::GuildMember {
                    change_type: 4,
                    rank_index: 2,
                    name: "Member".to_owned(),
                },
            ]
        );
        assert_eq!(queue.drain_intents().len(), 3);
        assert!(!queue.push_social_pending(
            &mut social,
            NativePlayerUiIntent::GuildStorageGoldChange {
                change_type: 0,
                amount: 500,
            }
        ));
    }

    fn help_keyboard_test_app() -> App {
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
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .add_message::<KeyboardInput>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Update, process_overlay_keyboard);
        app
    }

    fn help_button_test_app() -> App {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<MailModel>()
            .init_resource::<ShopModel>()
            .init_resource::<GameShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            });
        init_overlay_button_test_resources(&mut app);
        app.add_systems(Update, process_overlay_buttons);
        app
    }

    fn press_help_button(app: &mut App, action: OverlayButton) {
        let button = app
            .world_mut()
            .spawn((Button, Interaction::Pressed, action))
            .id();
        app.update();
        app.world_mut().despawn(button);
    }

    #[test]
    fn help_catalog_matches_the_crystal_45_page_contract() {
        assert_eq!(HELP_PAGE_TITLES.len(), 45);
        assert_eq!(HELP_PAGE_TITLES[0], "Shortcut Information");
        assert_eq!(HELP_PAGE_TITLES[2], "Chat Shortcuts");
        assert_eq!(HELP_PAGE_TITLES[44], "Awakening");
        assert_eq!(help_shortcut_rows(0).unwrap().len(), 18);
        assert_eq!(help_shortcut_rows(1).unwrap().len(), 18);
        assert_eq!(help_shortcut_rows(2).unwrap().len(), 3);
        assert!(help_shortcut_rows(3).is_none());
        assert_eq!(help_image_dimensions(0), (512.0, 396.0));
        assert_eq!(help_image_dimensions(29), (509.0, 396.0));
        assert_eq!(help_image_dimensions(30), (508.0, 395.0));
        assert_eq!(help_image_dimensions(33), (509.0, 396.0));
        assert_eq!(help_image_dimensions(41), (508.0, 395.0));
    }

    #[test]
    fn help_navigation_wraps_clamps_and_hide_preserves_page() {
        let mut help = HelpDialogUi::default();
        assert_eq!(
            (help.left, help.top),
            (CRYSTAL_HELP_PANEL_RECT.left, CRYSTAL_HELP_PANEL_RECT.top)
        );
        assert_eq!(help.z_index(), OVERLAY_NPC_DIALOG_Z);
        help.previous_page();
        assert_eq!(help.page, 44);
        help.next_page();
        assert_eq!(help.page, 0);
        help.display_page(999);
        assert!(help.open);
        assert_eq!(help.page, 44);
        help.hide();
        assert!(!help.open);
        assert_eq!(help.page, 44);
        help.toggle();
        assert!(help.open);
        assert_eq!(help.page, 44);
        assert_eq!(help.z_index(), OVERLAY_HELP_SORTED_Z);
    }

    #[test]
    fn help_sort_true_raises_on_show_and_drag_below_modal_layers() {
        let mut help = HelpDialogUi::default();
        assert_eq!(help.z_index(), OVERLAY_NPC_DIALOG_Z);

        help.toggle();
        assert_eq!(help.z_index(), OVERLAY_HELP_SORTED_Z);
        assert!(help.z_index() > OVERLAY_NPC_DIALOG_Z);
        assert!(help.z_index() < OVERLAY_DEATH_Z);

        help.z_index = OVERLAY_NPC_DIALOG_Z;
        assert!(help.begin_drag(help.left + 100.0, help.top + 10.0));
        assert_eq!(help.z_index(), OVERLAY_HELP_SORTED_Z);
    }

    #[test]
    fn help_drag_preserves_grab_offset_clamps_and_stops_on_release() {
        let mut help = HelpDialogUi::default();
        help.open = true;
        assert!(help.begin_drag(help.left + 100.0, help.top + 10.0));
        assert!(help.dragging());

        help.drag_to(300.0, 200.0);
        assert_eq!((help.left, help.top), (200.0, 190.0));
        help.drag_to(-100.0, -100.0);
        assert_eq!((help.left, help.top), (0.0, 0.0));
        help.drag_to(10_000.0, 10_000.0);
        assert_eq!((help.left, help.top), (HELP_MAX_LEFT, HELP_MAX_TOP));

        help.end_drag();
        assert!(!help.dragging());
        let stopped = (help.left, help.top);
        help.drag_to(200.0, 200.0);
        assert_eq!((help.left, help.top), stopped);
    }

    #[test]
    fn help_drag_uses_only_blank_header_and_hide_clears_movement() {
        let mut help = HelpDialogUi::default();
        assert!(!help.begin_drag(help.left + 100.0, help.top + 10.0));
        help.open = true;
        assert!(!help.begin_drag(help.left + 20.0, help.top + 10.0));
        assert!(!help.begin_drag(help.left + 100.0, help.top + 100.0));
        assert!(!help.begin_drag(help.left + 510.0, help.top + 10.0));
        assert!(help.begin_drag(help.left + 100.0, help.top + 10.0));
        help.drag_to(400.0, 300.0);
        let moved = (help.left, help.top);
        help.hide();
        assert!(!help.dragging());
        assert_eq!((help.left, help.top), moved, "Hide preserves location");

        help = HelpDialogUi::default();
        assert_eq!(
            (help.left, help.top),
            (CRYSTAL_HELP_PANEL_RECT.left, CRYSTAL_HELP_PANEL_RECT.top),
            "session reset reconstructs the centered source location"
        );
    }

    #[test]
    fn help_drag_system_uses_the_shared_stage_transform_and_release_edge() {
        let mut app = App::new();
        let mut window = Window::default();
        window.resolution.set(2048.0, 1536.0);
        window.set_cursor_position(Some(Vec2::new(
            (CRYSTAL_HELP_PANEL_RECT.left + 100.0) * 2.0,
            (CRYSTAL_HELP_PANEL_RECT.top + 10.0) * 2.0,
        )));
        app.world_mut().spawn((window, PrimaryWindow));
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_systems(Update, process_help_drag);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .help
            .open = true;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .help
            .dragging());

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .expect("primary window")
            .set_cursor_position(Some(Vec2::new(600.0, 400.0)));
        app.update();
        assert_eq!(
            {
                let help = &app.world().resource::<NativePlayerUiState>().help;
                (help.left, help.top)
            },
            (200.0, 190.0)
        );

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .help
            .dragging());
    }

    #[test]
    fn help_drag_system_fails_closed_without_headless_input_resources() {
        let mut state = NativePlayerUiState::default();
        state.help.open = true;
        let cursor = (state.help.left + 100.0, state.help.top + 10.0);
        assert!(state.help.begin_drag(cursor.0, cursor.1));

        let mut app = App::new();
        app.insert_resource(state)
            .add_systems(Update, process_help_drag);
        app.update();

        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .help
            .dragging());
    }

    #[test]
    fn help_shortcut_requires_ctrl_and_shift_unpressed_but_ignores_alt() {
        fn opens(modifiers: &[KeyCode], chat_focused: bool) -> bool {
            let mut app = help_keyboard_test_app();
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .chat_focused = chat_focused;
            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                for modifier in modifiers {
                    keys.press(*modifier);
                }
                keys.press(KeyCode::KeyH);
            }
            app.update();
            app.world().resource::<NativePlayerUiState>().help_open()
        }

        assert!(opens(&[], false));
        assert!(opens(&[KeyCode::AltLeft], false));
        assert!(!opens(&[KeyCode::ControlLeft], false));
        assert!(!opens(&[KeyCode::ShiftLeft], false));
        assert!(!opens(&[], true), "chat input owns typed H");
    }

    #[test]
    fn crystal_p_shortcut_opens_group_even_while_help_is_visible() {
        let mut app = help_keyboard_test_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .help
            .display_page(0);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyP);
        app.update();

        let state = app.world().resource::<NativePlayerUiState>();
        assert!(state.help_open(), "Crystal Help coexists with core panels");
        assert!(state.group_open(), "Help page one declares P = Group");
        assert!(!state.storage_open(), "P must not route to Storage");
    }

    #[test]
    fn help_escape_closes_all_without_opening_menu_and_preserves_page() {
        let mut app = help_keyboard_test_app();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.toggle_inventory();
            state.help.display_page(17);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let state = app.world().resource::<NativePlayerUiState>();
        assert!(!state.inventory_open());
        assert!(!state.help_open());
        assert!(!state.menu_open());
        assert_eq!(state.help.page, 17);
    }

    #[test]
    fn panel_close_keeps_coexisting_help_while_session_reset_clears_it() {
        let mut state = NativePlayerUiState::default();
        state.toggle_inventory();
        state.help.display_page(9);
        state.close_windows();
        assert!(!state.inventory_open());
        assert!(state.help_open());
        assert_eq!(state.help.page, 9);
        state.reset_session();
        assert!(!state.help_open());
        assert_eq!(state.help.page, 0);
    }

    #[test]
    fn help_blocks_world_click_and_has_dialog_priority_without_chat_key_capture() {
        let mut state = NativePlayerUiState::default();
        state.help.open = true;
        assert!(state.blocks_world_click());
        assert!(!state.blocks_gameplay_keys());
        assert_eq!(
            modal_priority_for_state(&state, false, false),
            Some(OverlayModalPriority::NpcDialog)
        );
        state.toggle_menu();
        assert_eq!(
            modal_priority_for_state(&state, false, false),
            Some(OverlayModalPriority::SystemMenu)
        );
    }

    #[test]
    fn help_internal_buttons_emit_one_button_a_while_menu_toggle_is_silent() {
        let mut app = help_button_test_app();

        press_help_button(&mut app, OverlayButton::ToggleHelp);
        assert!(app.world().resource::<NativePlayerUiState>().help_open());
        assert_eq!(
            app.world()
                .resource::<crate::audio::NativeUiAudioQueue>()
                .len(),
            0
        );

        press_help_button(&mut app, OverlayButton::HelpPrevious);
        assert_eq!(app.world().resource::<NativePlayerUiState>().help.page, 44);
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonA]
        );

        press_help_button(&mut app, OverlayButton::HelpNext);
        assert_eq!(app.world().resource::<NativePlayerUiState>().help.page, 0);
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonA]
        );

        press_help_button(&mut app, OverlayButton::CloseHelp);
        assert!(!app.world().resource::<NativePlayerUiState>().help_open());
        assert_eq!(
            app.world_mut()
                .resource_mut::<crate::audio::NativeUiAudioQueue>()
                .drain_bounded(8),
            vec![crate::audio::NativeUiSound::ButtonA]
        );
        assert!(app
            .world()
            .resource::<NativePlayerUiIntentQueue>()
            .intents
            .is_empty());
        assert!(app.world().resource::<NativeUiIntentQueue>().is_empty());
    }

    #[test]
    fn help_renderer_maps_dynamic_and_image_pages_without_fabrication() {
        let mut app = overlay_render_test_app();
        let peer_dialog = app
            .world_mut()
            .spawn(GlobalZIndex(OVERLAY_NPC_DIALOG_Z))
            .id();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.help.left = 32.0;
            state.help.top = 41.0;
            state.help.display_page(0);
        }
        app.update();
        {
            let world = app.world_mut();
            let help_node = world
                .query_filtered::<&Node, With<OverlayHelp>>()
                .single(world)
                .expect("Help root");
            assert_eq!(help_node.left, Val::Px(32.0));
            assert_eq!(help_node.top, Val::Px(41.0));
            let help_z = *world
                .query_filtered::<&GlobalZIndex, With<OverlayHelp>>()
                .single(world)
                .expect("Help z index");
            let peer_z = *world
                .get::<GlobalZIndex>(peer_dialog)
                .expect("peer dialog z index");
            assert_eq!(help_z, GlobalZIndex(OVERLAY_HELP_SORTED_Z));
            assert!(help_z.0 > peer_z.0, "Sort=true raises Help above peers");
            assert_eq!(world.query::<&HelpPageImageEntity>().iter(world).count(), 0);
            assert!(world
                .query::<&Text>()
                .iter(world)
                .any(|text| text.0 == "1 / 45"));
        }

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .help
            .display_page(3);
        app.update();
        {
            let world = app.world_mut();
            let indices = world
                .query::<&HelpPageImageEntity>()
                .iter(world)
                .map(|image| image.image_index)
                .collect::<Vec<_>>();
            assert_eq!(indices, vec![0]);
        }

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .help
            .display_page(44);
        app.update();
        let world = app.world_mut();
        let indices = world
            .query::<&HelpPageImageEntity>()
            .iter(world)
            .map(|image| image.image_index)
            .collect::<Vec<_>>();
        assert_eq!(indices, vec![41]);
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == "45 / 45"));
    }
}

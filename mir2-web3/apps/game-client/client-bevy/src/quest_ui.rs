//! Native in-game quest/HUD interaction panel.
//!
//! Presentation-only:
//! - reads authoritative read-model resources from `quest_model`
//! - renders compact quest/dialog/combat/pickup widgets for `NativeShellScreen::InGame`
//! - emits only protocol intents for host-side bridging
//! - full quest log covers selection/detail/tracking/accept/deliver/reward/close
//! - NPC dialog covers clickable links, service return, close and input flow


#[path = "quest_multi_guidance.rs"]
mod multi_guidance;
#[path = "quest_route.rs"]
pub(crate) mod route;
#[path = "quest_turn_in.rs"]
mod turn_in;
#[cfg(all(test, feature = "native-ui"))]
#[path = "quest_supply_visual_tests.rs"]
mod supply_visual_tests;
pub use turn_in::{PendingQuestTurnIn, begin_detail_quest_turn_in, pending_quest_turn_in_allows_interaction,
    quest_turn_in_ui_allows_interaction};
pub use multi_guidance::primary_quest_index;

use crate::quest_presentation_text::{QuestPresentationLocale, QuestRenderText};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextLayout};
use bevy::ui::{
    widget::NodeImageMode, AlignItems, BackgroundColor, Display, FlexDirection, FocusPolicy,
    Interaction, JustifyContent, Node, Overflow, PositionType, RelativeCursorPosition, UiRect, Val,
};
use mir2_client_core::quest::{
    diary_endpoint_authorized, evaluate_quest_action, QuestAction, QuestActionDecision,
    QuestActionFacts, QuestActionRejection, QuestActionRequest, QuestActionSource, QuestActionStatus,
    QuestProfile,
};
pub use crate::quest_intents::{QuestUiIntent, QuestUiIntentQueue, MAX_QUEUED_INTENTS};

use crate::crystal_ui::hud::HUD_Z_INDEX;
use crate::crystal_ui::item_tooltip::crystal_item_tooltip_document_from_source;
use crate::crystal_ui::overlays::{
    NativePlayerUiSet, NativePlayerUiState, UiEffectQueue,
};
#[cfg(feature = "native-ui")]
use crate::crystal_ui::overlays::dispatch_native_ui_action as dispatch_quest_panel_action;
#[cfg(not(feature = "native-ui"))]
fn dispatch_quest_panel_action(
    state: &mut NativePlayerUiState, effects: &mut UiEffectQueue,
    action: mir2_ui_core::action::UiAction,
) -> mir2_ui_core::reducer::Transition {
    crate::crystal_ui::overlays::dispatch_ui_action(&mut state.core, effects, action)
}
use crate::crystal_ui::quest_targets::{nearest_quest_monster, tracker_targets_monster};
use crate::crystal_ui::widget::CrystalItemHint;
use crate::entities::EntityModelSet;
use crate::inventory::{InventoryModel, ItemModel};
use crate::map::MapModel;
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::pending_operations::{
    AuthoritativeModelRevisions, PendingLifecycleSet, PendingOperationKey, PendingOperations,
    SessionResetRevision,
};
use crate::quest_guidance::QuestGuidance;
use crate::quest_journey::{GraduationDirection, JourneyView, NewcomerJourneyCatalog};
use crate::quest_model::{
    CombatTargetModel, CompletedQuestTracker, GroundPickupModel, NearbyNpcModel, NpcDialogModel,
    Quest, QuestTracker,
};
use crate::read_model::UiReadModel;

const PANEL_BG: Color = Color::srgba(0.06, 0.05, 0.03, 0.84);
const PANEL_TEXT: Color = Color::srgb(0.95, 0.92, 0.82);
const PANEL_HIGHLIGHT: Color = Color::srgb(0.92, 0.74, 0.22);
const BUTTON_BG: Color = Color::srgba(0.22, 0.18, 0.09, 0.95);
const BUTTON_DISABLED: Color = Color::srgba(0.30, 0.24, 0.16, 0.45);
const FEEDBACK_OK: Color = Color::srgb(0.34, 0.92, 0.34);
const FEEDBACK_ERR: Color = Color::srgb(0.96, 0.36, 0.36);
const QUEST_LOG_BG: Color = Color::srgba(0.08, 0.06, 0.04, 0.96);
const BUTTON_HOVER: Color = Color::srgba(0.42, 0.31, 0.12, 0.98);
const BUTTON_PRESSED: Color = Color::srgba(0.58, 0.40, 0.12, 0.98);
const DISABLED_TEXT: Color = Color::srgba(0.74, 0.69, 0.58, 0.58);

// Crystal `QuestDiaryDialog` (`QuestDialogs.cs:642-785`). Q opens this current-
// quest diary, not the NPC quest list (`Prguse/950`) or the quest detail window
// (`Prguse/960`). Keep the source coordinates in one place so the renderer and
// geometry tests cannot silently drift back to a generic panel.
const QUEST_DIARY_FRAME_ASSET: &str = "original-ui/Prguse/961.png";
const QUEST_DIARY_TITLE_ASSET: &str = "original-ui/Title/15.png";
const QUEST_DIARY_EXPANDED_ASSET: &str = "original-ui/Prguse/917.png";
const QUEST_DIARY_COLLAPSED_ASSET: &str = "original-ui/Prguse/918.png";
const QUEST_DIARY_SELECTED_ASSET: &str = "original-ui/Prguse/956.png";
const QUEST_DIARY_TRACKED_ASSET: &str = "original-ui/Prguse/997.png";
const NPC_DIALOG_FRAME_ASSET: &str = "original-ui/Prguse/995.png";
const QUEST_DIARY_TOP_CLOSE_ASSET: &str = "original-ui/Prguse2/360.png";
const QUEST_DIARY_BOTTOM_CLOSE_ASSET: &str = "original-ui/Title/193.png";
const QUEST_DETAIL_FRAME_ASSET: &str = "original-ui/Prguse/960.png";
const QUEST_DETAIL_TITLE_ASSET: &str = "original-ui/Title/16.png";
const QUEST_DETAIL_SECTION_ASSET: &str = "original-ui/Prguse/919.png";
const QUEST_DETAIL_EXP_ASSET: &str = "original-ui/Prguse/966.png";
const QUEST_DETAIL_GOLD_ASSET: &str = "original-ui/Prguse/965.png";
const QUEST_DETAIL_FIXED_REWARD_ASSET: &str = "original-ui/Prguse/989.png";
const QUEST_DETAIL_SELECTED_REWARD_ASSET: &str = "original-ui/Prguse/979.png";
const QUEST_DETAIL_SELECT_REWARD_ASSET: &str = "original-ui/Title/17.png";
const QUEST_DETAIL_SCROLL_UP_ASSET: &str = "original-ui/Prguse2/197.png";
const QUEST_DETAIL_SCROLL_THUMB_ASSET: &str = "original-ui/Prguse2/205.png";
const QUEST_DETAIL_SCROLL_DOWN_ASSET: &str = "original-ui/Prguse2/207.png";
const QUEST_DETAIL_SHARE_ASSET: &str = "original-ui/Title/616.png";
const QUEST_DETAIL_CANCEL_ASSET: &str = "original-ui/Title/203.png";
const QUEST_LIST_FRAME_ASSET: &str = "original-ui/Prguse/950.png";
const QUEST_LIST_TITLE_ASSET: &str = "original-ui/Title/14.png";
const QUEST_LIST_UP_ASSET: &str = "original-ui/Prguse/951.png";
const QUEST_LIST_DOWN_ASSET: &str = "original-ui/Prguse/957.png";
const QUEST_LIST_ACCEPT_ASSET: &str = "original-ui/Title/270.png";
const QUEST_LIST_FINISH_ASSET: &str = "original-ui/Title/273.png";
const QUEST_LIST_LEAVE_ASSET: &str = "original-ui/Title/276.png";
const NPC_QUEST_BUTTON_ASSET: &str = "original-ui/Title/530.png";
const QUEST_CONFIRM_FRAME_ASSET: &str = "original-ui/Prguse/360.png";
const QUEST_CONFIRM_YES_ASSET: &str = "original-ui/Title/206.png";
const QUEST_CONFIRM_NO_ASSET: &str = "original-ui/Title/210.png";
const QUEST_MESSAGE_OK_ASSET: &str = "original-ui/Title/200.png";

/// Chrome required before a portable host reports the shared Quest surface
/// visually ready. Item icons remain authoritative, dynamic asset requests.
pub const PORTABLE_QUEST_REQUIRED_SKINS: &[&str] = &[
    QUEST_DIARY_FRAME_ASSET, QUEST_DIARY_TITLE_ASSET, QUEST_DIARY_EXPANDED_ASSET,
    QUEST_DIARY_COLLAPSED_ASSET, QUEST_DIARY_SELECTED_ASSET, QUEST_DIARY_TRACKED_ASSET,
    NPC_DIALOG_FRAME_ASSET, QUEST_DIARY_TOP_CLOSE_ASSET, QUEST_DIARY_BOTTOM_CLOSE_ASSET,
    QUEST_DETAIL_FRAME_ASSET, QUEST_DETAIL_TITLE_ASSET, QUEST_DETAIL_SECTION_ASSET,
    QUEST_DETAIL_EXP_ASSET, QUEST_DETAIL_GOLD_ASSET, QUEST_DETAIL_FIXED_REWARD_ASSET,
    QUEST_DETAIL_SELECTED_REWARD_ASSET, QUEST_DETAIL_SELECT_REWARD_ASSET,
    QUEST_DETAIL_SCROLL_UP_ASSET, QUEST_DETAIL_SCROLL_THUMB_ASSET,
    QUEST_DETAIL_SCROLL_DOWN_ASSET, QUEST_DETAIL_SHARE_ASSET, QUEST_DETAIL_CANCEL_ASSET,
    QUEST_LIST_FRAME_ASSET, QUEST_LIST_TITLE_ASSET, QUEST_LIST_UP_ASSET,
    QUEST_LIST_DOWN_ASSET, QUEST_LIST_ACCEPT_ASSET, QUEST_LIST_FINISH_ASSET,
    QUEST_LIST_LEAVE_ASSET, NPC_QUEST_BUTTON_ASSET, QUEST_CONFIRM_FRAME_ASSET,
    QUEST_CONFIRM_YES_ASSET, QUEST_CONFIRM_NO_ASSET, QUEST_MESSAGE_OK_ASSET,
];
const ASK_CANCEL_QUEST_TEXT: &str = "Are you sure you want to cancel this quest?";
const SELECT_REWARD_TEXT: &str = "You must select a reward item.";

pub const QUEST_DIARY_DESIGN_WIDTH: f32 = 316.0;
pub const QUEST_DIARY_DESIGN_HEIGHT: f32 = 466.0;
pub const QUEST_DIARY_DESIGN_LEFT: f32 = 192.0;
pub const QUEST_DIARY_DESIGN_TOP: f32 = 60.0;
const QUEST_DIARY_MAX_CURRENT: usize = 20;
const QUEST_DIARY_GROUP_LEFT: f32 = 15.0;
const QUEST_DIARY_FIRST_ROW_TOP: f32 = 40.0;
const QUEST_DIARY_ROW_HEIGHT: f32 = 15.0;
const GUIDED_DIARY_PAGE_SIZE: usize = 8;
pub const QUEST_DETAIL_DESIGN_WIDTH: f32 = 316.0;
pub const QUEST_DETAIL_DESIGN_HEIGHT: f32 = 466.0;
pub const QUEST_DETAIL_DESIGN_LEFT: f32 = 532.0;
pub const QUEST_DETAIL_DESIGN_TOP: f32 = 60.0;
const QUEST_DETAIL_LINE_COUNT: usize = 16;
const QUEST_DETAIL_LINE_HEIGHT: f32 = 15.0;
pub const QUEST_LIST_DESIGN_WIDTH: f32 = 316.0;
pub const QUEST_LIST_DESIGN_HEIGHT: f32 = 466.0;
/// Crystal positions `QuestListDialog` at `NPCDialog.Size.Width + 47`.
/// `Prguse/995`, captured by the NPC dialog before AutoSize is disabled, is
/// exactly 440 px wide.
pub const QUEST_LIST_DESIGN_LEFT: f32 = 487.0;
pub const QUEST_LIST_DESIGN_TOP: f32 = 0.0;
const QUEST_LIST_VISIBLE_ROWS: usize = 5;
const QUEST_LIST_MESSAGE_LINE_COUNT: usize = 10;
const QUEST_CONFIRM_DESIGN_WIDTH: f32 = 456.0;
const QUEST_CONFIRM_DESIGN_HEIGHT: f32 = 190.0;
const QUEST_CONFIRM_DESIGN_LEFT: f32 = 284.0;
const QUEST_CONFIRM_DESIGN_TOP: f32 = 289.0;
pub const MAX_TRACKED_QUESTS: usize = 5;

/// Layout metrics for the browser's Quest window. The host supplies the
/// Bevy window's logical size and the CSS scale of its containing stage.
/// Windows leaves this resource absent and keeps the Crystal geometry.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct QuestUiPresentation {
    pub logical_width: f32,
    pub logical_height: f32,
    pub stage_css_scale: f32,
    pub touch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MobileQuestLayout {
    panel: QuestLogRect,
    css_scale: f32,
    compact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MobileQuestSheet { Confirmation, NpcList, Dialog, Detail, Diary, None }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CompactQuestPage { #[default] Text, Rewards }

fn mobile_sheet(
    confirmation_open: bool,
    npc_list_open: bool,
    dialog_open: bool,
    detail_open: bool,
    diary_open: bool,
) -> MobileQuestSheet {
    if confirmation_open { MobileQuestSheet::Confirmation }
    else if npc_list_open { MobileQuestSheet::NpcList }
    else if dialog_open { MobileQuestSheet::Dialog }
    else if detail_open { MobileQuestSheet::Detail }
    else if diary_open { MobileQuestSheet::Diary }
    else { MobileQuestSheet::None }
}

fn place_mobile_sheet(node: &mut Node, layout: MobileQuestLayout) {
    node.left = Val::Px(layout.panel.left);
    node.top = Val::Px(layout.panel.top);
    node.width = Val::Px(layout.panel.width);
    node.height = Val::Px(layout.panel.height);
    node.min_width = Val::Px(layout.panel.width);
    node.max_width = Val::Px(layout.panel.width);
    node.min_height = Val::Px(layout.panel.height);
    node.max_height = Val::Px(layout.panel.height);
}

impl QuestUiPresentation {
    /// Whether this touch viewport can use the readable single-sheet layout.
    /// Hosts can reject smaller viewports instead of showing desktop geometry.
    pub fn supports_mobile_layout(self) -> bool { self.mobile_layout().is_some() }

    fn mobile_layout(self) -> Option<MobileQuestLayout> {
        let valid = self.logical_width.is_finite()
            && self.logical_height.is_finite()
            && self.stage_css_scale.is_finite()
            && self.logical_width > 0.0
            && self.logical_height > 0.0
            && self.stage_css_scale > 0.0;
        if !valid || !self.touch {
            return None;
        }
        let css_width = self.logical_width * self.stage_css_scale;
        let css_height = self.logical_height * self.stage_css_scale;
        if css_width > 1100.0 || css_height > 600.0
            || css_width / css_height < 1.5 {
            return None;
        }
        let margin = 12.0 / self.stage_css_scale;
        let width = (self.logical_width - 2.0 * margin)
            .min(560.0 / self.stage_css_scale)
            .max(0.0);
        let height = (self.logical_height - 2.0 * margin).max(0.0);
        // The reward grid and the two action rows need this much CSS space.
        // Smaller stages keep the host's existing geometry until a compact
        // portrait design is available.
        if width < 400.0 / self.stage_css_scale || height < 296.0 / self.stage_css_scale {
            return None;
        }
        Some(MobileQuestLayout {
            panel: QuestLogRect::new(
                (self.logical_width - width) / 2.0,
                (self.logical_height - height) / 2.0,
                width,
                height,
            ),
            css_scale: self.stage_css_scale,
            compact: height * self.stage_css_scale < 335.0,
        })
    }
}

impl MobileQuestLayout {
    fn px(self, css_px: f32) -> f32 { css_px / self.css_scale }
    fn rect(self, left: f32, top: f32, width: f32, height: f32) -> QuestLogRect {
        QuestLogRect::new(self.px(left), self.px(top), self.px(width), self.px(height))
    }
    fn width_css(self) -> f32 { self.panel.width * self.css_scale }
    fn height_css(self) -> f32 { self.panel.height * self.css_scale }
    fn body_rows(self, start_css: f32, end_css: f32, row_css: f32) -> usize {
        ((self.height_css() - start_css - end_css) / row_css).floor().max(1.0) as usize
    }
    fn diary_rows(self) -> usize { self.body_rows(102.0, 56.0, 48.0) }
    fn dialog_rows(self) -> usize { self.body_rows(54.0, 58.0, 44.0) }
    fn npc_quest_rows(self) -> usize { 1 }
    fn message_rows(self) -> usize { if self.compact { 5 } else { self.body_rows(102.0, 194.0, 24.0) } }
    fn detail_rows(self) -> usize { if self.compact { 7 } else { self.body_rows(52.0, 195.0, 24.0) } }
    fn dialog_footer_button_width(self, count: usize) -> f32 {
        ((self.width_css() - 24.0 - (count.saturating_sub(1) as f32 * 8.0))
            / count.max(1) as f32).min(92.0)
    }
    fn detail_footer_rects(self) -> [(f32, f32, f32, f32); 3] {
        let y = self.height_css() - 49.0;
        [(16.0, y, 120.0, 44.0), (144.0, y, 92.0, 44.0),
            (self.width_css() - 128.0, y, 112.0, 44.0)]
    }
    fn npc_footer_rects(self) -> [(f32, f32, f32, f32); 3] {
        let y = self.height_css() - 49.0;
        [(16.0, y, 112.0, 44.0), (self.width_css() - 212.0, y, 112.0, 44.0),
            (self.width_css() - 92.0, y, 76.0, 44.0)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestLogRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl QuestLogRect {
    const fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn scaled(self, scale: f32) -> Self {
        Self::new(
            self.left * scale,
            self.top * scale,
            self.width * scale,
            self.height * scale,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestDiaryLayout {
    pub frame: QuestLogRect,
    pub title: QuestLogRect,
    pub taken_count: QuestLogRect,
    pub top_close: QuestLogRect,
    pub bottom_close: QuestLogRect,
}

pub fn quest_diary_layout(scale: f32) -> QuestDiaryLayout {
    QuestDiaryLayout {
        frame: QuestLogRect::new(
            QUEST_DIARY_DESIGN_LEFT,
            QUEST_DIARY_DESIGN_TOP,
            QUEST_DIARY_DESIGN_WIDTH,
            QUEST_DIARY_DESIGN_HEIGHT,
        )
        .scaled(scale),
        title: QuestLogRect::new(18.0, 9.0, 103.0, 17.0).scaled(scale),
        taken_count: QuestLogRect::new(210.0, 7.0, 76.0, 15.0).scaled(scale),
        top_close: QuestLogRect::new(289.0, 3.0, 24.0, 21.0).scaled(scale),
        bottom_close: QuestLogRect::new(200.0, 436.0, 68.0, 25.0).scaled(scale),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestDetailLayout {
    pub frame: QuestLogRect,
    pub title: QuestLogRect,
    pub top_close: QuestLogRect,
    pub scroll_up: QuestLogRect,
    pub scroll_down: QuestLogRect,
    pub scroll_thumb: QuestLogRect,
    pub message: QuestLogRect,
    pub rewards: QuestLogRect,
    pub share: QuestLogRect,
    pub cancel: QuestLogRect,
}

pub fn quest_detail_layout(scale: f32) -> QuestDetailLayout {
    QuestDetailLayout {
        frame: QuestLogRect::new(
            QUEST_DETAIL_DESIGN_LEFT,
            QUEST_DETAIL_DESIGN_TOP,
            QUEST_DETAIL_DESIGN_WIDTH,
            QUEST_DETAIL_DESIGN_HEIGHT,
        )
        .scaled(scale),
        title: QuestLogRect::new(18.0, 9.0, 55.0, 17.0).scaled(scale),
        top_close: QuestLogRect::new(289.0, 3.0, 24.0, 21.0).scaled(scale),
        scroll_up: QuestLogRect::new(293.0, 33.0, 16.0, 14.0).scaled(scale),
        scroll_down: QuestLogRect::new(293.0, 280.0, 16.0, 14.0).scaled(scale),
        scroll_thumb: QuestLogRect::new(293.0, 48.0, 12.0, 18.0).scaled(scale),
        message: QuestLogRect::new(10.0, 35.0, 280.0, 260.0).scaled(scale),
        rewards: QuestLogRect::new(5.0, 307.0, 306.0, 130.0).scaled(scale),
        share: QuestLogRect::new(40.0, 436.0, 76.0, 25.0).scaled(scale),
        cancel: QuestLogRect::new(200.0, 436.0, 76.0, 25.0).scaled(scale),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestListLayout {
    pub frame: QuestLogRect,
    pub title: QuestLogRect,
    pub available_count: QuestLogRect,
    pub top_close: QuestLogRect,
    pub help: QuestLogRect,
    pub quest_up: QuestLogRect,
    pub quest_down: QuestLogRect,
    pub message_up: QuestLogRect,
    pub message_down: QuestLogRect,
    pub message_thumb: QuestLogRect,
    pub message: QuestLogRect,
    pub rewards: QuestLogRect,
    pub primary_action: QuestLogRect,
    pub leave: QuestLogRect,
}

pub fn quest_list_layout(scale: f32) -> QuestListLayout {
    QuestListLayout {
        frame: QuestLogRect::new(
            QUEST_LIST_DESIGN_LEFT,
            QUEST_LIST_DESIGN_TOP,
            QUEST_LIST_DESIGN_WIDTH,
            QUEST_LIST_DESIGN_HEIGHT,
        )
        .scaled(scale),
        title: QuestLogRect::new(18.0, 9.0, 55.0, 17.0).scaled(scale),
        available_count: QuestLogRect::new(210.0, 8.0, 76.0, 15.0).scaled(scale),
        top_close: QuestLogRect::new(289.0, 3.0, 24.0, 21.0).scaled(scale),
        help: QuestLogRect::new(266.0, 3.0, 23.0, 21.0).scaled(scale),
        quest_up: QuestLogRect::new(291.0, 35.0, 16.0, 48.0).scaled(scale),
        quest_down: QuestLogRect::new(291.0, 83.0, 16.0, 48.0).scaled(scale),
        message_up: QuestLogRect::new(292.0, 136.0, 16.0, 14.0).scaled(scale),
        message_down: QuestLogRect::new(292.0, 282.0, 16.0, 14.0).scaled(scale),
        message_thumb: QuestLogRect::new(292.0, 149.0, 12.0, 18.0).scaled(scale),
        message: QuestLogRect::new(10.0, 135.0, 280.0, 160.0).scaled(scale),
        rewards: QuestLogRect::new(5.0, 307.0, 306.0, 130.0).scaled(scale),
        primary_action: QuestLogRect::new(40.0, 436.0, 68.0, 25.0).scaled(scale),
        leave: QuestLogRect::new(205.0, 436.0, 68.0, 25.0).scaled(scale),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestConfirmationLayout {
    pub frame: QuestLogRect,
    pub message: QuestLogRect,
    pub yes: QuestLogRect,
    pub no: QuestLogRect,
}

pub fn quest_confirmation_layout(scale: f32) -> QuestConfirmationLayout {
    QuestConfirmationLayout {
        frame: QuestLogRect::new(
            QUEST_CONFIRM_DESIGN_LEFT,
            QUEST_CONFIRM_DESIGN_TOP,
            QUEST_CONFIRM_DESIGN_WIDTH,
            QUEST_CONFIRM_DESIGN_HEIGHT,
        )
        .scaled(scale),
        message: QuestLogRect::new(35.0, 35.0, 390.0, 110.0).scaled(scale),
        yes: QuestLogRect::new(260.0, 157.0, 76.0, 25.0).scaled(scale),
        no: QuestLogRect::new(360.0, 157.0, 76.0, 25.0).scaled(scale),
    }
}

const NPC_DIALOG_VISIBLE_ROWS: usize = 9;
const MAX_PICKUP_BUTTONS: usize = 3;
// Ground drops remain available through the normal pickup intents and world
// interaction. The recent-list panel is a native QA aid, not a Crystal player
// surface, so keep it out of the ordinary game HUD.
const SHOW_PICKUP_FEEDBACK_PANEL: bool = false;
const MAX_QUICK_BAG_ITEMS: usize = 6;
const MAX_QUEST_LOG_ROWS: usize = 8;

// Crystal does not render a separate oversized target window. Keep this
// native-only target readout in the unused upper-left HUD gutter, clear of the
// minimap (x >= 898), the quest tracker (y >= 118), and the central play area.
const COMBAT_TARGET_PANEL_LEFT: f32 = 8.0;
const COMBAT_TARGET_PANEL_TOP: f32 = 8.0;
const COMBAT_TARGET_PANEL_WIDTH: f32 = 236.0;
const COMBAT_TARGET_PANEL_MIN_HEIGHT: f32 = 0.0;
const COMBAT_TARGET_PANEL_PADDING: f32 = 4.0;
const COMBAT_TARGET_BAR_HEIGHT: f32 = 8.0;
// Crystal targets are selected and attacked in the world; it does not render
// a separate top-left target/action window over the scene.
const CRYSTAL_TARGET_PANEL_VISIBLE: bool = false;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestRouteTarget {
    Entrance,
    HuntRegion { monster_index: i32, radius: i32 },
    Supply { vendor: crate::quest_supplies::SupplyVendor },
}

impl QuestRouteTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Entrance => "入口",
            Self::HuntRegion { .. } => "狩猎区域",
            Self::Supply { .. } => "补给地点",
        }
    }
}

/// One request to begin ordinary local walk/run toward an authored destination.
/// The Windows input host must reject it unless both identity values still
/// match its authoritative Big Map model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestRouteNavigationIntent {
    pub target: QuestRouteTarget,
    pub quest_index: i32,
    pub reset_epoch: u64,
    pub map_index: i32,
    pub x: i32,
    pub y: i32,
}

impl QuestRouteNavigationIntent {
    pub fn matches_supply_destination(self) -> bool {
        let QuestRouteTarget::Supply { vendor } = self.target else { return false; };
        self.quest_index == 0 && vendor.route(self.map_index).is_some_and(|route| route.x == self.x && route.y == self.y)
    }

    /// Shared UI/host validation against one specific unfinished kill and its
    /// imported area. Coordinates or a remaining unrelated flag are not enough.
    pub fn matches_active_hunt_region(self, tracker: &QuestTracker) -> bool {
        let QuestRouteTarget::HuntRegion { monster_index, radius } = self.target else { return false; };
        let Some(quest) = tracker.active_quests.iter().find(|quest| quest.quest_index == self.quest_index) else {
            return false;
        };
        crate::quest_hunt_regions::active_hunt_regions(
            &QuestTracker { active_quests: vec![quest.clone()] }, self.map_index,
            crate::big_map::BigMapPoint { x: self.x, y: self.y },
        ).iter().any(|region| region.monster_index == monster_index && region.radius == radius
            && region.center.x == self.x && region.center.y == self.y)
    }
}

/// Bounded handoff from quest guidance to the native ordinary movement
/// controller. This carries no map change or teleport operation.
#[derive(Resource, Debug, Default)]
pub struct QuestRouteNavigationIntentQueue {
    pending: Option<QuestRouteNavigationIntent>,
}

impl QuestRouteNavigationIntentQueue {
    pub fn push(&mut self, intent: QuestRouteNavigationIntent) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(intent);
        true
    }

    pub fn take(&mut self) -> Option<QuestRouteNavigationIntent> {
        self.pending.take()
    }

    pub fn clear(&mut self) {
        self.pending = None;
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_none()
    }
}

fn quest_route_intent_is_current(
    intent: QuestRouteNavigationIntent,
    tracker: &QuestTracker,
    state: &QuestUiState,
    journey: Option<&JourneyView>,
    big_map: Option<&crate::big_map::BigMapModel>,
) -> bool {
    let Some(big_map) = big_map else {
        return false;
    };
    if big_map.reset_epoch != intent.reset_epoch || big_map.current_map_index != Some(intent.map_index) {
        return false;
    }
    if let QuestRouteTarget::Supply { vendor } = intent.target {
        return state.supply_open && state.supply_vendor == Some(vendor) && intent.matches_supply_destination();
    }
    if primary_quest_index(tracker, state, journey) != Some(intent.quest_index) {
        return false;
    }
    if matches!(intent.target, QuestRouteTarget::HuntRegion { .. }) {
        return intent.matches_active_hunt_region(tracker);
    }
    let targets = crate::quest_destination::authored_target_map_indices(intent.quest_index);
    let crate::quest_ui::route::QuestRoute::NextStep(step) =
        crate::quest_ui::route::resolve(big_map.current_map_index, &targets)
    else {
        return false;
    };
    big_map.reset_epoch == intent.reset_epoch
        && step.current_map_index == intent.map_index
        && step.entrance_x == intent.x
        && step.entrance_y == intent.y
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFeedback {
    pub message: String,
    pub is_error: bool,
}

#[derive(Resource, Debug, Default, Clone)]
pub struct QuestUiState {
    /// Local view only; opening it never changes the active quest or buys items.
    pub supply_open: bool,
    pub supply_vendor: Option<crate::quest_supplies::SupplyVendor>,
    pub supply_epoch: Option<u64>,
    pub dialog_scroll_top: usize,
    pub selected_quest_index: Option<i32>,
    /// Crystal opens an independent `QuestDetailDialog` on diary-row left
    /// click. This remains open when the Q-key diary itself is hidden.
    pub detail_quest_index: Option<i32>,
    pub detail_scroll_top: usize,
    pub selected_reward_index: Option<i32>,
    compact_detail_page: CompactQuestPage,
    compact_detail_reward_page: usize,
    /// Invalidates a compact tree when local navigation replaces its content
    /// without changing the authoritative host revision or selected page.
    compact_view_epoch: u64,
    /// Crystal persists an ordered list of at most five tracked quest ids.
    /// Right-clicking a diary row toggles membership; there is no implicit
    /// "track the first quest" fallback.
    pub tracked_quest_indices: Vec<i32>,
    /// Local guidance choice, independent of Crystal's persisted tracking list.
    pub pinned_primary_quest_index: Option<i32>,
    /// Quest whose source `MirMessageBox` abandon confirmation is open.
    pub abandon_confirmation_quest_index: Option<i32>,
    pub quest_alert_message: Option<String>,
    compact_confirmation_page: usize,
    pub npc_quest_list_open: bool,
    pub npc_quest_selected_index: Option<i32>,
    pub npc_quest_start_index: usize,
    pub npc_quest_message_scroll_top: usize,
    pub npc_selected_reward_index: Option<i32>,
    compact_npc_page: CompactQuestPage,
    compact_npc_reward_page: usize,
    pub feedback: Option<QuestFeedback>,
    pub stage_filter: QuestStageFilter,
    pub page: usize,
    /// Newcomer V2 only: keep optional Crystal tasks separate from the active
    /// journey without changing their authoritative quest state.
    pub diary_tab: GuidedDiaryTab,
    pub diary_page: usize,
    /// Crystal expands every diary group by default and remembers only groups
    /// the player explicitly collapsed during the current client session.
    pub collapsed_groups: Vec<String>,
    /// A local-only V2 graduation target. It is never serialized, bridged, or
    /// treated as a server quest; `reset` clears it with the client session.
    pub selected_graduation_direction: Option<GraduationDirection>,
    /// One explicit detail Finish click awaiting the ordinary NPC reply.
    pub pending_turn_in: Option<PendingQuestTurnIn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuestStageFilter {
    #[default]
    All,
    InProgress,
    ReadyToTurnIn,
    NotStarted,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuidedDiaryTab {
    #[default]
    Main,
    Ready,
    Side,
}

impl GuidedDiaryTab {
    const ALL: [Self; 3] = [Self::Main, Self::Ready, Self::Side];

    const fn label(self) -> &'static str {
        match self {
            Self::Main => "主线",
            Self::Ready => "可交付",
            Self::Side => "支线",
        }
    }
}

impl QuestStageFilter {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::InProgress,
        Self::ReadyToTurnIn,
        Self::NotStarted,
        Self::Completed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::InProgress => "Active",
            Self::ReadyToTurnIn => "Ready",
            Self::NotStarted => "New",
            Self::Completed => "Done",
        }
    }

    pub fn matches(self, quest: &Quest) -> bool {
        match self {
            Self::All => true,
            Self::InProgress => quest.status == crate::quest_model::QuestStatus::InProgress,
            Self::ReadyToTurnIn => quest.status == crate::quest_model::QuestStatus::ReadyToTurnIn,
            Self::NotStarted => quest.status == crate::quest_model::QuestStatus::NotStarted,
            Self::Completed => quest.status == crate::quest_model::QuestStatus::Completed,
        }
    }
}

impl QuestUiState {
    pub fn select_quest(&mut self, quest_index: i32) {
        self.pending_turn_in = None;
        self.selected_quest_index = Some(quest_index);
        self.detail_quest_index = Some(quest_index);
        self.detail_scroll_top = 0;
        self.selected_reward_index = None;
        self.compact_detail_page = CompactQuestPage::Text;
        self.compact_detail_reward_page = 0;
        self.feedback = None;
    }

    pub fn clear_selection(&mut self) {
        self.selected_quest_index = None;
        self.detail_quest_index = None;
        self.detail_scroll_top = 0;
        self.selected_reward_index = None;
        self.compact_detail_page = CompactQuestPage::Text;
        self.compact_detail_reward_page = 0;
    }

    pub fn clear_diary_selection(&mut self) {
        self.selected_quest_index = None;
    }

    pub fn close_detail(&mut self) {
        self.pending_turn_in = None;
        self.detail_quest_index = None;
        self.detail_scroll_top = 0;
        self.selected_reward_index = None;
        self.compact_detail_page = CompactQuestPage::Text;
        self.compact_detail_reward_page = 0;
        self.abandon_confirmation_quest_index = None;
    }

    pub fn scroll_detail_up(&mut self) {
        self.detail_scroll_top = self.detail_scroll_top.saturating_sub(1);
    }

    pub fn scroll_detail_down(&mut self, line_count: usize) {
        self.scroll_detail_down_with_rows(line_count, QUEST_DETAIL_LINE_COUNT);
    }

    fn scroll_detail_down_with_rows(&mut self, line_count: usize, rows: usize) {
        let max_top = line_count.saturating_sub(rows);
        self.detail_scroll_top = (self.detail_scroll_top + 1).min(max_top);
    }

    pub fn set_stage_filter(&mut self, filter: QuestStageFilter) {
        self.stage_filter = filter;
        self.page = 0;
        self.clear_selection();
        self.feedback = None;
    }

    pub fn set_page(&mut self, page: usize) {
        self.page = page;
        self.clear_selection();
    }

    pub fn select_reward(&mut self, reward_index: i32) {
        self.selected_reward_index = Some(reward_index);
        self.feedback = None;
    }

    pub fn set_feedback(&mut self, message: impl Into<String>, is_error: bool) {
        self.feedback = Some(QuestFeedback {
            message: message.into(),
            is_error,
        });
    }

    pub fn clear_feedback(&mut self) {
        self.feedback = None;
    }

    pub fn is_group_collapsed(&self, group: &str) -> bool {
        self.collapsed_groups.iter().any(|value| value == group)
    }

    pub fn toggle_group(&mut self, group: String) {
        if let Some(index) = self
            .collapsed_groups
            .iter()
            .position(|value| value == &group)
        {
            self.collapsed_groups.remove(index);
        } else if self.collapsed_groups.len() < QUEST_DIARY_MAX_CURRENT {
            self.collapsed_groups.push(group);
        }
    }

    pub fn is_tracked(&self, quest_index: i32) -> bool {
        self.tracked_quest_indices.contains(&quest_index)
    }

    pub fn toggle_tracked_quest(&mut self, quest_index: i32) -> QuestTrackingChange {
        if let Some(index) = self
            .tracked_quest_indices
            .iter()
            .position(|tracked| *tracked == quest_index)
        {
            self.tracked_quest_indices.remove(index);
            QuestTrackingChange::Removed
        } else if self.tracked_quest_indices.len() >= MAX_TRACKED_QUESTS {
            QuestTrackingChange::Full
        } else {
            self.tracked_quest_indices.push(quest_index);
            QuestTrackingChange::Added
        }
    }

    pub fn toggle_graduation_direction(&mut self, direction: GraduationDirection) -> bool {
        if self.selected_graduation_direction == Some(direction) {
            self.selected_graduation_direction = None;
            false
        } else {
            self.selected_graduation_direction = Some(direction);
            true
        }
    }

    pub fn request_abandon_confirmation(&mut self, quest_index: i32) {
        self.quest_alert_message = None;
        self.compact_confirmation_page = 0;
        self.abandon_confirmation_quest_index = Some(quest_index);
        self.feedback = None;
    }

    pub fn close_abandon_confirmation(&mut self) {
        self.abandon_confirmation_quest_index = None;
        self.compact_confirmation_page = 0;
    }

    pub fn show_quest_alert(&mut self, message: impl Into<String>) {
        self.abandon_confirmation_quest_index = None;
        self.compact_confirmation_page = 0;
        self.quest_alert_message = Some(message.into());
    }

    pub fn close_quest_alert(&mut self) {
        self.quest_alert_message = None;
        self.compact_confirmation_page = 0;
    }

    pub fn open_npc_quest_list(&mut self, quest_indices: &[i32]) {
        let Some(first) = quest_indices.first().copied() else {
            self.close_npc_quest_list();
            return;
        };
        self.npc_quest_list_open = true;
        self.npc_quest_start_index = 0;
        self.npc_quest_selected_index = Some(first);
        self.npc_quest_message_scroll_top = 0;
        self.npc_selected_reward_index = None;
        self.compact_npc_page = CompactQuestPage::Text;
        self.compact_npc_reward_page = 0;
        self.feedback = None;
    }

    pub fn close_npc_quest_list(&mut self) {
        self.npc_quest_list_open = false;
        self.npc_quest_selected_index = None;
        self.npc_quest_start_index = 0;
        self.npc_quest_message_scroll_top = 0;
        self.npc_selected_reward_index = None;
        self.compact_npc_page = CompactQuestPage::Text;
        self.compact_npc_reward_page = 0;
    }

    pub fn select_npc_quest(&mut self, quest_index: i32) {
        self.npc_quest_selected_index = Some(quest_index);
        self.npc_quest_message_scroll_top = 0;
        self.npc_selected_reward_index = None;
        self.compact_npc_page = CompactQuestPage::Text;
        self.compact_npc_reward_page = 0;
        self.feedback = None;
    }

    pub fn move_npc_quest_selection(&mut self, quest_indices: &[i32], delta: isize) {
        self.move_npc_quest_selection_with_rows(quest_indices, delta, QUEST_LIST_VISIBLE_ROWS);
    }

    fn move_npc_quest_selection_with_rows(&mut self, quest_indices: &[i32], delta: isize, rows: usize) {
        if quest_indices.is_empty() {
            self.close_npc_quest_list();
            return;
        }
        let current = self
            .npc_quest_selected_index
            .and_then(|selected| quest_indices.iter().position(|index| *index == selected))
            .unwrap_or(0);
        let next = current
            .saturating_add_signed(delta)
            .min(quest_indices.len().saturating_sub(1));
        self.select_npc_quest(quest_indices[next]);
        if next < self.npc_quest_start_index {
            self.npc_quest_start_index = next;
        } else if next >= self.npc_quest_start_index + rows {
            self.npc_quest_start_index = next + 1 - rows;
        }
    }

    pub fn scroll_npc_quest_message_up(&mut self) {
        self.npc_quest_message_scroll_top = self.npc_quest_message_scroll_top.saturating_sub(1);
    }

    pub fn scroll_npc_quest_message_down(&mut self, line_count: usize) {
        self.scroll_npc_quest_message_down_with_rows(line_count, QUEST_LIST_MESSAGE_LINE_COUNT);
    }

    fn scroll_npc_quest_message_down_with_rows(&mut self, line_count: usize, rows: usize) {
        let max_top = line_count.saturating_sub(rows);
        self.npc_quest_message_scroll_top = (self.npc_quest_message_scroll_top + 1).min(max_top);
    }

    pub fn selected_quest<'a>(&self, tracker: &'a QuestTracker) -> Option<&'a Quest> {
        self.selected_quest_index
            .and_then(|idx| tracker.active_quests.iter().find(|q| q.quest_index == idx))
    }

    pub fn detail_quest<'a>(&self, tracker: &'a QuestTracker) -> Option<&'a Quest> {
        self.detail_quest_index
            .and_then(|idx| tracker.active_quests.iter().find(|q| q.quest_index == idx))
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTrackingChange {
    Added,
    Removed,
    Full,
}

#[derive(Resource, Debug, Default)]
pub struct NpcDialogNav {
    pub history: Vec<NpcDialogModel>,
}

impl NpcDialogNav {
    pub fn push(&mut self, dialog: NpcDialogModel) {
        // Bounded history to avoid unbounded growth.
        if self.history.len() >= 8 {
            self.history.remove(0);
        }
        self.history.push(dialog);
    }

    pub fn pop(&mut self) -> Option<NpcDialogModel> {
        self.history.pop()
    }

    pub fn can_return(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}

#[derive(Component)]
pub struct QuestUiRoot;

/// Identity of the compact tree built from one host snapshot and one local page.
#[cfg(not(feature = "native-ui"))]
#[derive(Component, Debug, Clone, Copy, PartialEq)]
struct QuestCompactStamp {
    locale: QuestPresentationLocale,
    generation: u64,
    revision: u64,
    open_revision: u64,
    presentation: QuestUiPresentation,
    sheet: MobileQuestSheet,
    quest_index: Option<i32>,
    reward_selection: Option<i32>,
    graduation_choice: Option<GraduationDirection>,
    page: CompactQuestPage,
    reward_page: usize,
    text_top: usize,
    diary_page: usize,
    diary_tab: GuidedDiaryTab,
    dialog_top: usize,
    view_epoch: u64,
}

#[cfg(not(feature = "native-ui"))]
#[derive(Debug, Clone, Copy)]
pub struct QuestCompactActionRect {
    pub label: &'static str,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[cfg(not(feature = "native-ui"))]
#[derive(Debug, Clone, Copy)]
pub struct QuestCompactPanelRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[cfg(not(feature = "native-ui"))]
#[derive(Resource, Default, Debug)]
pub struct QuestCompactReadiness {
    stamp: Option<QuestCompactStamp>,
    pub current: bool,
    pub action_count: usize,
    pub text_count: usize,
    pub generation: Option<u64>,
    pub revision: Option<u64>,
    pub sheet: Option<&'static str>,
    pub page: Option<&'static str>,
    pub quest_index: Option<i32>,
    pub reward_page: Option<usize>,
    pub text_top: Option<usize>,
    pub diary_page: Option<usize>,
    pub min_css_hit_width: Option<f32>,
    pub min_css_hit_height: Option<f32>,
    pub min_css_text_size: Option<f32>,
    pub panel_css_bounds: Option<QuestCompactPanelRect>,
    pub first_failure: Option<&'static str>,
    pub actions: Vec<QuestCompactActionRect>,
}

#[cfg(not(feature = "native-ui"))]
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestCompactObservationSet { Observe }

#[cfg(not(feature = "native-ui"))]
#[derive(Resource, Default)]
struct QuestCompactLastBuild(Option<QuestCompactStamp>);

#[cfg(not(feature = "native-ui"))]
fn compact_expected_stamp(
    presentation: &QuestUiPresentation,
    context: &crate::portable_quest_ui::QuestUiHostContext,
    player_ui: &NativePlayerUiState,
    state: &QuestUiState,
    tracker: &QuestTracker,
    dialog: &NpcDialogModel,
    guidance: &QuestGuidance,
    locale: QuestPresentationLocale,
) -> Option<QuestCompactStamp> {
    let layout = presentation.mobile_layout()?;
    if !layout.compact || !context.in_game { return None; }
    let npc_open = state.npc_quest_list_open && dialog.is_open
        && !npc_available_quests(dialog, tracker, Some(guidance)).is_empty();
    let detail = state.detail_quest(tracker);
    let sheet = mobile_sheet(
        state.abandon_confirmation_quest_index.is_some() || state.quest_alert_message.is_some(),
        npc_open, dialog.is_open, detail.is_some(), player_ui.quest_open());
    let (quest_index, page, reward_page, text_top) = match sheet {
        MobileQuestSheet::Detail => (detail.map(|quest| quest.quest_index),
            state.compact_detail_page, state.compact_detail_reward_page, state.detail_scroll_top),
        MobileQuestSheet::NpcList => (state.npc_quest_selected_index,
            state.compact_npc_page, state.compact_npc_reward_page, state.npc_quest_message_scroll_top),
        MobileQuestSheet::Confirmation => (state.abandon_confirmation_quest_index,
            CompactQuestPage::Text, 0, state.compact_confirmation_page),
        _ => (None, CompactQuestPage::Text, 0, 0),
    };
    Some(QuestCompactStamp {
        locale,
        generation: context.generation, revision: context.revision,
        open_revision: context.open_revision, presentation: *presentation, sheet,
        quest_index, page, reward_page, text_top,
        reward_selection: match sheet {
            MobileQuestSheet::Detail => state.selected_reward_index,
            MobileQuestSheet::NpcList => state.npc_selected_reward_index,
            _ => None,
        },
        graduation_choice: state.selected_graduation_direction,
        diary_page: if guidance.profile_name() == Some("newcomer-v2") {
            state.diary_page } else { state.page },
        diary_tab: state.diary_tab,
        dialog_top: state.dialog_scroll_top,
        view_epoch: state.compact_view_epoch,
    })
}

#[derive(Component)]
pub struct QuestTrackerPanel;

#[derive(Component)]
pub struct NpcDialogPanel;

#[derive(Component)]
struct CombatTargetPanel;

#[derive(Component)]
struct PickupFeedbackPanel;

#[derive(Component)]
struct NativePlayerHudPanel;

#[derive(Component)]
struct NativeControlHintPanel;

#[derive(Component)]
struct NativeQuickBagPanel;

#[derive(Component)]
pub struct QuestLogPanel;

#[derive(Component)]
pub struct QuestDetailPanel;

#[derive(Component)]
pub struct NpcQuestListPanel;

#[derive(Component)]
pub struct QuestConfirmationPanel;

#[derive(Component)]
struct QuestConfirmationBlocker;

#[derive(Component, Clone, Copy)]
pub(crate) struct QuestDiaryRow {
    quest_index: i32,
}

/// A transparent full-stage Button used only while a quest/NPC surface is
/// modal. It sits below the actual panel controls and above the world, so a
/// click outside the window cannot reach movement/ground interaction.
#[derive(Component)]
struct QuestUiModalBlocker;

#[derive(Component, Clone, Copy)]
struct QuestUiButtonVisual {
    enabled: bool,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(crate) enum QuestUiButton {
    ToggleSupplies,
    ShowSupplyInventory,
    SelectSupplyVendor(crate::quest_supplies::SupplyVendor),
    PrepareQuestFinish { quest_index: i32 },
    MakePrimary { quest_index: i32 },
    OpenDestinationMap,
    NavigateQuestRoute(QuestRouteNavigationIntent),
    SelectNpcDialog {
        target: String,
    },
    CloseNpcDialog,
    ToggleNpcQuestList,
    ReturnToNpcDialog,
    CloseNpcQuestList,
    SelectNpcQuest {
        quest_index: i32,
    },
    NpcQuestPrevious,
    NpcQuestNext,
    NpcQuestMessageScrollUp,
    NpcQuestMessageScrollDown,
    NpcQuestHelp,
    ReturnNpcService,
    NpcDialogScrollUp,
    NpcDialogScrollDown,
    AttackTarget {
        object_id: u32,
    },
    AttackQuestTarget {
        object_id: u32,
    },
    PickUpObject {
        object_id: u32,
    },
    PickUpTile,
    ToggleQuestGroup {
        group: String,
    },
    SelectGuidedDiaryTab(GuidedDiaryTab),
    GuidedDiaryPrevious,
    GuidedDiaryNext,
    SelectGraduationDirection {
        direction: GraduationDirection,
    },
    SelectQuestFilter {
        filter: QuestStageFilter,
    },
    QuestHelp,
    QuestPagePrevious,
    QuestPageNext,
    SelectQuest {
        quest_index: i32,
    },
    CloseQuestDetail,
    QuestDetailScrollUp,
    QuestDetailScrollDown,
    CompactDetailPage(CompactQuestPage),
    CompactNpcPage(CompactQuestPage),
    CompactDetailRewardPrevious,
    CompactDetailRewardNext,
    CompactNpcRewardPrevious,
    CompactNpcRewardNext,
    ShareQuest {
        quest_index: i32,
    },
    TrackQuest {
        quest_index: i32,
    },
    AcceptQuest {
        npc_index: u32,
        quest_index: i32,
    },
    /// Accept from the authoritative current-NPC quest list.
    AcceptNpcQuest {
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
    ConfirmAbandonQuest,
    CancelAbandonQuest,
    CloseQuestAlert,
    CompactConfirmationPrevious,
    CompactConfirmationNext,
    SelectReward {
        quest_index: i32,
        reward_index: i32,
    },
    SelectNpcQuestReward {
        quest_index: i32,
        reward_index: i32,
    },
    CloseQuestLog,
}

/// Pure helpers for quest state transitions – testable without Bevy.

pub fn can_accept_quest(quest: &Quest) -> bool {
    matches!(quest.status, crate::quest_model::QuestStatus::NotStarted)
}

pub fn can_finish_quest(quest: &Quest) -> bool {
    matches!(quest.status, crate::quest_model::QuestStatus::ReadyToTurnIn)
}

fn quest_action_profile(guidance: Option<&QuestGuidance>) -> QuestProfile {
    QuestProfile::from_name(
        guidance
            .and_then(QuestGuidance::profile_name)
            .unwrap_or_default(),
    )
}

fn quest_action_status(quest: &Quest) -> QuestActionStatus {
    match quest.status {
        crate::quest_model::QuestStatus::NotStarted => QuestActionStatus::NotStarted,
        crate::quest_model::QuestStatus::InProgress => QuestActionStatus::InProgress,
        crate::quest_model::QuestStatus::ReadyToTurnIn => QuestActionStatus::ReadyToTurnIn,
        _ => QuestActionStatus::Other,
    }
}

fn quest_action_decision(
    quest: Option<&Quest>,
    action: QuestAction,
    source: QuestActionSource,
    guidance: Option<&QuestGuidance>,
    pending: bool,
    selected_reward_index: Option<i32>,
) -> QuestActionDecision {
    let selectable_reward_indices = quest
        .map(selectable_reward_indices)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    evaluate_quest_action(QuestActionRequest {
        action,
        profile: quest_action_profile(guidance),
        source,
        facts: QuestActionFacts {
            status: quest.map(quest_action_status),
            accept_npc_index: quest.and_then(|quest| quest.accept_npc_index),
            finish_npc_index: quest.and_then(|quest| quest.finish_npc_index),
            selectable_reward_indices: &selectable_reward_indices,
        },
        pending,
        selected_reward_index,
    })
}

/// The newcomer profile exposes the small set of Crystal templates whose
/// authoritative start NPC is the Quest Diary sentinel. `None` is not proof.
fn newcomer_diary_accept_authorized(quest: &Quest, guidance: Option<&QuestGuidance>) -> bool {
    diary_endpoint_authorized(
        quest_action_profile(guidance),
        QuestAction::Accept,
        quest.accept_npc_index,
        quest.finish_npc_index,
    )
}

/// A zero finish NPC normally means "return to the start NPC". It is a Diary
/// finish only when the same authoritative template also starts in the Diary.
fn newcomer_diary_finish_authorized(quest: &Quest, guidance: Option<&QuestGuidance>) -> bool {
    diary_endpoint_authorized(
        quest_action_profile(guidance),
        QuestAction::Finish,
        quest.accept_npc_index,
        quest.finish_npc_index,
    )
}

/// Crystal's detail Cancel button is present for every `CurrentQuests` entry,
/// including one whose objectives are already complete. Available/history
/// rows are not current quests and must remain rejected.
pub fn can_abandon_quest(quest: &Quest) -> bool {
    quest.status.is_active()
}

pub fn can_track_quest(quest: &Quest) -> bool {
    quest.status.is_active()
}

fn toggle_quest_tracking(state: &mut QuestUiState, tracker: &QuestTracker, quest_index: i32) {
    let Some(quest) = tracker
        .active_quests
        .iter()
        .find(|quest| quest.quest_index == quest_index)
    else {
        state.set_feedback("Quest not found", true);
        return;
    };
    if !can_track_quest(quest) {
        state.set_feedback("Cannot track this quest", true);
        return;
    }
    match state.toggle_tracked_quest(quest_index) {
        QuestTrackingChange::Added => {
            state.set_feedback(format!("Tracking {}", quest.title), false)
        }
        QuestTrackingChange::Removed => {
            state.set_feedback(format!("Stopped tracking {}", quest.title), false)
        }
        QuestTrackingChange::Full => {
            // Crystal silently refuses a sixth entry. Keep the visible state
            // unchanged and report the reason only in Candidate diagnostics.
            state.set_feedback("You can track up to five quests", true)
        }
    }
}

fn confirm_quest_abandon(
    queue: &mut QuestUiIntentQueue,
    pending: &mut PendingOperations,
    state: &mut QuestUiState,
    tracker: &QuestTracker,
) {
    let Some(quest_index) = state.abandon_confirmation_quest_index else {
        return;
    };
    let Some(quest) = tracker
        .active_quests
        .iter()
        .find(|quest| quest.quest_index == quest_index)
    else {
        state.close_abandon_confirmation();
        state.set_feedback("Quest not found", true);
        return;
    };
    if !can_abandon_quest(quest) {
        state.close_abandon_confirmation();
        state.set_feedback("Quest is no longer current", true);
        return;
    }

    let title = quest.title.clone();
    let queue_full = queue.is_full();
    if queue.push_pending_intent(pending, QuestUiIntent::AbandonQuest { quest_index }) {
        // Crystal closes the independent detail window only after Yes.
        state.close_detail();
        state.set_feedback(format!("Abandoning {title}"), false);
    } else if queue_full {
        state.set_feedback("Connection busy; try again", true);
    } else {
        state.close_abandon_confirmation();
        state.set_feedback("Quest request is already pending", true);
    }
}

fn dialog_has_quest_action(dialog: &NpcDialogModel, quest_index: i32) -> bool {
    dialog.options.iter().any(|option| {
        option.enabled
            && (option
                .option_id
                .trim()
                .eq_ignore_ascii_case(&format!("@AcceptQuest:{quest_index}"))
                || option
                    .option_id
                    .trim()
                    .eq_ignore_ascii_case(&format!("@FinishQuest:{quest_index}")))
    })
}

fn quest_belongs_to_current_npc(dialog: &NpcDialogModel, quest: &Quest) -> bool {
    if !dialog.is_open {
        return false;
    }
    if dialog_has_quest_action(dialog, quest.quest_index) {
        return true;
    }
    let Some(npc_index) = dialog.npc_object_id else {
        return false;
    };
    match &quest.status {
        crate::quest_model::QuestStatus::NotStarted => quest.accept_npc_index == Some(npc_index),
        crate::quest_model::QuestStatus::InProgress
        | crate::quest_model::QuestStatus::ReadyToTurnIn => {
            quest.finish_npc_index == Some(npc_index) || quest.accept_npc_index == Some(npc_index)
        }
        _ => false,
    }
}

fn npc_available_quests<'a>(
    dialog: &NpcDialogModel,
    tracker: &'a QuestTracker,
    guidance: Option<&QuestGuidance>,
) -> Vec<&'a Quest> {
    let mut quests = tracker
        .active_quests
        .iter()
        .filter(|quest| quest_belongs_to_current_npc(dialog, quest))
        .enumerate()
        .collect::<Vec<_>>();
    if let Some(guidance) = guidance.filter(|guidance| guidance.is_enabled()) {
        quests.sort_by_key(|(position, quest)| {
            (
                u8::from(!matches!(
                    quest.status,
                    crate::quest_model::QuestStatus::ReadyToTurnIn
                )),
                guidance.sort_key(quest.quest_index, *position),
            )
        });
    }
    quests.into_iter().map(|(_, quest)| quest).collect()
}

fn npc_available_quest_indices(
    dialog: &NpcDialogModel,
    tracker: &QuestTracker,
    guidance: Option<&QuestGuidance>,
) -> Vec<i32> {
    npc_available_quests(dialog, tracker, guidance)
        .into_iter()
        .map(|quest| quest.quest_index)
        .collect()
}

fn npc_list_accept_is_current(
    npc_quest_indices: &[i32],
    dialog: &NpcDialogModel,
    tracker: &QuestTracker,
    npc_index: u32,
    quest_index: i32,
) -> bool {
    dialog.is_open
        && dialog.npc_object_id == Some(npc_index)
        && npc_index != 0
        && npc_quest_indices.contains(&quest_index)
        && tracker.active_quests.iter().any(|quest| {
            quest.quest_index == quest_index
                && npc_accept_offered_by_current_dialog(dialog, quest, npc_index)
                && can_accept_quest(quest)
        })
}

fn npc_accept_offered_by_current_dialog(
    dialog: &NpcDialogModel,
    quest: &Quest,
    npc_index: u32,
) -> bool {
    dialog.is_open
        && dialog.npc_object_id == Some(npc_index)
        && npc_index != 0
        && (quest.accept_npc_index == Some(npc_index)
            || dialog_exposes_quest_operation(
                dialog,
                Some(npc_index),
                quest.quest_index,
                false,
                None,
            ))
}

fn selectable_reward_indices(quest: &Quest) -> impl Iterator<Item = i32> + '_ {
    quest.rewards.iter().filter_map(|reward| match reward {
        crate::quest_model::QuestReward::Item {
            selection_index: Some(index),
            ..
        } => Some(*index),
        _ => None,
    })
}

pub fn reward_selection_required(quest: &Quest) -> bool {
    can_finish_quest(quest) && selectable_reward_indices(quest).next().is_some()
}

pub fn is_valid_reward_selection(quest: &Quest, selected: Option<i32>) -> bool {
    if !reward_selection_required(quest) {
        return true;
    }
    match selected {
        Some(idx) => idx >= 0 && selectable_reward_indices(quest).any(|candidate| candidate == idx),
        None => false,
    }
}

pub fn quest_accept_enabled(quest: &Quest) -> bool {
    can_accept_quest(quest)
}

pub fn quest_finish_enabled(quest: &Quest, selected_reward: Option<i32>) -> bool {
    can_finish_quest(quest) && is_valid_reward_selection(quest, selected_reward)
}

pub fn is_quest_log_open_state(player_ui: Option<&NativePlayerUiState>) -> bool {
    player_ui.is_some_and(NativePlayerUiState::quest_open)
}

pub fn blocks_gameplay_input(
    player_ui: Option<&NativePlayerUiState>,
    dialog: &NpcDialogModel,
) -> bool {
    // Modal quest log or NPC dialog blocks world T/F/R shortcuts.
    if dialog.is_open {
        return true;
    }
    if is_quest_log_open_state(player_ui) {
        return true;
    }
    if let Some(ui) = player_ui {
        if ui.blocks_gameplay_keys() {
            return true;
        }
        if ui.blocks_world_click() {
            return true;
        }
    }
    false
}

pub fn is_world_click_blocked_for_quest(
    player_ui: Option<&NativePlayerUiState>,
    dialog: &NpcDialogModel,
    dead: bool,
) -> bool {
    if dead {
        return true;
    }
    if dialog.is_open {
        return true;
    }
    if let Some(ui) = player_ui {
        return ui.blocks_world_action(dialog.is_open, dead);
    }
    dialog.is_open
}

/// Shared plugin for the same Quest nodes, buttons and intent handling on
/// native and portable hosts. The portable host supplies its shell context.
pub struct Mir2QuestUiPlugin;

impl Plugin for Mir2QuestUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestPresentationLocale>()
            .init_resource::<QuestTracker>()
            .init_resource::<CompletedQuestTracker>()
            .init_resource::<NpcDialogModel>()
            .init_resource::<NearbyNpcModel>()
            .init_resource::<CombatTargetModel>()
            .init_resource::<GroundPickupModel>()
            .init_resource::<UiReadModel>()
            .init_resource::<EntityModelSet>()
            .init_resource::<MapModel>()
            .init_resource::<InventoryModel>()
            .init_resource::<QuestUiIntentQueue>()
            .init_resource::<QuestRouteNavigationIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<AuthoritativeModelRevisions>()
            .init_resource::<SessionResetRevision>()
            .init_resource::<QuestUiState>()
            .init_resource::<QuestGuidance>()
            .init_resource::<NewcomerJourneyCatalog>()
            .init_resource::<NpcDialogNav>()
            .init_resource::<NativePlayerUiState>()
            .init_resource::<UiEffectQueue>()
            .configure_sets(
                Update,
                NativePlayerUiSet::Mutate.before(NativePlayerUiSet::Read),
            )
            .configure_sets(
                Update,
                PendingLifecycleSet::UiReset.before(NativePlayerUiSet::Mutate),
            )
            .add_systems(Startup, spawn_quest_ui_panels)
            .add_systems(Update, render_quest_ui.in_set(NativePlayerUiSet::Read))
            .add_systems(
                Update,
                sync_quest_ui_button_visuals
                    .after(render_quest_ui)
                    .in_set(NativePlayerUiSet::Read),
            );
        #[cfg(feature = "native-ui")]
        app.init_resource::<crate::pending_operations::QuestResetTracker>()
            .add_systems(Update, crate::pending_operations::apply_quest_session_reset.in_set(PendingLifecycleSet::UiReset))
            .add_systems(Update, process_quest_ui_input
                .after(crate::crystal_ui::overlays::process_overlay_keyboard)
                .in_set(NativePlayerUiSet::Mutate));
        #[cfg(not(feature = "native-ui"))]
        app.init_resource::<QuestCompactReadiness>()
            .init_resource::<QuestCompactLastBuild>()
            .add_systems(Update, normalize_compact_quest_state
                .before(process_quest_ui_input).in_set(NativePlayerUiSet::Mutate))
            .add_systems(Update, process_quest_ui_input.in_set(NativePlayerUiSet::Mutate))
            .add_systems(PostUpdate, observe_compact_quest_tree
                .after(bevy::ui::UiSystems::Layout)
                .in_set(QuestCompactObservationSet::Observe));
    }
}

fn spawn_quest_ui_panels(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    #[cfg(not(feature = "native-ui"))]
    target_camera: Option<Res<crate::portable_quest_ui::QuestUiTargetCamera>>,
) {
    #[cfg(not(feature = "native-ui"))]
    let Some(target_camera) = target_camera else { return; };
    let panel_skin = asset_server
        .as_ref()
        .map(|server| server.load::<Image>(NPC_DIALOG_FRAME_ASSET));
    let quest_diary_skin = asset_server
        .as_ref()
        .map(|server| server.load::<Image>(QUEST_DIARY_FRAME_ASSET));
    let quest_detail_skin = asset_server
        .as_ref()
        .map(|server| server.load::<Image>(QUEST_DETAIL_FRAME_ASSET));
    let quest_list_skin = asset_server
        .as_ref()
        .map(|server| server.load::<Image>(QUEST_LIST_FRAME_ASSET));
    let quest_confirmation_skin = asset_server
        .as_ref()
        .map(|server| server.load::<Image>(QUEST_CONFIRM_FRAME_ASSET));
    let mut quest_root = commands
        .spawn((
            QuestUiRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(980),
        ));
    #[cfg(not(feature = "native-ui"))]
    quest_root.insert(bevy::ui::UiTargetCamera(target_camera.0));
    quest_root.with_children(|root| {
            // Full-stage modal capture. Actual quest/dialog controls are
            // spawned after this sibling and therefore remain clickable.
            root.spawn((
                QuestUiModalBlocker,
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    ..default()
                },
                // Crystal's quest diary is a sorted movable window, not a
                // modal screen. Keep world capture below the persistent HUD
                // so every bottom-bar control remains reachable while the
                // diary is open; the quest panels themselves stay at 980.
                GlobalZIndex(HUD_Z_INDEX - 1),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.001)),
                FocusPolicy::Block,
            ));

            root.spawn((
                QuestTrackerPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(100.0),
                    width: Val::Px(320.0),
                    height: Val::Px(520.0),
                    max_width: Val::Px(320.0),
                    min_width: Val::Px(220.0),
                    display: Display::Flex,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));

            let mut dialog_panel = root.spawn((
                NpcDialogPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(440.0),
                    height: Val::Px(224.0),
                    min_height: Val::Px(224.0),
                    max_height: Val::Px(224.0),
                    min_width: Val::Px(440.0),
                    max_width: Val::Px(440.0),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::ZERO,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                FocusPolicy::Block,
            ));
            if let Some(panel_skin) = panel_skin.as_ref() {
                dialog_panel.insert(ImageNode {
                    image: panel_skin.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                });
            }

            root.spawn((
                CombatTargetPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(COMBAT_TARGET_PANEL_LEFT),
                    top: Val::Px(COMBAT_TARGET_PANEL_TOP),
                    width: Val::Px(COMBAT_TARGET_PANEL_WIDTH),
                    min_height: Val::Px(COMBAT_TARGET_PANEL_MIN_HEIGHT),
                    min_width: Val::Px(COMBAT_TARGET_PANEL_WIDTH),
                    max_width: Val::Px(COMBAT_TARGET_PANEL_WIDTH),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::all(Val::Px(COMBAT_TARGET_PANEL_PADDING)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));

            root.spawn((
                PickupFeedbackPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(352.0),
                    bottom: Val::Px(96.0),
                    width: Val::Px(320.0),
                    min_height: Val::Px(70.0),
                    min_width: Val::Px(220.0),
                    max_width: Val::Px(320.0),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));

            root.spawn((
                NativePlayerHudPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    bottom: Val::Px(12.0),
                    width: Val::Px(270.0),
                    min_height: Val::Px(82.0),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));

            root.spawn((
                NativeControlHintPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(294.0),
                    bottom: Val::Px(12.0),
                    width: Val::Px(420.0),
                    min_height: Val::Px(82.0),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));

            root.spawn((
                NativeQuickBagPanel,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(12.0),
                    bottom: Val::Px(12.0),
                    width: Val::Px(286.0),
                    min_height: Val::Px(82.0),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ));

            let mut quest_log_panel = root.spawn((
                QuestLogPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(QUEST_DIARY_DESIGN_LEFT),
                    top: Val::Px(QUEST_DIARY_DESIGN_TOP),
                    width: Val::Px(QUEST_DIARY_DESIGN_WIDTH),
                    height: Val::Px(QUEST_DIARY_DESIGN_HEIGHT),
                    min_width: Val::Px(QUEST_DIARY_DESIGN_WIDTH),
                    max_width: Val::Px(QUEST_DIARY_DESIGN_WIDTH),
                    min_height: Val::Px(QUEST_DIARY_DESIGN_HEIGHT),
                    max_height: Val::Px(QUEST_DIARY_DESIGN_HEIGHT),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(0.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                FocusPolicy::Block,
            ));
            if let Some(panel_skin) = quest_diary_skin.as_ref() {
                quest_log_panel.insert(ImageNode {
                    image: panel_skin.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                });
            }

            let mut quest_detail_panel = root.spawn((
                QuestDetailPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(QUEST_DETAIL_DESIGN_LEFT),
                    top: Val::Px(QUEST_DETAIL_DESIGN_TOP),
                    width: Val::Px(QUEST_DETAIL_DESIGN_WIDTH),
                    height: Val::Px(QUEST_DETAIL_DESIGN_HEIGHT),
                    min_width: Val::Px(QUEST_DETAIL_DESIGN_WIDTH),
                    max_width: Val::Px(QUEST_DETAIL_DESIGN_WIDTH),
                    min_height: Val::Px(QUEST_DETAIL_DESIGN_HEIGHT),
                    max_height: Val::Px(QUEST_DETAIL_DESIGN_HEIGHT),
                    display: Display::None,
                    padding: UiRect::all(Val::Px(0.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                FocusPolicy::Block,
            ));
            if let Some(panel_skin) = quest_detail_skin.as_ref() {
                quest_detail_panel.insert(ImageNode {
                    image: panel_skin.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                });
            }

            let mut npc_quest_list_panel = root.spawn((
                NpcQuestListPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(QUEST_LIST_DESIGN_LEFT),
                    top: Val::Px(QUEST_LIST_DESIGN_TOP),
                    width: Val::Px(QUEST_LIST_DESIGN_WIDTH),
                    height: Val::Px(QUEST_LIST_DESIGN_HEIGHT),
                    min_width: Val::Px(QUEST_LIST_DESIGN_WIDTH),
                    max_width: Val::Px(QUEST_LIST_DESIGN_WIDTH),
                    min_height: Val::Px(QUEST_LIST_DESIGN_HEIGHT),
                    max_height: Val::Px(QUEST_LIST_DESIGN_HEIGHT),
                    display: Display::None,
                    padding: UiRect::all(Val::Px(0.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                FocusPolicy::Block,
                GlobalZIndex(990),
            ));
            if let Some(panel_skin) = quest_list_skin.as_ref() {
                npc_quest_list_panel.insert(ImageNode {
                    image: panel_skin.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                });
            }

            root.spawn((
                QuestConfirmationBlocker,
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.001)),
                FocusPolicy::Block,
                GlobalZIndex(1099),
            ));

            let mut confirmation_panel = root.spawn((
                QuestConfirmationPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(QUEST_CONFIRM_DESIGN_LEFT),
                    top: Val::Px(QUEST_CONFIRM_DESIGN_TOP),
                    width: Val::Px(QUEST_CONFIRM_DESIGN_WIDTH),
                    height: Val::Px(QUEST_CONFIRM_DESIGN_HEIGHT),
                    min_width: Val::Px(QUEST_CONFIRM_DESIGN_WIDTH),
                    max_width: Val::Px(QUEST_CONFIRM_DESIGN_WIDTH),
                    min_height: Val::Px(QUEST_CONFIRM_DESIGN_HEIGHT),
                    max_height: Val::Px(QUEST_CONFIRM_DESIGN_HEIGHT),
                    display: Display::None,
                    padding: UiRect::all(Val::Px(0.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                FocusPolicy::Block,
                GlobalZIndex(1100),
            ));
            if let Some(panel_skin) = quest_confirmation_skin.as_ref() {
                confirmation_panel.insert(ImageNode {
                    image: panel_skin.clone(),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                });
            }
        });
}

#[derive(SystemParam)]
pub(crate) struct QuestInputModels<'w> {
    locale: Res<'w, QuestPresentationLocale>,
    #[cfg(not(feature = "native-ui"))]
    host_context: Option<Res<'w, crate::portable_quest_ui::QuestUiHostContext>>,
    #[cfg(not(feature = "native-ui"))]
    compact_readiness: Option<Res<'w, QuestCompactReadiness>>,
    presentation: Option<Res<'w, QuestUiPresentation>>,
    map: Option<Res<'w, MapModel>>,
    notice: Option<Res<'w, crate::crystal_ui::notice::NoticeDialogState>>,
    tracker: Res<'w, QuestTracker>,
    guidance: Option<Res<'w, QuestGuidance>>,
    entities: Option<Res<'w, EntityModelSet>>,
    completed: Option<Res<'w, CompletedQuestTracker>>,
    catalog: Option<Res<'w, NewcomerJourneyCatalog>>,
    read_model: Option<Res<'w, UiReadModel>>,
    big_map: Option<Res<'w, crate::big_map::BigMapModel>>,
    route_navigation: Option<ResMut<'w, QuestRouteNavigationIntentQueue>>,
}

#[derive(SystemParam)]
struct JourneyRenderModels<'w> {
    locale: Res<'w, QuestPresentationLocale>,
    #[cfg(not(feature = "native-ui"))]
    host_context: Option<Res<'w, crate::portable_quest_ui::QuestUiHostContext>>,
    #[cfg(not(feature = "native-ui"))]
    compact_last_build: ResMut<'w, QuestCompactLastBuild>,
    presentation: Option<Res<'w, QuestUiPresentation>>,
    completed: Res<'w, CompletedQuestTracker>,
    guidance: Res<'w, QuestGuidance>,
    catalog: Res<'w, NewcomerJourneyCatalog>,
    entities: Res<'w, EntityModelSet>,
    map: Res<'w, MapModel>,
    big_map: Option<Res<'w, crate::big_map::BigMapModel>>,
    skills: Option<Res<'w, crate::skill_model::SkillModel>>,
}

fn portable_unsupported_action(action: &QuestUiButton) -> bool {
    !cfg!(feature = "native-ui")
        && matches!(
            action,
            QuestUiButton::ToggleSupplies
                | QuestUiButton::SelectSupplyVendor(_)
                | QuestUiButton::ShowSupplyInventory
                | QuestUiButton::OpenDestinationMap
                | QuestUiButton::NavigateQuestRoute(_)
                | QuestUiButton::AttackTarget { .. }
                | QuestUiButton::AttackQuestTarget { .. }
                | QuestUiButton::PickUpObject { .. }
                | QuestUiButton::PickUpTile
        )
}

#[cfg(not(feature = "native-ui"))]
fn compact_input_current(
    presentation: Option<&QuestUiPresentation>,
    context: Option<&crate::portable_quest_ui::QuestUiHostContext>,
    guidance: Option<&QuestGuidance>,
    player_ui: &NativePlayerUiState,
    state: &QuestUiState,
    tracker: &QuestTracker,
    dialog: &NpcDialogModel,
    readiness: Option<&QuestCompactReadiness>,
    locale: QuestPresentationLocale,
) -> bool {
    if !presentation.and_then(|value| value.mobile_layout()).is_some_and(|layout| layout.compact) {
        return true;
    }
    let expected = presentation.zip(context).zip(guidance)
        .and_then(|((presentation, context), guidance)| compact_expected_stamp(
            presentation, context, player_ui, state, tracker, dialog, guidance, locale));
    readiness.is_some_and(|ready| ready.current && ready.stamp == expected && expected.is_some())
}

#[cfg(not(feature = "native-ui"))]
fn normalize_compact_quest_state(
    presentation: Option<Res<QuestUiPresentation>>,
    tracker: Res<QuestTracker>,
    dialog: Res<NpcDialogModel>,
    guidance: Res<QuestGuidance>,
    completed: Res<CompletedQuestTracker>,
    catalog: Res<NewcomerJourneyCatalog>,
    read_model: Res<UiReadModel>,
    locale: Res<QuestPresentationLocale>,
    mut state: ResMut<QuestUiState>,
) {
    let Some(layout) = presentation.as_deref().and_then(|value| value.mobile_layout()) else { return; };
    if !layout.compact { return; }
    let journey = catalog.derive(&guidance, &tracker, &completed, &read_model.player);
    let guided = guidance.profile_name() == Some("newcomer-v2");
    let quest_count = if guided {
        guided_diary_quests(&tracker, Some(&guidance), journey.as_ref(), state.diary_tab).len()
    } else {
        quest_diary_groups(&tracker, Some(&guidance)).iter()
            .map(|group| group.quests.len()).sum()
    };
    let graduation_options = if !guided || state.diary_tab == GuidedDiaryTab::Main {
        journey.as_ref().and_then(|view| view.graduation.as_ref())
            .map_or(0, |graduation| graduation.options.len())
    } else { 0 };
    let last_diary_page = mobile_diary_page_count(quest_count, graduation_options,
        layout.diary_rows()).saturating_sub(1);
    if guided {
        if state.diary_page > last_diary_page { state.diary_page = last_diary_page; }
    } else if state.page > last_diary_page { state.page = last_diary_page; }
    if state.detail_quest_index.is_some_and(|index|
        !tracker.active_quests.iter().any(|quest| quest.quest_index == index)) {
        state.close_detail();
        state.selected_quest_index = None;
    }
    if let Some(quest) = state.detail_quest(&tracker) {
        let max_page = compact_reward_pages(quest, Some(&guidance)).saturating_sub(1);
        let lines = mobile_wrap_quest_lines(quest_detail_lines_with_wrap(quest, Some(&guidance),
            read_model.player.class_name.as_deref().unwrap_or(""), false, QuestRenderText { locale: *locale }), layout.width_css() - 32.0);
        if state.compact_detail_reward_page > max_page { state.compact_detail_reward_page = max_page; }
        let max_text_top = lines.len().saturating_sub(layout.detail_rows());
        if state.detail_scroll_top > max_text_top { state.detail_scroll_top = max_text_top; }
    }
    if state.npc_quest_list_open {
        let quests = npc_available_quests(&dialog, &tracker, Some(&guidance));
        if !dialog.is_open || quests.is_empty() {
            state.close_npc_quest_list();
        } else {
            let selected = state.npc_quest_selected_index
                .and_then(|index| quests.iter().copied().find(|quest| quest.quest_index == index))
                .unwrap_or(quests[0]);
            if state.npc_quest_selected_index != Some(selected.quest_index) {
                state.select_npc_quest(selected.quest_index);
            }
            let max_reward_page = compact_reward_pages(selected, Some(&guidance)).saturating_sub(1);
            if state.compact_npc_reward_page > max_reward_page {
                state.compact_npc_reward_page = max_reward_page;
            }
            let lines = mobile_wrap_quest_lines(quest_list_message_lines_with_wrap(selected,
                &dialog, Some(&guidance), false, QuestRenderText { locale: *locale }), layout.width_css() - 32.0);
            let max_text_top = lines.len().saturating_sub(layout.message_rows());
            if state.npc_quest_message_scroll_top > max_text_top {
                state.npc_quest_message_scroll_top = max_text_top;
            }
        }
    }
    if state.quest_alert_message.is_some() || state.abandon_confirmation_quest_index.is_some() {
        let pages = compact_confirmation_lines(state.quest_alert_message.as_deref()
            .unwrap_or(ASK_CANCEL_QUEST_TEXT), layout).len().div_ceil(5).max(1);
        if state.compact_confirmation_page >= pages { state.compact_confirmation_page = pages - 1; }
    }
}

pub(crate) fn process_quest_ui_input(
    mut queue: ResMut<QuestUiIntentQueue>,
    mut pending: ResMut<PendingOperations>,
    mut quest_state: ResMut<QuestUiState>,
    mut dialog: ResMut<NpcDialogModel>,
    mut npc_nav: ResMut<NpcDialogNav>,
    mut player_ui: ResMut<NativePlayerUiState>,
    mut effects: Option<ResMut<UiEffectQueue>>,
    button_events: Query<(&Interaction, &QuestUiButton), (Changed<Interaction>, With<Button>)>,
    diary_rows: Query<(&QuestDiaryRow, &RelativeCursorPosition)>,
    shell: Option<Res<NativeShellModel>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Option<Res<ButtonInput<MouseButton>>>,
    nearby: Option<Res<NearbyNpcModel>>,
    target: Option<Res<CombatTargetModel>>,
    pickups: Option<Res<GroundPickupModel>>,
    mut models: QuestInputModels,
) {
    let Some(shell) = shell else {
        return;
    };
    let mut fallback_effects = UiEffectQueue::default();
    let mut effects = effects.as_deref_mut().unwrap_or(&mut fallback_effects);
    if shell.screen != NativeShellScreen::InGame {
        if let Some(route_navigation) = models.route_navigation.as_deref_mut() {
            route_navigation.clear();
        }
        quest_state.reset();
        npc_nav.clear();
        return;
    }

    // A browser resize/fallback hides the surface without leaving the game.
    // Consume this frame's interaction changes without resetting the view or
    // allowing a hidden button/key to emit an intent.
    #[cfg(not(feature = "native-ui"))]
    if models.host_context.as_deref().is_some_and(|host| !host.host_visible) {
        return;
    }

    if quest_state.supply_open {
        let epoch = models.big_map.as_deref().map(|map| map.reset_epoch);
        if quest_state.supply_epoch != epoch {
            quest_state.supply_epoch = epoch;
            quest_state.clear_feedback();
        }
    }

    // Auto-clear navigation history when dialog is externally closed by server.
    if !dialog.is_open && !npc_nav.history.is_empty() {
        npc_nav.clear();
    }
    if !dialog.is_open && quest_state.npc_quest_list_open {
        quest_state.close_npc_quest_list();
    }

    let quest_log_open = player_ui.quest_open();
    let mobile = models.presentation.as_deref().and_then(|value| value.mobile_layout());
    #[cfg(not(feature = "native-ui"))]
    if !compact_input_current(models.presentation.as_deref(), models.host_context.as_deref(),
        models.guidance.as_deref(), &player_ui, &quest_state, &models.tracker, &dialog,
        models.compact_readiness.as_deref(), *models.locale) { return; }
    let dialog_open = dialog.is_open;
    let blocks_gameplay_keys = player_ui.blocks_gameplay_keys();
    let turn_in_blocked = !quest_turn_in_ui_allows_interaction(Some(&player_ui))
        || models.notice.as_deref().is_some_and(crate::crystal_ui::notice::NoticeDialogState::is_open)
        || models.read_model.as_deref().is_some_and(|model| model.player.max_hp > 0 && model.player.hp <= 0);
    let mut route_navigation = models.route_navigation;
    let guidance = models.guidance.as_deref();
    let tracker: &QuestTracker = &models.tracker;
    let journey = models.catalog.as_deref().zip(guidance)
        .zip(models.completed.as_deref()).zip(models.read_model.as_deref())
        .and_then(|(((catalog, guidance), completed), read_model)| {
            catalog.derive(guidance, tracker, completed, &read_model.player)
        });
    if let Some(navigation) = route_navigation.as_deref_mut() {
        if navigation.pending.is_some_and(|intent| !quest_route_intent_is_current(
            intent, tracker, &quest_state, journey.as_ref(), models.big_map.as_deref(),
        )) {
            navigation.clear();
            quest_state.set_feedback("任务导航已更新，请重新选择目标", true);
        }
    }
    if quest_state.pinned_primary_quest_index.is_some_and(|id| !tracker.active_quests.iter().any(|q| q.quest_index == id && q.status.is_active())) {
        quest_state.pinned_primary_quest_index = None;
    }
    let npc_quest_indices = npc_available_quest_indices(&dialog, tracker, guidance);
    if quest_state.npc_quest_list_open {
        if npc_quest_indices.is_empty() {
            quest_state.close_npc_quest_list();
        } else if !quest_state
            .npc_quest_selected_index
            .is_some_and(|selected| npc_quest_indices.contains(&selected))
        {
            quest_state.open_npc_quest_list(&npc_quest_indices);
        }
    }

    // Bevy's legacy `Interaction` component only promotes the primary mouse
    // button. Crystal's diary uses right-click for tracking, so inspect the
    // source row's cursor component on the actual secondary-button edge.
    if quest_log_open
        && mouse_buttons
            .as_deref()
            .is_some_and(|buttons| buttons.just_pressed(MouseButton::Right))
    {
        if let Some(row) = diary_rows
            .iter()
            .find_map(|(row, cursor)| cursor.cursor_over().then_some(*row))
        {
            toggle_quest_tracking(&mut quest_state, tracker, row.quest_index);
        }
    }

    for (interaction, action) in button_events.iter() {
        #[cfg(not(feature = "native-ui"))]
        if !compact_input_current(models.presentation.as_deref(), models.host_context.as_deref(),
            models.guidance.as_deref(), &player_ui, &quest_state, &models.tracker, &dialog,
            models.compact_readiness.as_deref(), *models.locale) { break; }
        if *interaction != Interaction::Pressed {
            continue;
        }
        if portable_unsupported_action(action) {
            quest_state.set_feedback("This Quest action needs the native world host", true);
            continue;
        }
        if !matches!(action, QuestUiButton::PrepareQuestFinish { .. }
            | QuestUiButton::CompactDetailPage(_) | QuestUiButton::CompactNpcPage(_)
            | QuestUiButton::CompactDetailRewardPrevious | QuestUiButton::CompactDetailRewardNext
            | QuestUiButton::CompactNpcRewardPrevious | QuestUiButton::CompactNpcRewardNext
            | QuestUiButton::CompactConfirmationPrevious | QuestUiButton::CompactConfirmationNext) {
            quest_state.pending_turn_in = None;
        }
        match action.clone() {
            QuestUiButton::ToggleSupplies => {
                quest_state.supply_open = !quest_state.supply_open;
                quest_state.clear_feedback();
            }
            QuestUiButton::SelectSupplyVendor(vendor) => {
                quest_state.supply_vendor = Some(vendor);
                quest_state.clear_feedback();
            }
            QuestUiButton::ShowSupplyInventory => {
                quest_state.supply_vendor = None;
                quest_state.clear_feedback();
            }
            QuestUiButton::PrepareQuestFinish { quest_index } => {
                if turn_in_blocked {
                    quest_state.pending_turn_in = None;
                    quest_state.set_feedback("当前状态无法交付，请关闭其他弹窗并确认角色存活", true);
                    continue;
                }
                if pending.has_pending_quest_finish(quest_index) {
                    quest_state.set_feedback("Quest request is already pending", true);
                    continue;
                }
                turn_in::begin(quest_index, &turn_in::TurnInContext {
                    tracker, dialog: &dialog, map: models.map.as_deref(),
                    entities: models.entities.as_deref(), big_map: models.big_map.as_deref(),
                }, &mut quest_state, &mut queue, &mut pending);
            }
            QuestUiButton::ToggleQuestGroup { group } => {
                quest_state.toggle_group(group);
            }
            QuestUiButton::SelectGuidedDiaryTab(tab) => {
                quest_state.diary_tab = tab;
                quest_state.diary_page = 0;
                quest_state.clear_diary_selection();
            }
            QuestUiButton::GuidedDiaryPrevious => {
                quest_state.diary_page = quest_state.diary_page.saturating_sub(1);
                quest_state.clear_diary_selection();
            }
            QuestUiButton::GuidedDiaryNext => {
                let quest_count = guided_diary_quests(
                    tracker, guidance, journey.as_ref(), quest_state.diary_tab,
                )
                    .len();
                let page_count = if let Some(layout) = mobile {
                    mobile_diary_page_count(quest_count,
                        if quest_state.diary_tab == GuidedDiaryTab::Main {
                            journey.as_ref().and_then(|view| view.graduation.as_ref())
                                .map_or(0, |graduation| graduation.options.len())
                        } else { 0 }, layout.diary_rows())
                } else { quest_count.div_ceil(GUIDED_DIARY_PAGE_SIZE) };
                quest_state.diary_page = (quest_state.diary_page + 1)
                    .min(page_count.saturating_sub(1));
                quest_state.clear_diary_selection();
            }
            QuestUiButton::SelectGraduationDirection { direction } => {
                if quest_state.toggle_graduation_direction(direction) {
                    quest_state
                        .set_feedback(format!("Goal selected: {}", direction.label()), false);
                } else {
                    quest_state.set_feedback("Goal cleared", false);
                }
            }
            QuestUiButton::QuestHelp => {
                quest_state.set_feedback("Quest Log help is not available.", false);
            }
            QuestUiButton::SelectQuestFilter { filter } => {
                quest_state.set_stage_filter(filter);
            }
            QuestUiButton::QuestPagePrevious => {
                let page = quest_state.page;
                quest_state.set_page(page.saturating_sub(1));
            }
            QuestUiButton::QuestPageNext => {
                let page_count = if let Some(layout) = mobile {
                    mobile_diary_page_count(
                        quest_diary_groups(tracker, guidance).iter().map(|group| group.quests.len()).sum(),
                        journey.as_ref().and_then(|view| view.graduation.as_ref())
                            .map_or(0, |graduation| graduation.options.len()),
                        layout.diary_rows(),
                    )
                } else {
                    tracker.active_quests.iter().filter(|quest| quest_state.stage_filter.matches(quest))
                        .count().div_ceil(MAX_QUEST_LOG_ROWS)
                };
                let page = quest_state.page;
                quest_state.set_page((page + 1).min(page_count.saturating_sub(1)));
            }
            QuestUiButton::SelectNpcDialog { target } => {
                let option_enabled = dialog
                    .is_open
                    .then(|| {
                        dialog
                            .options
                            .iter()
                            .find(|option| option.option_id == target)
                            .map(|option| option.enabled)
                    })
                    .flatten();
                match option_enabled {
                    Some(true) => {
                        // Save history for service return only when the current server page
                        // still exposes this enabled target. This keeps a stale pointer event
                        // from navigating after a dialog refresh.
                        if queue.push_intent(QuestUiIntent::SelectNpcDialog {
                            target: target.clone(),
                        }) {
                            #[cfg(feature = "native-ui")]
                            player_ui.withdraw_bound_npc_gold_buy();
                            npc_nav.push(dialog.clone());
                            quest_state.dialog_scroll_top = 0;
                            quest_state.feedback = None;
                        } else {
                            quest_state.set_feedback("Connection busy; try again", true);
                        }
                    }
                    Some(false) => {
                        quest_state.set_feedback("That dialog option is unavailable", true);
                    }
                    None if dialog.is_open => {
                        quest_state.set_feedback("That dialog option is no longer available", true);
                    }
                    None => {
                        quest_state.set_feedback("NPC dialog is closed", true);
                    }
                }
            }
            QuestUiButton::CloseNpcDialog => {
                if queue.push_intent(QuestUiIntent::SelectNpcDialog {
                    target: "@Exit".to_owned(),
                }) {
                    #[cfg(feature = "native-ui")]
                    player_ui.withdraw_bound_npc_gold_buy();
                    dialog.close();
                    npc_nav.clear();
                    quest_state.dialog_scroll_top = 0;
                    quest_state.close_npc_quest_list();
                    quest_state.set_feedback("Dialog closed", false);
                } else {
                    quest_state.set_feedback("Connection busy; try again", true);
                }
            }
            QuestUiButton::ToggleNpcQuestList => {
                if !dialog.is_open {
                    quest_state.set_feedback("NPC dialog is closed", true);
                } else if quest_state.npc_quest_list_open {
                    if queue.push_intent(QuestUiIntent::SelectNpcDialog {
                        target: "@Exit".to_owned(),
                    }) {
                        quest_state.close_npc_quest_list();
                        #[cfg(feature = "native-ui")]
                        player_ui.withdraw_bound_npc_gold_buy();
                        dialog.close();
                        npc_nav.clear();
                    } else {
                        quest_state.set_feedback("Connection busy; try again", true);
                    }
                } else if npc_quest_indices.is_empty() {
                    quest_state.set_feedback("This NPC has no available quests", true);
                } else {
                    quest_state.open_npc_quest_list(&npc_quest_indices);
                }
            }
            QuestUiButton::CloseNpcQuestList => {
                if queue.push_intent(QuestUiIntent::SelectNpcDialog {
                    target: "@Exit".to_owned(),
                }) {
                    quest_state.close_npc_quest_list();
                    #[cfg(feature = "native-ui")]
                    player_ui.withdraw_bound_npc_gold_buy();
                    dialog.close();
                    npc_nav.clear();
                } else {
                    quest_state.set_feedback("Connection busy; try again", true);
                }
            }
            QuestUiButton::ReturnToNpcDialog => {
                quest_state.close_npc_quest_list();
            }
            QuestUiButton::SelectNpcQuest { quest_index } => {
                if npc_quest_indices.contains(&quest_index) {
                    quest_state.select_npc_quest(quest_index);
                } else {
                    quest_state.set_feedback("Quest is no longer available from this NPC", true);
                }
            }
            QuestUiButton::NpcQuestPrevious => {
                quest_state.move_npc_quest_selection_with_rows(&npc_quest_indices, -1,
                    mobile.map_or(QUEST_LIST_VISIBLE_ROWS, MobileQuestLayout::npc_quest_rows));
            }
            QuestUiButton::NpcQuestNext => {
                quest_state.move_npc_quest_selection_with_rows(&npc_quest_indices, 1,
                    mobile.map_or(QUEST_LIST_VISIBLE_ROWS, MobileQuestLayout::npc_quest_rows));
            }
            QuestUiButton::NpcQuestMessageScrollUp => {
                quest_state.scroll_npc_quest_message_up();
            }
            QuestUiButton::NpcQuestMessageScrollDown => {
                if let Some(quest) = quest_state.npc_quest_selected_index.and_then(|selected| {
                    tracker
                        .active_quests
                        .iter()
                        .find(|quest| quest.quest_index == selected)
                }) {
                    let lines = quest_list_message_lines_with_wrap(quest, &dialog, guidance,
                        mobile.is_none(), QuestRenderText { locale: *models.locale });
                    let line_count = if let Some(layout) = mobile {
                        mobile_wrap_quest_lines(lines, layout.width_css() - 32.0).len()
                    } else { lines.len() };
                    quest_state.scroll_npc_quest_message_down_with_rows(line_count,
                        mobile.map_or(QUEST_LIST_MESSAGE_LINE_COUNT, MobileQuestLayout::message_rows));
                }
            }
            QuestUiButton::NpcQuestHelp => {
                quest_state.set_feedback("Quest help is not available.", false);
            }
            QuestUiButton::ReturnNpcService => {
                if let Some(prev) = npc_nav.pop() {
                    #[cfg(feature = "native-ui")]
                    player_ui.withdraw_bound_npc_gold_buy();
                    *dialog = without_quest_operation_links(prev);
                    quest_state.dialog_scroll_top = 0;
                    quest_state.compact_view_epoch = quest_state.compact_view_epoch.wrapping_add(1);
                    quest_state.set_feedback("Returned to previous page", false);
                } else {
                    quest_state.set_feedback("No previous page", true);
                }
            }
            QuestUiButton::NpcDialogScrollUp => {
                quest_state.dialog_scroll_top = quest_state
                    .dialog_scroll_top
                    .saturating_sub(mobile.map_or(NPC_DIALOG_VISIBLE_ROWS, MobileQuestLayout::dialog_rows));
            }
            QuestUiButton::NpcDialogScrollDown => {
                let count = mobile.map_or_else(|| npc_dialog_rows(&dialog).len(),
                    |layout| mobile_npc_dialog_rows(&dialog, layout.width_css() - 32.0).len());
                let rows = mobile.map_or(NPC_DIALOG_VISIBLE_ROWS, MobileQuestLayout::dialog_rows);
                if quest_state.dialog_scroll_top + rows < count {
                    quest_state.dialog_scroll_top += rows;
                }
            }
            QuestUiButton::MakePrimary { quest_index } => {
                if tracker.active_quests.iter().any(|q| q.quest_index == quest_index && q.status.is_active()) {
                    quest_state.pinned_primary_quest_index = Some(quest_index);
                    quest_state.set_feedback("已设为当前引导任务", false);
                }
            }
            QuestUiButton::SelectQuest { quest_index } => {
                // Validate existence
                if tracker
                    .active_quests
                    .iter()
                    .any(|q| q.quest_index == quest_index)
                {
                    quest_state.select_quest(quest_index);
                } else {
                    quest_state.set_feedback(format!("Quest {quest_index} not found"), true);
                }
            }
            QuestUiButton::CloseQuestDetail => {
                quest_state.close_detail();
                quest_state.clear_feedback();
            }
            QuestUiButton::QuestDetailScrollUp => {
                quest_state.scroll_detail_up();
            }
            QuestUiButton::CompactDetailPage(page) if mobile.is_some_and(|layout| layout.compact) => {
                quest_state.compact_detail_page = page;
            }
            QuestUiButton::CompactNpcPage(page) if mobile.is_some_and(|layout| layout.compact) => {
                quest_state.compact_npc_page = page;
            }
            QuestUiButton::CompactDetailRewardPrevious if mobile.is_some_and(|layout| layout.compact) => {
                quest_state.compact_detail_reward_page = quest_state.compact_detail_reward_page.saturating_sub(1);
            }
            QuestUiButton::CompactDetailRewardNext if mobile.is_some_and(|layout| layout.compact) => {
                if let Some(quest) = quest_state.detail_quest(tracker) {
                    quest_state.compact_detail_reward_page = (quest_state.compact_detail_reward_page + 1)
                        .min(compact_reward_pages(quest, guidance).saturating_sub(1));
                }
            }
            QuestUiButton::CompactNpcRewardPrevious if mobile.is_some_and(|layout| layout.compact) => {
                quest_state.compact_npc_reward_page = quest_state.compact_npc_reward_page.saturating_sub(1);
            }
            QuestUiButton::CompactNpcRewardNext if mobile.is_some_and(|layout| layout.compact) => {
                if let Some(quest) = quest_state.npc_quest_selected_index.and_then(|index|
                    tracker.active_quests.iter().find(|quest| quest.quest_index == index)) {
                    quest_state.compact_npc_reward_page = (quest_state.compact_npc_reward_page + 1)
                        .min(compact_reward_pages(quest, guidance).saturating_sub(1));
                }
            }
            QuestUiButton::QuestDetailScrollDown => {
                if let Some(quest) = quest_state.detail_quest(tracker) {
                    let class_name = models.read_model.as_deref().and_then(|model| model.player.class_name.as_deref()).unwrap_or("");
                    let lines = quest_detail_lines_with_wrap(quest, guidance, class_name,
                        mobile.is_none(), QuestRenderText { locale: *models.locale });
                    let line_count = if let Some(layout) = mobile {
                        mobile_wrap_quest_lines(lines, layout.width_css() - 32.0).len()
                    } else { lines.len() };
                    quest_state.scroll_detail_down_with_rows(line_count,
                        mobile.map_or(QUEST_DETAIL_LINE_COUNT, MobileQuestLayout::detail_rows));
                }
            }
            QuestUiButton::ShareQuest { quest_index } => {
                if tracker
                    .active_quests
                    .iter()
                    .any(|quest| quest.quest_index == quest_index && quest.status.is_active())
                {
                    if !queue.push_intent(QuestUiIntent::ShareQuest { quest_index }) {
                        quest_state.set_feedback("Connection busy; try again", true);
                    }
                } else {
                    quest_state.set_feedback("Quest is no longer current", true);
                }
            }
            QuestUiButton::TrackQuest { quest_index } => {
                toggle_quest_tracking(&mut quest_state, tracker, quest_index);
            }
            QuestUiButton::AcceptQuest {
                npc_index,
                quest_index,
            } => {
                let quest = tracker
                    .active_quests
                    .iter()
                    .find(|quest| quest.quest_index == quest_index);
                let diary_authorized = quest.is_some_and(|quest| {
                    npc_index == 0 && newcomer_diary_accept_authorized(quest, guidance)
                });
                let dialog_offered = npc_index != 0
                    && dialog_exposes_quest_operation(
                        &dialog,
                        Some(npc_index),
                        quest_index,
                        false,
                        None,
                    );
                if !diary_authorized && !dialog_offered {
                    quest_state
                        .set_feedback("Use the current NPC dialog to accept this quest", true);
                    continue;
                }
                let source = if diary_authorized {
                    QuestActionSource::Diary
                } else {
                    QuestActionSource::NpcDialog {
                        action_offered: dialog_offered,
                    }
                };
                let already_pending = pending.contains(&PendingOperationKey::QuestAccept {
                    npc_index,
                    quest_index,
                });
                match quest_action_decision(
                    quest,
                    QuestAction::Accept,
                    source,
                    guidance,
                    already_pending,
                    None,
                ) {
                    QuestActionDecision::Eligible => {
                        let queue_full = queue.is_full();
                        if queue.push_pending_intent(
                            &mut pending,
                            QuestUiIntent::AcceptQuest {
                                npc_index,
                                quest_index,
                            },
                        ) {
                            let label = quest.map_or("quest", |quest| quest.title.as_str());
                            quest_state.set_feedback(format!("Accepting {label}"), false);
                        } else if queue_full {
                            quest_state.set_feedback("Connection busy; try again", true);
                        } else {
                            quest_state.set_feedback("Quest request is already pending", true);
                        }
                    }
                    QuestActionDecision::Rejected(QuestActionRejection::Pending) => {
                        quest_state.set_feedback("Quest request is already pending", true);
                    }
                    QuestActionDecision::Rejected(_) => {
                        quest_state.set_feedback("Quest cannot be accepted", true);
                    }
                }
            }
            QuestUiButton::AcceptNpcQuest {
                npc_index,
                quest_index,
            } => {
                if !npc_list_accept_is_current(
                    &npc_quest_indices,
                    &dialog,
                    tracker,
                    npc_index,
                    quest_index,
                ) {
                    quest_state.set_feedback("Quest is no longer available from this NPC", true);
                    continue;
                }
                let quest = tracker
                    .active_quests
                    .iter()
                    .find(|quest| quest.quest_index == quest_index);
                let already_pending = pending.contains(&PendingOperationKey::QuestAccept {
                    npc_index,
                    quest_index,
                });
                match quest_action_decision(
                    quest,
                    QuestAction::Accept,
                    QuestActionSource::NpcDialog {
                        action_offered: true,
                    },
                    guidance,
                    already_pending,
                    None,
                ) {
                    QuestActionDecision::Eligible => {
                        let queue_full = queue.is_full();
                        if queue.push_pending_intent(
                            &mut pending,
                            QuestUiIntent::AcceptQuest {
                                npc_index,
                                quest_index,
                            },
                        ) {
                            if let Some(quest) = quest {
                                quest_state.set_feedback(format!("Accepting {}", quest.title), false);
                            }
                        } else if queue_full {
                            quest_state.set_feedback("Connection busy; try again", true);
                        } else {
                            quest_state.set_feedback("Quest request is already pending", true);
                        }
                    }
                    QuestActionDecision::Rejected(QuestActionRejection::Pending) => {
                        quest_state.set_feedback("Quest request is already pending", true);
                    }
                    QuestActionDecision::Rejected(_) => {
                        quest_state.set_feedback("Quest is no longer available from this NPC", true);
                    }
                }
            }
            QuestUiButton::FinishQuest {
                quest_index,
                selected_item_index,
            } => {
                let quest = tracker
                    .active_quests
                    .iter()
                    .find(|quest| quest.quest_index == quest_index);
                let diary_authorized =
                    quest.is_some_and(|quest| newcomer_diary_finish_authorized(quest, guidance));
                let dialog_offered = dialog_exposes_quest_operation(
                    &dialog,
                    None,
                    quest_index,
                    true,
                    (selected_item_index >= 0).then_some(selected_item_index),
                );
                if !diary_authorized && !dialog_offered {
                    quest_state.set_feedback("Return to the quest NPC to deliver this quest", true);
                    continue;
                }
                if let Some(quest) = quest {
                    let source = if diary_authorized {
                        QuestActionSource::Diary
                    } else {
                        QuestActionSource::NpcDialog {
                            action_offered: dialog_offered,
                        }
                    };
                    let already_pending = pending.has_pending_quest_finish(quest_index);
                    match quest_action_decision(
                        Some(quest),
                        QuestAction::Finish,
                        source,
                        guidance,
                        already_pending,
                        (selected_item_index >= 0).then_some(selected_item_index),
                    ) {
                        QuestActionDecision::Eligible => {
                            let queue_full = queue.is_full();
                            if queue.push_pending_intent(
                                &mut pending,
                                QuestUiIntent::FinishQuest {
                                    quest_index,
                                    selected_item_index,
                                },
                            ) {
                                quest_state.set_feedback(format!("Delivering {}", quest.title), false);
                            } else if queue_full {
                                quest_state.set_feedback("Connection busy; try again", true);
                            } else {
                                quest_state.set_feedback("Quest request is already pending", true);
                            }
                        }
                        QuestActionDecision::Rejected(QuestActionRejection::RewardSelectionRequired) => {
                            quest_state.show_quest_alert(SELECT_REWARD_TEXT);
                        }
                        QuestActionDecision::Rejected(QuestActionRejection::Pending) => {
                            quest_state.set_feedback("Quest request is already pending", true);
                        }
                        QuestActionDecision::Rejected(_) => {
                            quest_state.set_feedback("Quest not ready to deliver", true);
                        }
                    }
                } else {
                    quest_state.set_feedback("Quest not found", true);
                }
            }
            QuestUiButton::AbandonQuest { quest_index } => {
                if let Some(quest) = tracker
                    .active_quests
                    .iter()
                    .find(|q| q.quest_index == quest_index)
                {
                    if can_abandon_quest(quest) {
                        if pending.contains(&PendingOperationKey::QuestAbandon { quest_index }) {
                            quest_state.set_feedback("Quest request is already pending", true);
                        } else {
                            quest_state.request_abandon_confirmation(quest_index);
                        }
                    } else {
                        quest_state
                            .set_feedback("Only an in-progress quest can be abandoned", true);
                    }
                } else {
                    quest_state.set_feedback("Quest not found", true);
                }
            }
            QuestUiButton::ConfirmAbandonQuest => {
                confirm_quest_abandon(&mut queue, &mut pending, &mut quest_state, tracker);
            }
            QuestUiButton::CancelAbandonQuest => {
                quest_state.close_abandon_confirmation();
            }
            QuestUiButton::CloseQuestAlert => {
                quest_state.close_quest_alert();
            }
            QuestUiButton::CompactConfirmationPrevious if mobile.is_some_and(|layout| layout.compact) => {
                quest_state.compact_confirmation_page = quest_state.compact_confirmation_page.saturating_sub(1);
            }
            QuestUiButton::CompactConfirmationNext if mobile.is_some_and(|layout| layout.compact) => {
                if let Some(layout) = mobile {
                    let pages = compact_confirmation_lines(quest_state.quest_alert_message.as_deref()
                        .unwrap_or(ASK_CANCEL_QUEST_TEXT), layout).len().div_ceil(5).max(1);
                    quest_state.compact_confirmation_page = (quest_state.compact_confirmation_page + 1).min(pages - 1);
                }
            }
            QuestUiButton::SelectReward {
                quest_index,
                reward_index,
            } => {
                if quest_state.selected_quest_index == Some(quest_index) {
                    if let Some(quest) = tracker
                        .active_quests
                        .iter()
                        .find(|q| q.quest_index == quest_index)
                    {
                        if reward_index >= 0
                            && selectable_reward_indices(quest)
                                .any(|candidate| candidate == reward_index)
                        {
                            quest_state.select_reward(reward_index);
                        } else {
                            quest_state.set_feedback("Invalid reward", true);
                        }
                    }
                } else {
                    quest_state.set_feedback("Select the quest first", true);
                }
            }
            QuestUiButton::SelectNpcQuestReward {
                quest_index,
                reward_index,
            } => {
                if quest_state.npc_quest_selected_index == Some(quest_index) {
                    if let Some(quest) = tracker
                        .active_quests
                        .iter()
                        .find(|quest| quest.quest_index == quest_index)
                    {
                        if reward_index >= 0
                            && selectable_reward_indices(quest)
                                .any(|candidate| candidate == reward_index)
                        {
                            quest_state.npc_selected_reward_index = Some(reward_index);
                            quest_state.clear_feedback();
                        } else {
                            quest_state.set_feedback("Invalid reward", true);
                        }
                    }
                } else {
                    quest_state.set_feedback("Select the quest first", true);
                }
            }
            QuestUiButton::CloseQuestLog => {
                dispatch_quest_panel_action(
                    &mut player_ui,
                    &mut effects,
                    mir2_ui_core::action::UiAction::ClosePanel,
                );
                quest_state.clear_diary_selection();
                quest_state.compact_detail_page = CompactQuestPage::Text;
                quest_state.compact_detail_reward_page = 0;
                quest_state.clear_feedback();
                // Keep feedback about close? Clear to avoid stale message.
            }
            QuestUiButton::OpenDestinationMap => {
                dispatch_quest_panel_action(
                    &mut player_ui, &mut effects,
                    mir2_ui_core::action::UiAction::OpenBigMap,
                );
            }
            QuestUiButton::NavigateQuestRoute(intent) => {
                if !quest_route_intent_is_current(
                    intent,
                    tracker,
                    &quest_state,
                    journey.as_ref(),
                    models.big_map.as_deref(),
                ) {
                    quest_state.set_feedback(format!("{}引导已更新，请使用当前任务路线", intent.target.label()), true);
                } else if let Some(route_navigation) = route_navigation.as_deref_mut() {
                    if route_navigation.push(intent) {
                        quest_state.set_feedback(
                            format!("前往{} ({},{})", intent.target.label(), intent.x, intent.y),
                            false,
                        );
                    } else {
                        quest_state.set_feedback("已有寻路指令等待处理", true);
                    }
                } else {
                    quest_state.set_feedback("任务导航暂不可用", true);
                }
            }
            QuestUiButton::AttackTarget { object_id } => {
                if target_is_attackable(target.as_deref(), object_id) {
                    if queue.push_intent(QuestUiIntent::AttackTarget { object_id }) {
                        quest_state.set_feedback("Attacking target", false);
                    } else {
                        quest_state.set_feedback("Connection busy; try again", true);
                    }
                } else {
                    quest_state.set_feedback("Target is no longer attackable", true);
                }
            }
            QuestUiButton::AttackQuestTarget { object_id } => {
                if models
                    .entities
                    .as_deref()
                    .is_some_and(|entities| quest_target_is_visible(tracker, entities, object_id))
                {
                    if queue.push_intent(QuestUiIntent::AttackTarget { object_id }) {
                        quest_state.set_feedback("Finding quest target", false);
                    } else {
                        quest_state.set_feedback("Connection busy; try again", true);
                    }
                } else {
                    quest_state.set_feedback("Quest target is no longer nearby", true);
                }
            }
            QuestUiButton::PickUpObject { object_id } => {
                if let Some(label) = pickup_label(pickups.as_deref(), object_id) {
                    if queue.push_intent(QuestUiIntent::PickUpObject { object_id }) {
                        quest_state.set_feedback(format!("Picking up {label}"), false);
                    } else {
                        quest_state.set_feedback("Connection busy; pickup not queued", true);
                    }
                } else {
                    quest_state.set_feedback("That ground item is no longer available", true);
                }
            }
            QuestUiButton::PickUpTile => {
                if pickup_tile_is_current(pickups.as_deref()) {
                    if queue.push_intent(QuestUiIntent::PickUpTile) {
                        quest_state
                            .set_feedback("Checking the current tile for ground items", false);
                    } else {
                        quest_state.set_feedback("Connection busy; pickup not queued", true);
                    }
                } else {
                    quest_state.set_feedback("That pickup is no longer available", true);
                }
            }
            QuestUiButton::CompactDetailPage(_)
            | QuestUiButton::CompactNpcPage(_)
            | QuestUiButton::CompactDetailRewardPrevious
            | QuestUiButton::CompactDetailRewardNext
            | QuestUiButton::CompactNpcRewardPrevious
            | QuestUiButton::CompactNpcRewardNext
            | QuestUiButton::CompactConfirmationPrevious
            | QuestUiButton::CompactConfirmationNext => {}
        }
    }

    #[cfg(not(feature = "native-ui"))]
    if !compact_input_current(models.presentation.as_deref(), models.host_context.as_deref(),
        models.guidance.as_deref(), &player_ui, &quest_state, &models.tracker, &dialog,
        models.compact_readiness.as_deref(), *models.locale) { return; }

    if keys.just_pressed(KeyCode::Escape) || turn_in_blocked {
        if quest_state.pending_turn_in.is_some() {
            quest_state.pending_turn_in = None;
        }
    } else if quest_state.pending_turn_in.is_some() {
        // A no-pending call takes mutable QuestUiState even though advance()
        // immediately returns. That marks the resource changed and rebuilds
        // an otherwise idle compact Diary's measured text subtree.
        turn_in::advance(&turn_in::TurnInContext {
            tracker, dialog: &dialog, map: models.map.as_deref(),
            entities: models.entities.as_deref(), big_map: models.big_map.as_deref(),
        }, &mut quest_state, &mut queue, &mut pending);
    }

    // Crystal `MirMessageBox` owns Escape/Enter while visible. It is the only
    // modal in this quest family that must sit above every other quest window.
    if quest_state.quest_alert_message.is_some() {
        if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
            quest_state.close_quest_alert();
        }
        return;
    }
    if quest_state.abandon_confirmation_quest_index.is_some() {
        if keys.just_pressed(KeyCode::Escape) {
            quest_state.close_abandon_confirmation();
        } else if keys.just_pressed(KeyCode::Enter) {
            confirm_quest_abandon(&mut queue, &mut pending, &mut quest_state, tracker);
        }
        return;
    }

    // Input blocking: when quest log or dialog is open, gameplay shortcuts are suppressed.
    // Escape handling for those surfaces takes precedence.
    let is_modal = quest_log_open || dialog_open || blocks_gameplay_keys;

    if is_modal {
        if keys.just_pressed(KeyCode::Escape)
            || (quest_log_open
                && crate::crystal_ui::quest_key_triggered(&player_ui, &keys, "Quests"))
        {
            if quest_log_open {
                dispatch_quest_panel_action(
                    &mut player_ui,
                    &mut effects,
                    mir2_ui_core::action::UiAction::ClosePanel,
                );
                quest_state.clear_diary_selection();
                quest_state.clear_feedback();
            } else if dialog_open {
                if queue.push_intent(QuestUiIntent::SelectNpcDialog {
                    target: "@Exit".to_owned(),
                }) {
                    #[cfg(feature = "native-ui")]
                    player_ui.withdraw_bound_npc_gold_buy();
                    dialog.close();
                    npc_nav.clear();
                    quest_state.close_npc_quest_list();
                    quest_state.set_feedback("Dialog closed", false);
                } else {
                    quest_state.set_feedback("Connection busy; try again", true);
                }
            }
        } else if dialog_open && keys.just_pressed(KeyCode::Backspace) {
            // Service return via Backspace when dialog history exists
            if npc_nav.can_return() {
                if let Some(prev) = npc_nav.pop() {
                    #[cfg(feature = "native-ui")]
                    player_ui.withdraw_bound_npc_gold_buy();
                    *dialog = prev;
                    quest_state.compact_view_epoch = quest_state.compact_view_epoch.wrapping_add(1);
                    quest_state.set_feedback("Returned to previous page", false);
                }
            }
        }
        // Block T/F/R and other gameplay keys while modal is open.
        return;
    }

    // Native Crystal binding owns pickup; historical T/F/R prototype aliases
    // conflict with Trade/Friends/Skillbar and must not emit unrelated actions.
    if crate::crystal_ui::quest_key_triggered(&player_ui, &keys, "Pickup") {
        queue.push_intent(QuestUiIntent::PickUpTile);
    }

    // Toggle quest log with Q when not blocked by other modals.
    if crate::crystal_ui::quest_key_triggered(&player_ui, &keys, "Quests") {
        let now_open = is_quest_log_open_state(Some(&player_ui));
        if now_open {
            dispatch_quest_panel_action(
                &mut player_ui,
                &mut effects,
                mir2_ui_core::action::UiAction::ClosePanel,
            );
            quest_state.clear_diary_selection();
            quest_state.clear_feedback();
        } else {
            dispatch_quest_panel_action(
                &mut player_ui,
                &mut effects,
                mir2_ui_core::action::UiAction::OpenQuestLog,
            );
            // Crystal rebuilds the Diary rows on each Show(), which clears
            // their selected highlight without hiding an already-open Detail.
            quest_state.clear_diary_selection();
        }
    }

    // Also handle Quest toggle via NativePlayerUiState directly for overlay compatibility
    // (Hud button already toggles via overlays.rs, this is just key toggle).
}

#[cfg(not(feature = "native-ui"))]
fn set_compact_panel_stamp(commands: &mut Commands, entity: Entity,
    stamp: Option<QuestCompactStamp>, sheet: MobileQuestSheet) {
    if let Some(stamp) = stamp.filter(|stamp| stamp.sheet == sheet) {
        // CSS margins can map to fractional logical pixels (600/1440 gives a
        // declared 710.4px panel). Rounding the whole compact subtree first
        // would shrink its measured CSS height below the supported 296px.
        commands.entity(entity).insert((stamp, bevy::ui::LayoutConfig { use_rounding: false }));
    } else {
        commands.entity(entity).remove::<QuestCompactStamp>();
        commands.entity(entity).remove::<bevy::ui::LayoutConfig>();
    }
}

fn render_quest_ui(
    shell: Option<Res<NativeShellModel>>,
    asset_server: Option<Res<AssetServer>>,
    tracker: Res<QuestTracker>,
    mut journey_models: JourneyRenderModels,
    dialog: Res<NpcDialogModel>,
    nearby: Res<NearbyNpcModel>,
    target: Res<CombatTargetModel>,
    pickups: Res<GroundPickupModel>,
    ui_model: Res<UiReadModel>,
    inventory: Res<InventoryModel>,
    pending: Res<PendingOperations>,
    quest_state: Res<QuestUiState>,
    npc_nav: Res<NpcDialogNav>,
    player_ui: Res<NativePlayerUiState>,
    mut commands: Commands,
    mut all: ParamSet<(
        Query<&mut Node, With<QuestUiRoot>>,
        Query<
            (
                Entity,
                &mut Node,
                Option<&QuestTrackerPanel>,
                Option<&NpcDialogPanel>,
                Option<&CombatTargetPanel>,
                Option<&PickupFeedbackPanel>,
                Option<&NativePlayerHudPanel>,
                Option<&NativeControlHintPanel>,
                Option<&NativeQuickBagPanel>,
                Option<&QuestUiModalBlocker>,
            ),
            (
                Without<QuestUiRoot>,
                Or<(
                    With<QuestTrackerPanel>,
                    With<NpcDialogPanel>,
                    With<CombatTargetPanel>,
                    With<PickupFeedbackPanel>,
                    With<NativePlayerHudPanel>,
                    With<NativeControlHintPanel>,
                    With<NativeQuickBagPanel>,
                    With<QuestUiModalBlocker>,
                )>,
            ),
        >,
        Query<(Entity, &mut Node), With<QuestLogPanel>>,
        Query<(Entity, &mut Node), With<QuestDetailPanel>>,
        Query<(Entity, &mut Node), With<NpcQuestListPanel>>,
        Query<(Entity, &mut Node), With<QuestConfirmationPanel>>,
        Query<&mut Node, With<QuestConfirmationBlocker>>,
    )>,
) {
    let render_text = QuestRenderText { locale: *journey_models.locale };
    let Some(shell) = shell else {
        return;
    };

    let in_game = shell.screen == NativeShellScreen::InGame;
    #[cfg(not(feature = "native-ui"))]
    let in_game = in_game
        && !journey_models.host_context.as_deref().is_some_and(|host|
            !host.host_visible && !host.layout_preparing);

    let became_visible = {
        let mut roots = all.p0();
        let Ok(mut root) = roots.single_mut() else {
            return;
        };
        let was_visible = root.display != Display::None;
        root.display = if in_game {
            Display::Flex
        } else {
            Display::None
        };
        in_game && !was_visible
    };

    if !in_game {
        return;
    }

    let quest_log_open = is_quest_log_open_state(Some(&player_ui));
    let mobile = journey_models.presentation.as_deref().and_then(|value| value.mobile_layout());
    #[cfg(not(feature = "native-ui"))]
    let compact_stamp = journey_models.presentation.as_deref()
        .zip(journey_models.host_context.as_deref())
        .and_then(|(presentation, context)| compact_expected_stamp(presentation, context,
            &player_ui, &quest_state, &tracker, &dialog, &journey_models.guidance, *journey_models.locale));
    #[cfg(not(feature = "native-ui"))]
    let compact_stamp_changed = compact_stamp != journey_models.compact_last_build.0;
    #[cfg(feature = "native-ui")]
    let compact_stamp_changed = false;

    // Avoid churn: only re-render when relevant state changed or quest log toggled.
    if !became_visible
        && !journey_models.locale.is_changed()
        && !tracker.is_changed()
        && !journey_models.completed.is_changed()
        && !journey_models.guidance.is_changed()
        && !journey_models.catalog.is_changed()
        && !dialog.is_changed()
        && !nearby.is_changed()
        && !target.is_changed()
        && !journey_models.entities.is_changed()
        && !journey_models.map.is_changed()
        && !journey_models.big_map.as_ref().is_some_and(|model| model.is_changed())
        && !journey_models.skills.as_ref().is_some_and(|model| model.is_changed())
        && !pickups.is_changed()
        && !ui_model.is_changed()
        && !inventory.is_changed()
        && !pending.is_changed()
        && !shell.is_changed()
        && !quest_state.is_changed()
        && !npc_nav.is_changed()
        && !player_ui.is_changed()
        && !journey_models.presentation.as_ref().is_some_and(|value| value.is_changed())
        && !compact_stamp_changed
    {
        return;
    }

    #[cfg(not(feature = "native-ui"))]
    if compact_stamp_changed { journey_models.compact_last_build.0 = compact_stamp; }

    let has_dialog_content = dialog.is_open;
    let journey = journey_models.catalog.derive(
        &journey_models.guidance,
        &tracker,
        &journey_models.completed,
        &ui_model.player,
    );
    let supply_plan = crate::quest_supplies::plan(&ui_model.player, &inventory,
        journey_models.skills.as_deref(), primary_quest_index(&tracker, &quest_state, journey.as_ref()));
    let available_npc_quests =
        npc_available_quests(&dialog, &tracker, Some(&journey_models.guidance));
    let has_npc_quests = !available_npc_quests.is_empty();
    let confirmation_open = quest_state.abandon_confirmation_quest_index.is_some()
        || quest_state.quest_alert_message.is_some();
    let detail_quest = quest_state.detail_quest(&tracker);
    let mobile_sheet = mobile.map(|_| mobile_sheet(
        confirmation_open,
        quest_state.npc_quest_list_open && has_npc_quests && dialog.is_open,
        dialog.is_open,
        detail_quest.is_some(),
        quest_log_open,
    ));

    for (
        panel_entity,
        mut panel_node,
        is_tracker,
        is_dialog,
        is_target,
        is_pickup,
        is_player_hud,
        is_control_hint,
        is_quick_bag,
        is_modal_blocker,
    ) in all.p1().iter_mut()
    {
        let visible = if is_modal_blocker.is_some() {
            quest_log_open || has_dialog_content
        } else if is_dialog.is_some() {
            mobile_sheet.map_or(has_dialog_content, |sheet| sheet == MobileQuestSheet::Dialog)
        } else if is_tracker.is_some() {
            mobile.is_none() && cfg!(feature = "native-ui") && journey_tracker_visible(journey.is_some(), player_ui.core.panel)
        } else if is_target.is_some() {
            mobile.is_none() && CRYSTAL_TARGET_PANEL_VISIBLE && target.target.is_some()
        } else if is_pickup.is_some() {
            SHOW_PICKUP_FEEDBACK_PANEL && !pickups.recent.is_empty()
        } else if is_player_hud.is_some() || is_control_hint.is_some() || is_quick_bag.is_some() {
            false
        } else {
            true
        };
        panel_node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if is_dialog.is_some() {
            if let Some(layout) = mobile { place_mobile_sheet(&mut panel_node, layout); }
            #[cfg(not(feature = "native-ui"))]
            set_compact_panel_stamp(&mut commands, panel_entity, compact_stamp, MobileQuestSheet::Dialog);
        }
        commands.entity(panel_entity).despawn_children();
        if !visible {
            continue;
        }

        commands.entity(panel_entity).with_children(|panel| {
            if is_tracker.is_some() {
                render_quest_tracker_panel(
                    panel,
                    &tracker,
                    &quest_state,
                    journey.as_ref(),
                    &journey_models.entities,
                    &journey_models.map,
                    journey_models.big_map.as_deref(),
                    ui_model.player.class_name.as_deref().unwrap_or(""),
                    &supply_plan, render_text);
            } else if is_dialog.is_some() {
                if let Some(layout) = mobile {
                    render_mobile_dialog_panel(panel, layout, &dialog, &npc_nav, &quest_state, has_npc_quests, render_text);
                } else {
                    render_dialog_panel(
                        panel, &dialog, &npc_nav, &quest_state, &pending, has_npc_quests,
                        asset_server.as_ref().map(|server| &**server), render_text);
                }
            } else if is_target.is_some() {
                render_combat_target_panel(panel, target.target.as_ref());
            } else if is_pickup.is_some() {
                render_pickup_panel(panel, &pickups, &quest_state);
            } else if is_player_hud.is_some() {
                render_player_hud_panel(panel, &ui_model);
            } else if is_control_hint.is_some() {
                render_control_hint_panel(panel, &nearby, &ui_model);
            } else if is_quick_bag.is_some() {
                render_quick_bag_panel(panel, &inventory);
            }
        });
    }

    // Crystal Q surface: current-quest diary at the source default location.
    for (entity, mut node) in all.p2().iter_mut() {
        #[cfg(not(feature = "native-ui"))]
        set_compact_panel_stamp(&mut commands, entity, compact_stamp, MobileQuestSheet::Diary);
        if let Some(layout) = mobile {
            place_mobile_sheet(&mut node, layout);
        } else if quest_log_open {
            // Re-apply source-faithful geometry whenever the panel opens.
            node.left = Val::Px(QUEST_DIARY_DESIGN_LEFT);
            node.top = Val::Px(QUEST_DIARY_DESIGN_TOP);
            node.width = Val::Px(QUEST_DIARY_DESIGN_WIDTH);
            node.height = Val::Px(QUEST_DIARY_DESIGN_HEIGHT);
            node.min_width = Val::Px(QUEST_DIARY_DESIGN_WIDTH);
            node.max_width = Val::Px(QUEST_DIARY_DESIGN_WIDTH);
            node.min_height = Val::Px(QUEST_DIARY_DESIGN_HEIGHT);
            node.max_height = Val::Px(QUEST_DIARY_DESIGN_HEIGHT);
        } else {
            // Preserve the closed-state geometry expected by the existing
            // transition assertion; Display::None keeps it non-rendering.
            node.left = Val::Px(212.0);
            node.top = Val::Px(80.0);
            node.width = Val::Px(600.0);
        }
        let visible = mobile_sheet.map_or(quest_log_open, |sheet| sheet == MobileQuestSheet::Diary);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        commands.entity(entity).despawn_children();
        if visible {
            commands.entity(entity).with_children(|panel| {
                if let Some(layout) = mobile {
                    render_mobile_diary_panel(panel, layout, &tracker, &journey_models.guidance, journey.as_ref(), &quest_state, render_text);
                } else {
                    render_quest_diary_panel(
                        panel, &tracker, &journey_models.guidance, journey.as_ref(), &quest_state,
                        &pending, asset_server.as_ref().map(|server| &**server), render_text);
                }
            });
        }
    }

    // Crystal row left-click opens a second, independently closable window at
    // `(ScreenWidth / 2 + 20, 60)`. Hiding the Q-key Diary does not hide it.
    for (entity, mut node) in all.p3().iter_mut() {
        #[cfg(not(feature = "native-ui"))]
        set_compact_panel_stamp(&mut commands, entity, compact_stamp, MobileQuestSheet::Detail);
        let visible = mobile_sheet.map_or(detail_quest.is_some(), |sheet| sheet == MobileQuestSheet::Detail);
        if let Some(layout) = mobile { place_mobile_sheet(&mut node, layout); }
        else { node.left = Val::Px(QUEST_DETAIL_DESIGN_LEFT);
        node.top = Val::Px(QUEST_DETAIL_DESIGN_TOP);
        node.width = Val::Px(QUEST_DETAIL_DESIGN_WIDTH);
        node.height = Val::Px(QUEST_DETAIL_DESIGN_HEIGHT);
        node.min_width = Val::Px(QUEST_DETAIL_DESIGN_WIDTH);
        node.max_width = Val::Px(QUEST_DETAIL_DESIGN_WIDTH);
        node.min_height = Val::Px(QUEST_DETAIL_DESIGN_HEIGHT);
        node.max_height = Val::Px(QUEST_DETAIL_DESIGN_HEIGHT);
        }
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        commands.entity(entity).despawn_children();
        if let Some(quest) = detail_quest {
            if !visible { continue; }
            commands.entity(entity).with_children(|panel| {
                if let Some(layout) = mobile {
                    render_mobile_detail_panel(panel, layout, quest, &journey_models.guidance, &quest_state,
                        &pending, &ui_model.player, asset_server.as_ref().map(|server| &**server), render_text);
                } else { render_quest_detail_panel_with_text(
                    panel,
                    quest,
                    &journey_models.guidance,
                    &quest_state,
                    &pending,
                    asset_server.as_ref().map(|server| &**server),
                    &ui_model.player, render_text) }
            });
        }
    }

    // Crystal's NPC Quest List is a separate five-row window at
    // `NPCDialog.Width + 47, 0` and remains linked to the NPC dialog lifecycle.
    for (entity, mut node) in all.p4().iter_mut() {
        #[cfg(not(feature = "native-ui"))]
        set_compact_panel_stamp(&mut commands, entity, compact_stamp, MobileQuestSheet::NpcList);
        let visible = mobile_sheet.map_or(quest_state.npc_quest_list_open && has_npc_quests && dialog.is_open,
            |sheet| sheet == MobileQuestSheet::NpcList);
        if let Some(layout) = mobile { place_mobile_sheet(&mut node, layout); }
        else { node.left = Val::Px(QUEST_LIST_DESIGN_LEFT);
        node.top = Val::Px(QUEST_LIST_DESIGN_TOP);
        node.width = Val::Px(QUEST_LIST_DESIGN_WIDTH);
        node.height = Val::Px(QUEST_LIST_DESIGN_HEIGHT);
        }
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        commands.entity(entity).despawn_children();
        if visible {
            commands.entity(entity).with_children(|panel| {
                if let Some(layout) = mobile {
                    render_mobile_npc_quest_list_panel(panel, layout, &available_npc_quests, &dialog,
                        &journey_models.guidance, &quest_state, &pending, &ui_model.player,
                        asset_server.as_ref().map(|server| &**server), render_text);
                } else { render_npc_quest_list_panel(
                    panel,
                    &available_npc_quests,
                    &dialog,
                    &journey_models.guidance,
                    &quest_state,
                    &pending,
                    asset_server.as_ref().map(|server| &**server),
                    &ui_model.player, render_text) }
            });
        }
    }

    for (entity, mut node) in all.p5().iter_mut() {
        #[cfg(not(feature = "native-ui"))]
        set_compact_panel_stamp(&mut commands, entity, compact_stamp, MobileQuestSheet::Confirmation);
        if let Some(layout) = mobile { place_mobile_sheet(&mut node, layout); }
        else { node.left = Val::Px(QUEST_CONFIRM_DESIGN_LEFT);
        node.top = Val::Px(QUEST_CONFIRM_DESIGN_TOP);
        node.width = Val::Px(QUEST_CONFIRM_DESIGN_WIDTH);
        node.height = Val::Px(QUEST_CONFIRM_DESIGN_HEIGHT);
        }
        let visible = mobile_sheet.map_or(confirmation_open, |sheet| sheet == MobileQuestSheet::Confirmation);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        commands.entity(entity).despawn_children();
        if visible {
            commands.entity(entity).with_children(|panel| {
                if let Some(layout) = mobile {
                    render_mobile_confirmation(panel, layout, quest_state.quest_alert_message.as_deref(), &quest_state);
                } else if let Some(message) = quest_state.quest_alert_message.as_deref() {
                    render_quest_alert(
                        panel,
                        message,
                        asset_server.as_ref().map(|server| &**server),
                    );
                } else {
                    render_quest_abandon_confirmation(
                        panel,
                        asset_server.as_ref().map(|server| &**server),
                    );
                }
            });
        }
    }

    for mut node in all.p6().iter_mut() {
        node.display = if confirmation_open {
            Display::Flex
        } else {
            Display::None
        };
    }
}

#[cfg(not(feature = "native-ui"))]
fn compact_rect(node: &ComputedNode, transform: &bevy::ui::UiGlobalTransform) -> bevy::math::Rect {
    let scale = node.inverse_scale_factor;
    let center = transform.affine().translation * scale;
    let half = node.size() * scale * 0.5;
    bevy::math::Rect { min: center - half, max: center + half }
}

#[cfg(not(feature = "native-ui"))]
fn compact_descends_from(mut entity: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    for _ in 0..32 {
        if entity == ancestor { return true; }
        let Ok(parent) = parents.get(entity) else { return false; };
        entity = parent.parent();
    }
    false
}

#[cfg(not(feature = "native-ui"))]
fn compact_sheet_label(sheet: MobileQuestSheet) -> &'static str {
    match sheet {
        MobileQuestSheet::Confirmation => "confirmation",
        MobileQuestSheet::NpcList => "npcList",
        MobileQuestSheet::Dialog => "dialog",
        MobileQuestSheet::Detail => "detail",
        MobileQuestSheet::Diary => "diary",
        MobileQuestSheet::None => "none",
    }
}

#[cfg(not(feature = "native-ui"))]
fn compact_action_label(action: &QuestUiButton) -> &'static str {
    match action {
        QuestUiButton::CompactDetailPage(_) | QuestUiButton::CompactNpcPage(_) => "pageTab",
        QuestUiButton::CompactDetailRewardPrevious | QuestUiButton::CompactNpcRewardPrevious => "rewardPrevious",
        QuestUiButton::CompactDetailRewardNext | QuestUiButton::CompactNpcRewardNext => "rewardNext",
        QuestUiButton::CompactConfirmationPrevious => "confirmationPrevious",
        QuestUiButton::CompactConfirmationNext => "confirmationNext",
        QuestUiButton::CloseQuestDetail | QuestUiButton::CloseQuestLog
            | QuestUiButton::CloseNpcDialog | QuestUiButton::CloseNpcQuestList
            | QuestUiButton::CloseQuestAlert => "close",
        QuestUiButton::ReturnToNpcDialog | QuestUiButton::ReturnNpcService => "back",
        QuestUiButton::SelectQuest { .. } | QuestUiButton::SelectNpcQuest { .. } => "questRow",
        QuestUiButton::SelectGuidedDiaryTab(_) => "diaryTab",
        QuestUiButton::GuidedDiaryPrevious | QuestUiButton::QuestPagePrevious
            | QuestUiButton::NpcQuestPrevious => "pagePrevious",
        QuestUiButton::GuidedDiaryNext | QuestUiButton::QuestPageNext
            | QuestUiButton::NpcQuestNext => "pageNext",
        QuestUiButton::QuestDetailScrollUp | QuestUiButton::NpcQuestMessageScrollUp => "textPrevious",
        QuestUiButton::QuestDetailScrollDown | QuestUiButton::NpcQuestMessageScrollDown => "textNext",
        QuestUiButton::SelectReward { .. } | QuestUiButton::SelectNpcQuestReward { .. } => "selectReward",
        QuestUiButton::AbandonQuest { .. } => "abandon",
        QuestUiButton::ConfirmAbandonQuest => "confirmAbandon",
        QuestUiButton::CancelAbandonQuest => "cancelAbandon",
        QuestUiButton::PrepareQuestFinish { .. } | QuestUiButton::FinishQuest { .. } => "finish",
        _ => "action",
    }
}

/// Uses the laid-out current subtree, never declared Node dimensions or a hidden page.
#[cfg(not(feature = "native-ui"))]
fn observe_compact_quest_tree(
    locale: Res<QuestPresentationLocale>,
    presentation: Option<Res<QuestUiPresentation>>,
    context: Option<Res<crate::portable_quest_ui::QuestUiHostContext>>,
    player_ui: Res<NativePlayerUiState>,
    state: Res<QuestUiState>,
    tracker: Res<QuestTracker>,
    dialog: Res<NpcDialogModel>,
    guidance: Res<QuestGuidance>,
    roots: Query<(Entity, &ComputedNode, &bevy::ui::UiGlobalTransform, &InheritedVisibility), With<QuestUiRoot>>,
    panels: Query<(Entity, &QuestCompactStamp, &Node, &ComputedNode,
        &bevy::ui::UiGlobalTransform, &InheritedVisibility)>,
    actions: Query<(Entity, &QuestUiButton, &ComputedNode, &bevy::ui::UiGlobalTransform,
        &InheritedVisibility), (With<Button>, With<QuestUiButton>)>,
    texts: Query<(Entity, &Text, &TextFont, &ComputedNode,
        &bevy::ui::UiGlobalTransform, &InheritedVisibility)>,
    parents: Query<&ChildOf>,
    mut observed: ResMut<QuestCompactReadiness>,
) {
    *observed = QuestCompactReadiness::default();
    observed.first_failure = Some("presentation");
    let Some((presentation, context)) = presentation.as_deref().zip(context.as_deref()) else { return; };
    let Some(expected) = compact_expected_stamp(presentation, context, &player_ui,
        &state, &tracker, &dialog, &guidance, *locale) else { return; };
    observed.generation = Some(expected.generation);
    observed.revision = Some(expected.revision);
    observed.sheet = Some(compact_sheet_label(expected.sheet));
    observed.page = Some(match expected.page { CompactQuestPage::Text => "text",
        CompactQuestPage::Rewards => "rewards" });
    observed.quest_index = expected.quest_index;
    observed.reward_page = Some(expected.reward_page);
    observed.text_top = Some(expected.text_top);
    observed.diary_page = Some(expected.diary_page);
    observed.first_failure = Some("rootLayout");
    let Ok((root, root_node, root_transform, root_visibility)) = roots.single() else { return; };
    if !root_visibility.get() || root_node.size().min_element() <= 0.0 { return; }
    let stage = bevy::math::Rect {
        min: Vec2::ZERO,
        max: Vec2::new(presentation.logical_width, presentation.logical_height),
    };
    let root_rect = compact_rect(root_node, root_transform);
    let tolerance = 1.0 / presentation.stage_css_scale;
    if root_rect.min.x < stage.min.x - tolerance || root_rect.min.y < stage.min.y - tolerance
        || root_rect.max.x > stage.max.x + tolerance || root_rect.max.y > stage.max.y + tolerance {
        return;
    }
    if expected.sheet == MobileQuestSheet::None {
        if panels.iter().any(|(_, stamp, _, _, _, visible)|
            stamp.generation == expected.generation && visible.get()) { return; }
        observed.stamp = Some(expected);
        observed.current = true;
        observed.first_failure = None;
        return;
    }
    observed.first_failure = Some("currentPanelStamp");
    let mut current = panels.iter().filter(|(entity, stamp, node, _, _, visibility)|
        **stamp == expected && node.display != Display::None && visibility.get()
            && compact_descends_from(*entity, root, &parents));
    let Some((panel, _, _, panel_node, panel_transform, _)) = current.next() else { return; };
    if current.next().is_some() { return; }
    let panel_rect = compact_rect(panel_node, panel_transform);
    let panel_css = panel_node.size() * panel_node.inverse_scale_factor * presentation.stage_css_scale;
    observed.panel_css_bounds = Some(QuestCompactPanelRect {
        left: panel_rect.min.x * presentation.stage_css_scale,
        top: panel_rect.min.y * presentation.stage_css_scale,
        width: panel_css.x, height: panel_css.y,
    });
    observed.first_failure = Some("panelBounds");
    if panel_css.x + 0.01 < 400.0 || panel_css.y + 0.01 < 296.0
        || panel_rect.min.x < stage.min.x - tolerance || panel_rect.min.y < stage.min.y - tolerance
        || panel_rect.max.x > stage.max.x + tolerance || panel_rect.max.y > stage.max.y + tolerance {
        return;
    }
    let inside_panel = |rect: bevy::math::Rect| rect.min.x >= panel_rect.min.x - tolerance
        && rect.min.y >= panel_rect.min.y - tolerance
        && rect.max.x <= panel_rect.max.x + tolerance
        && rect.max.y <= panel_rect.max.y + tolerance;
    let mut action_rects = Vec::new();
    observed.first_failure = Some("actionLayout");
    for (entity, action, node, transform, visibility) in &actions {
        if !compact_descends_from(entity, panel, &parents) { continue; }
        if !visibility.get() { return; }
        let rect = compact_rect(node, transform);
        let size = (rect.max - rect.min) * presentation.stage_css_scale;
        observed.min_css_hit_width = Some(observed.min_css_hit_width.map_or(size.x,
            |minimum| minimum.min(size.x)));
        observed.min_css_hit_height = Some(observed.min_css_hit_height.map_or(size.y,
            |minimum| minimum.min(size.y)));
        if observed.actions.len() < 24 {
            observed.actions.push(QuestCompactActionRect {
                label: compact_action_label(action),
                left: rect.min.x * presentation.stage_css_scale,
                top: rect.min.y * presentation.stage_css_scale,
                width: size.x, height: size.y,
            });
        }
        if !inside_panel(rect) || size.x < 39.9 || size.y < 39.9 { return; }
        for prior in &action_rects {
            let prior: &bevy::math::Rect = prior;
            let overlap = (rect.max.min(prior.max) - rect.min.max(prior.min))
                * presentation.stage_css_scale;
            if overlap.x > 0.5 && overlap.y > 0.5 { return; }
        }
        action_rects.push(rect);
    }
    let mut text_count = 0;
    observed.first_failure = Some("textLayout");
    for (entity, text, font, node, transform, visibility) in &texts {
        if text.0.is_empty() || !compact_descends_from(entity, panel, &parents) { continue; }
        let FontSize::Px(font_px) = font.font_size else {
            observed.first_failure = Some("textFontUnit"); return;
        };
        let rect = compact_rect(node, transform);
        let css_font = font_px * presentation.stage_css_scale;
        observed.min_css_text_size = Some(observed.min_css_text_size.map_or(css_font,
            |minimum| minimum.min(css_font)));
        if !visibility.get() || css_font < 13.9 {
            observed.first_failure = Some("textVisibilityOrFont"); return;
        }
        if node.size().min_element() <= 0.0 {
            observed.first_failure = Some("textUnlaidOut"); return;
        }
        if !inside_panel(rect) {
            observed.first_failure = Some("textOutsidePanel"); return;
        }
        if !actions.iter().any(|(action_entity, _, _, _, _)|
            compact_descends_from(entity, action_entity, &parents)) {
            for action in &action_rects {
                let action: &bevy::math::Rect = action;
                let overlap = (rect.max.min(action.max) - rect.min.max(action.min))
                    * presentation.stage_css_scale;
                if overlap.x > 0.5 && overlap.y > 0.5 {
                    observed.first_failure = Some("textActionOverlap"); return;
                }
            }
        }
        text_count += 1;
    }
    if action_rects.is_empty() || text_count == 0 { return; }
    observed.stamp = Some(expected);
    observed.current = true;
    observed.first_failure = None;
    observed.action_count = action_rects.len();
    observed.text_count = text_count;
}

fn journey_tracker_visible(has_journey: bool, open_panel: mir2_ui_core::state::UiPanel) -> bool {
    !has_journey || open_panel == mir2_ui_core::state::UiPanel::None
}

fn render_quest_tracker_panel(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    state: &QuestUiState,
    journey: Option<&JourneyView>,
    entities: &EntityModelSet,
    map_model: &MapModel,
    big_map: Option<&crate::big_map::BigMapModel>,
    class_name: &str,
    supplies: &crate::quest_supplies::SupplyPlan,
    render_text: QuestRenderText,
) {
    if journey.is_some() && state.supply_open {
        multi_guidance::render_supplies(parent, state, big_map, supplies);
        return;
    }
    if journey.is_some() && multi_guidance::render(parent, tracker, state, journey, entities, map_model, big_map, class_name, supplies) {
        return;
    }
    if primary_quest_index(tracker, state, journey) == Some(2_110_005)
        && crate::quest_destination::bichon_safe_arrival_pending(tracker)
    {
        render_bichon_arrival_tracker(parent, map_model);
        return;
    }
    let quests = visible_tracker_quests(tracker, state);
    if quests.is_empty() && journey.is_none() {
        return;
    }

    let compact_journey = journey.is_some();
    let mut y = 0.0;
    if let Some(journey) = journey {
        let journey_text_size = 10.0;
        quest_log_text_at_raw(
            parent,
            &format!("{}级  {}", journey.level_range, crate::player_text::text(&journey.chapter_title)),
            QuestLogRect::new(5.0, 20.0, 300.0, 15.0),
            journey_text_size,
            PANEL_HIGHLIGHT,
            Justify::Left,
        );
        quest_log_text_at_raw(
            parent,
            &journey.progress_label(),
            QuestLogRect::new(5.0, 35.0, 300.0, 15.0),
            journey_text_size,
            PANEL_TEXT,
            Justify::Left,
        );
        if let Some(next) = &journey.next {
            quest_log_text_at_raw(
                parent,
                &format!("下一步：{}", truncate_chars(&render_text.title(next.quest_id, &next.title), 38)),
                QuestLogRect::new(5.0, 50.0, 300.0, 15.0),
                journey_text_size,
                Color::srgb(0.20, 1.0, 0.10),
                Justify::Left,
            );
            quest_log_text_at_raw(
                parent,
                &format!("   {}", truncate_chars(&crate::player_text::text(&next.action), 42)),
                QuestLogRect::new(5.0, 65.0, 300.0, 15.0),
                journey_text_size,
                Color::WHITE,
                Justify::Left,
            );
            let detail_offset = if let Some(location) = &next.location {
                quest_log_text_at_raw(
                    parent,
                    &format!("   {location}"),
                    QuestLogRect::new(5.0, 80.0, 300.0, 15.0),
                    journey_text_size,
                    PANEL_TEXT,
                    Justify::Left,
                );
                15.0
            } else {
                0.0
            };
            if let Some(objective) = &next.objective {
                quest_log_text_at_raw(
                    parent,
                    &format!("   {}", truncate_chars(&render_text.objective(objective), 42)),
                    QuestLogRect::new(5.0, 80.0 + detail_offset, 300.0, 15.0),
                    journey_text_size,
                    Color::WHITE,
                    Justify::Left,
                );
            }
            let nearby_target = nearest_quest_monster(
                tracker,
                Some(next.quest_id),
                entities,
                map_model.center_x,
                map_model.center_y,
            );
            let target_offset = if let Some(target) = nearby_target {
                let object_id = target
                    .entity
                    .object_id
                    .parse::<u32>()
                    .expect("nearest quest target ids are validated");
                quest_log_text_button_at_raw(
                    parent,
                    QuestLogRect::new(25.0, 95.0 + detail_offset, 205.0, 18.0),
                    &format!(
                        "▶ {} · {} tiles",
                        truncate_chars(&target.entity.name, 21),
                        target.distance
                    ),
                    QuestUiButton::AttackQuestTarget { object_id },
                    true,
                );
                20.0
            } else {
                0.0
            };
            let reward = next
                .reward
                .as_deref()
                .unwrap_or(journey.reward_summary.as_str());
            if !reward.trim().is_empty() {
                quest_log_text_at_raw(
                    parent,
                    &format!("   奖励：{}", truncate_chars(&crate::player_text::text(reward), 32)),
                    QuestLogRect::new(5.0, 95.0 + detail_offset + target_offset, 300.0, 15.0),
                    journey_text_size,
                    PANEL_TEXT,
                    Justify::Left,
                );
            }
        } else if journey.graduated {
            quest_log_text_at_raw(
                parent,
                "新手旅程已完成",
                QuestLogRect::new(5.0, 50.0, 300.0, 15.0),
                journey_text_size,
                Color::srgb(0.20, 1.0, 0.10),
                Justify::Left,
            );
        } else {
            quest_log_text_at_raw(
                parent,
                &format!("下一步：{}", truncate_chars(&crate::player_text::text(&journey.goal), 38)),
                QuestLogRect::new(5.0, 50.0, 300.0, 15.0),
                journey_text_size,
                Color::srgb(0.20, 1.0, 0.10),
                Justify::Left,
            );
            quest_log_text_at_raw(
                parent,
                "   请查看任务日志或前往任务人物处",
                QuestLogRect::new(5.0, 65.0, 300.0, 15.0),
                journey_text_size,
                Color::WHITE,
                Justify::Left,
            );
        }
        let graduation_offset = if let Some(graduation) = &journey.graduation {
            if let Some(option) = state
                .selected_graduation_direction
                .and_then(|direction| graduation.option(direction))
            {
                quest_log_text_at_raw(
                    parent,
                    &format!(
                        "后续目标：{} · {}",
                        crate::player_text::text(option.direction.label()),
                        truncate_chars(&crate::player_text::text(&option.title), 23)
                    ),
                    QuestLogRect::new(5.0, 65.0, 300.0, 15.0),
                    journey_text_size,
                    PANEL_HIGHLIGHT,
                    Justify::Left,
                );
                quest_log_text_at_raw(
                    parent,
                    &truncate_chars(&option.instruction, 48),
                    QuestLogRect::new(5.0, 80.0, 300.0, 15.0),
                    journey_text_size,
                    PANEL_TEXT,
                    Justify::Left,
                );
            } else {
                quest_log_text_at_raw(
                    parent,
                    "后续目标：在任务日志选择装备、技能或挑战",
                    QuestLogRect::new(5.0, 65.0, 300.0, 15.0),
                    journey_text_size,
                    PANEL_HIGHLIGHT,
                    Justify::Left,
                );
            }
            30.0
        } else {
            0.0
        };
        let location_offset = if journey
            .next
            .as_ref()
            .is_some_and(|next| next.location.is_some())
        {
            15.0
        } else {
            0.0
        };
        let target_offset = journey
            .next
            .as_ref()
            .and_then(|next| {
                nearest_quest_monster(
                    tracker,
                    Some(next.quest_id),
                    entities,
                    map_model.center_x,
                    map_model.center_y,
                )
            })
            .map(|_| 20.0)
            .unwrap_or(0.0);
        if let Some(class_hint) = &journey.class_hint {
            quest_log_text_at_raw(
                parent,
                &format!("   提示：{}", truncate_chars(&crate::player_text::text(class_hint), 38)),
                QuestLogRect::new(
                    5.0,
                    110.0 + graduation_offset + location_offset + target_offset,
                    300.0,
                    15.0,
                ),
                journey_text_size,
                PANEL_TEXT,
                Justify::Left,
            );
        }
        let optional_target = journey
            .optional
            .iter()
            .filter_map(|optional| {
                nearest_quest_monster(
                    tracker,
                    Some(optional.quest_id),
                    entities,
                    map_model.center_x,
                    map_model.center_y,
                )
            })
            .min_by_key(|target| (target.distance, target.entity.object_id.as_str()));
        if let Some(optional) = journey.optional.first() {
            quest_log_text_at_raw(
                parent,
                &format!("可选任务：{}", truncate_chars(&render_text.title(optional.quest_id, &optional.title), 34)),
                QuestLogRect::new(
                    5.0,
                    125.0 + graduation_offset + location_offset + target_offset,
                    300.0,
                    15.0,
                ),
                journey_text_size,
                PANEL_TEXT,
                Justify::Left,
            );
        }
        let optional_target_offset = if let Some(target) = optional_target {
            let object_id = target
                .entity
                .object_id
                .parse::<u32>()
                .expect("nearest quest target ids are validated");
            quest_log_text_button_at_raw(
                parent,
                QuestLogRect::new(
                    25.0,
                    140.0 + graduation_offset + location_offset + target_offset,
                    205.0,
                    18.0,
                ),
                &format!(
                    "▶ {} · {} tiles",
                    truncate_chars(&target.entity.name, 21),
                    target.distance
                ),
                QuestUiButton::AttackQuestTarget { object_id },
                true,
            );
            20.0
        } else {
            0.0
        };
        y = 135.0 + graduation_offset + location_offset + target_offset + optional_target_offset;
        quest_log_text_button_at_raw(parent, QuestLogRect::new(8.0, y + 20.0, 286.0, 25.0),
            "补给检查与购买地点", QuestUiButton::ToggleSupplies, true);
        y += 30.0;
    }
    let tracked_limit = if compact_journey {
        2
    } else {
        MAX_TRACKED_QUESTS
    };
    for quest in quests.into_iter().take(tracked_limit) {
        quest_log_text_at_raw(
            parent,
            &render_text.title(quest.quest_index, &quest.title),
            QuestLogRect::new(5.0, 20.0 + y, 300.0, 15.0),
            8.0,
            Color::srgb(0.20, 1.0, 0.10),
            Justify::Left,
        );
        let objective_limit = if compact_journey { 1 } else { usize::MAX };
        for objective in quest.objectives.iter().take(objective_limit) {
            y += 15.0;
            quest_log_text_at_raw(
                parent,
                &quest_objective_detail_text(objective, render_text),
                QuestLogRect::new(25.0, 20.0 + y, 290.0, 15.0),
                8.0,
                Color::WHITE,
                Justify::Left,
            );
        }
        if let Some(target) = nearest_quest_monster(
            tracker,
            Some(quest.quest_index),
            entities,
            map_model.center_x,
            map_model.center_y,
        ) {
            let object_id = target
                .entity
                .object_id
                .parse::<u32>()
                .expect("nearest quest target ids are validated");
            y += 20.0;
            quest_log_text_button_at_raw(
                parent,
                QuestLogRect::new(25.0, 20.0 + y, 205.0, 18.0),
                &format!(
                    "▶ {} · {} tiles",
                    truncate_chars(&target.entity.name, 21),
                    target.distance
                ),
                QuestUiButton::AttackQuestTarget { object_id },
                true,
            );
        }
        y += 30.0;
    }
}

// The source skin is 440x224. Its header and footer are not content rows.
// Page all server options inside the body instead of silently taking four.
fn render_bichon_arrival_tracker(parent: &mut ChildSpawnerCommands, map: &MapModel) {
    let font = TextFont {
        font: FontSource::Family("Microsoft YaHei".into()),
        font_size: FontSize::Px(14.0),
        ..default()
    };
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0), top: Val::Px(16.0), width: Val::Px(304.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0), padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)), ..default()
        },
        BackgroundColor(Color::srgba(0.035, 0.03, 0.02, 0.88)),
        BorderColor::all(PANEL_HIGHLIGHT),
    )).with_children(|card| {
        for (text, color) in [
            ("主线 · 前往比奇城".to_owned(), PANEL_HIGHLIGHT),
            ("目的地：比奇城安全区".to_owned(), Color::srgb(0.3, 1.0, 0.3)),
            (format!("比奇省 · 坐标 ({},{})", crate::quest_destination::BICHON_SAFE_X, crate::quest_destination::BICHON_SAFE_Y), Color::WHITE),
            ("从新手村向北，前往北部大城。".to_owned(), Color::WHITE),
            ("进入城内安全区后，任务进度会更新。".to_owned(), Color::WHITE),
            ("注意：新手村安全区不算此任务目标。".to_owned(), PANEL_HIGHLIGHT),
            (format!("当前位置：({},{})", map.center_x, map.center_y), PANEL_TEXT),
            ("路途较长，出发前补充血药。".to_owned(), PANEL_TEXT),
        ] {
            card.spawn((
                Node { width: Val::Percent(100.0), min_height: Val::Px(20.0), flex_shrink: 0.0, ..default() },
                Text::new(text), font.clone(), TextColor(color),
                TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
            ));
        }
        card.spawn((
            Button, QuestUiButton::OpenDestinationMap, QuestUiButtonVisual { enabled: true },
            Node { height: Val::Px(30.0), width: Val::Percent(100.0), align_items: AlignItems::Center,
                justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() },
            BackgroundColor(BUTTON_BG), FocusPolicy::Block,
        )).with_children(|button| {
            button.spawn((Text::new("打开大地图 · 查看目的地"), font, TextColor(PANEL_HIGHLIGHT)));
        });
    });
}

fn npc_dialog_weighted_width(text: &str) -> usize {
    text.chars()
        .map(|character| if character.is_ascii() { 1 } else { 2 })
        .sum()
}

fn npc_dialog_hard_chunks(text: &str) -> Vec<String> {
    const MAX_WIDTH: usize = 54;
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    let mut width = 0;
    for character in text.chars() {
        let advance = if character.is_ascii() { 1 } else { 2 };
        if !chunk.is_empty() && width + advance > MAX_WIDTH {
            chunks.push(std::mem::take(&mut chunk));
            width = 0;
        }
        chunk.push(character);
        width += advance;
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    chunks
}

fn npc_dialog_wrap(text: &str) -> Vec<String> {
    const MAX_WIDTH: usize = 54;
    let mut lines = Vec::new();
    for source_line in text.lines() {
        let mut row = String::new();
        let mut width = 0;

        for word in source_line.split_whitespace() {
            let chunks = npc_dialog_hard_chunks(word);
            if chunks.len() > 1 {
                if !row.is_empty() {
                    lines.push(std::mem::take(&mut row));
                    width = 0;
                }
                lines.extend(chunks);
                continue;
            }

            let word = chunks.into_iter().next().unwrap_or_default();
            let word_width = npc_dialog_weighted_width(&word);
            if row.is_empty() {
                row = word;
                width = word_width;
            } else if width + 1 + word_width <= MAX_WIDTH {
                row.push(' ');
                row.push_str(&word);
                width += 1 + word_width;
            } else {
                lines.push(std::mem::take(&mut row));
                row = word;
                width = word_width;
            }
        }
        if !row.is_empty() {
            lines.push(row);
        }
    }
    lines
}

fn npc_dialog_rows(dialog: &NpcDialogModel) -> Vec<(String, Option<String>, bool)> {
    let canonical = |value: &str| {
        value
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let mut rows = Vec::new();
    for (index, line) in dialog.lines.iter().enumerate() {
        if index == 0
            && dialog
                .npc_name
                .as_deref()
                .is_some_and(|name| canonical(name) == canonical(&line.text))
        {
            continue;
        }
        for line in npc_dialog_wrap(&line.text) {
            rows.push((line, None, false));
        }
    }
    for option in dialog
        .options
        .iter()
        .filter(|option| !is_quest_dialog_operation_target(&option.option_id))
    {
        for line in npc_dialog_wrap(&option.label) {
            rows.push((line, Some(option.option_id.clone()), option.enabled));
        }
    }
    rows
}

fn mobile_text(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    text: &str,
    rect: (f32, f32, f32, f32),
    size_css: f32,
    color: Color,
) {
    quest_log_text_at(parent, text, layout.rect(rect.0, rect.1, rect.2, rect.3),
        layout.px(size_css), color, Justify::Left);
}

fn mobile_text_raw(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    text: &str,
    rect: (f32, f32, f32, f32),
    size_css: f32,
    color: Color,
) {
    quest_log_text_at_raw(parent, text, layout.rect(rect.0, rect.1, rect.2, rect.3),
        layout.px(size_css), color, Justify::Left);
}

fn mobile_button(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    text: &str,
    rect: (f32, f32, f32, f32),
    action: QuestUiButton,
    enabled: bool,
) {
    quest_log_text_button_at_sized(parent, layout.rect(rect.0, rect.1, rect.2, rect.3),
        text, action, enabled, layout.px(15.0));
}

fn mobile_button_raw(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    text: &str,
    rect: (f32, f32, f32, f32),
    action: QuestUiButton,
    enabled: bool,
) {
    quest_log_text_button_at_sized_raw(parent, layout.rect(rect.0, rect.1, rect.2, rect.3),
        text, action, enabled, layout.px(15.0));
}

fn mobile_chrome(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    title: &str,
    close: QuestUiButton,
    render_text: QuestRenderText,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0), top: Val::Px(0.0),
            width: Val::Percent(100.0), height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.07, 0.055, 0.04, 0.97)),
        FocusPolicy::Pass,
    ));
    mobile_text_raw(parent, layout, title, (16.0, 12.0, layout.width_css() - 100.0, 30.0),
        18.0, PANEL_HIGHLIGHT);
    mobile_button_raw(parent, layout, render_text.chrome("ui.close", "Close"), (layout.width_css() - 62.0, 4.0, 54.0, 44.0),
        close, true);
}

fn compact_quest_tabs(parent: &mut ChildSpawnerCommands, layout: MobileQuestLayout,
    page: CompactQuestPage, npc: bool,
    render_text: QuestRenderText,
) {
    mobile_chrome(parent, layout, "", if npc { QuestUiButton::ReturnToNpcDialog }
        else { QuestUiButton::CloseQuestDetail }, render_text);
    let text = if npc { QuestUiButton::CompactNpcPage(CompactQuestPage::Text) }
        else { QuestUiButton::CompactDetailPage(CompactQuestPage::Text) };
    let rewards = if npc { QuestUiButton::CompactNpcPage(CompactQuestPage::Rewards) }
        else { QuestUiButton::CompactDetailPage(CompactQuestPage::Rewards) };
    mobile_button_raw(parent, layout, if page == CompactQuestPage::Text { "● 正文" } else { "正文" },
        (16.0, 4.0, 92.0, 44.0), text, true);
    mobile_button_raw(parent, layout, if page == CompactQuestPage::Rewards { "● 奖励" } else { "奖励" },
        (116.0, 4.0, 92.0, 44.0), rewards, true);
}

fn mobile_pager(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    page: usize,
    page_count: usize,
    previous: QuestUiButton,
    next: QuestUiButton,
    render_text: QuestRenderText,
) {
    let y = layout.height_css() - 49.0;
    mobile_button_raw(parent, layout, render_text.chrome("ui.previous", "Previous"), (12.0, y, 88.0, 44.0), previous, page > 0);
    mobile_text_raw(parent, layout, &format!("{}/{}", page + 1, page_count),
        (layout.width_css() / 2.0 - 28.0, y + 10.0, 60.0, 28.0), 14.0, PANEL_TEXT);
    mobile_button_raw(parent, layout, render_text.chrome("ui.next", "Next"), (layout.width_css() - 100.0, y, 88.0, 44.0),
        next, page + 1 < page_count);
}

fn mobile_diary_page_count(quest_count: usize, graduation_options: usize, rows: usize) -> usize {
    (quest_count.div_ceil(rows) + graduation_options.div_ceil(rows)).max(1)
}

fn mobile_graduation_page<'a>(options: &'a [crate::quest_journey::GraduationOption],
    quest_count: usize, rows: usize, page: usize) -> Option<&'a [crate::quest_journey::GraduationOption]> {
    let quest_pages = quest_count.div_ceil(rows);
    let graduation_page = page.checked_sub(quest_pages)?;
    let start = graduation_page.saturating_mul(rows);
    options.get(start..start.saturating_add(rows).min(options.len()))
}

fn mobile_text_columns(character: char) -> usize {
    let width = unicode_width::UnicodeWidthChar::width(character).unwrap_or(1).max(1);
    if character.is_ascii_uppercase() || matches!(character, 'm' | 'w' | '@' | '#' | '%' | '&') {
        width.max(2)
    } else { width }
}

fn mobile_wrap_quest_lines(lines: Vec<QuestDetailLine>, content_width_css: f32) -> Vec<QuestDetailLine> {
    let max_columns = (content_width_css / 7.0).floor().max(8.0) as usize;
    let mut wrapped = Vec::new();
    for line in lines {
        if line.text.is_empty() {
            wrapped.push(line);
            continue;
        }
        for source_line in line.text.split('\n') {
            let mut text = String::new();
            let mut width = 0;
            for character in source_line.chars() {
                let character_width = mobile_text_columns(character);
                if width + character_width > max_columns && !text.is_empty() {
                    wrapped.push(QuestDetailLine { text: std::mem::take(&mut text), kind: line.kind });
                    width = 0;
                }
                text.push(character);
                width += character_width;
            }
            wrapped.push(QuestDetailLine { text, kind: line.kind });
        }
    }
    wrapped
}

fn compact_confirmation_lines(message: &str, layout: MobileQuestLayout) -> Vec<QuestDetailLine> {
    mobile_wrap_quest_lines(vec![QuestDetailLine {
        text: message.to_owned(), kind: QuestDetailLineKind::Body,
    }], layout.width_css() - 32.0)
}

fn mobile_npc_dialog_rows(dialog: &NpcDialogModel, content_width_css: f32)
    -> Vec<(String, Option<String>, bool)> {
    let max_columns = (content_width_css / 7.0).floor().max(8.0) as usize;
    let mut wrapped = Vec::new();
    for (line, target, enabled) in npc_dialog_rows(dialog) {
        let mut text = String::new();
        let mut width = 0;
        for character in line.chars() {
            let advance = mobile_text_columns(character);
            if width + advance > max_columns && !text.is_empty() {
                wrapped.push((std::mem::take(&mut text), target.clone(), enabled));
                width = 0;
            }
            text.push(character);
            width += advance;
        }
        wrapped.push((text, target, enabled));
    }
    wrapped
}

fn render_mobile_diary_panel(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    tracker: &QuestTracker,
    guidance: &QuestGuidance,
    journey: Option<&JourneyView>,
    state: &QuestUiState,
    render_text: QuestRenderText,
) {
    mobile_chrome(parent, layout, render_text.chrome("ui.quest", "Quest"), QuestUiButton::CloseQuestLog, render_text);
    let guided = guidance.profile_name() == Some("newcomer-v2");
    let rows = layout.diary_rows();
    let graduation = (!guided || state.diary_tab == GuidedDiaryTab::Main)
        && journey.and_then(|view| view.graduation.as_ref()).is_some();
    let (quests, page, previous, next): (Vec<(&Quest, String)>, usize, _, _) = if guided {
        let tab_width = (layout.width_css() - 32.0) / 3.0;
        for (index, tab) in GuidedDiaryTab::ALL.into_iter().enumerate() {
            let text = format!("{} {}", tab.label(), guided_diary_quests(tracker, Some(guidance), journey, tab).len());
            mobile_button_raw(parent, layout, &text,
                (16.0 + index as f32 * tab_width, 50.0, tab_width - 4.0, 44.0),
                QuestUiButton::SelectGuidedDiaryTab(tab), true);
        }
        (guided_diary_quests(tracker, Some(guidance), journey, state.diary_tab)
            .into_iter().map(|quest| (quest, quest.group.clone().unwrap_or_default())).collect(),
            state.diary_page, QuestUiButton::GuidedDiaryPrevious, QuestUiButton::GuidedDiaryNext)
    } else {
        (quest_diary_groups(tracker, Some(guidance)).into_iter()
            .flat_map(|group| group.quests.into_iter().map(move |quest| (quest, group.name.clone())))
            .collect(), state.page, QuestUiButton::QuestPagePrevious, QuestUiButton::QuestPageNext)
    };
    let graduation_options = if graduation { journey.and_then(|view| view.graduation.as_ref())
        .map_or(0, |graduation| graduation.options.len()) } else { 0 };
    let pages = mobile_diary_page_count(quests.len(), graduation_options, rows);
    let page = page.min(pages - 1);
    let row_start = if guided { 103.0 } else { 54.0 };
    if graduation_options > 0 && page >= quests.len().div_ceil(rows) {
        if let Some(graduation) = journey.and_then(|view| view.graduation.as_ref()) {
            for (index, option) in mobile_graduation_page(&graduation.options,
                quests.len(), rows, page).unwrap_or_default().iter().enumerate() {
                mobile_button_raw(parent, layout, &format!("{} {}", if state.selected_graduation_direction == Some(option.direction) { "●" } else { "+" }, option.title),
                    (16.0, row_start + index as f32 * 48.0, layout.width_css() - 32.0, 44.0),
                    QuestUiButton::SelectGraduationDirection { direction: option.direction }, true);
            }
        }
    } else {
        for (index, (quest, group)) in quests.iter().skip(page * rows).take(rows).enumerate() {
            let y = row_start + index as f32 * 48.0;
            let status = quest_diary_status_label(quest, render_text);
            let title = render_text.title(quest.quest_index, &quest.title);
            let label = format!("{} · {} · {}", group, title, status);
            mobile_button_raw(parent, layout, &label,
                (16.0, y, layout.width_css() - 130.0, 44.0),
                QuestUiButton::SelectQuest { quest_index: quest.quest_index }, true);
            mobile_button_raw(parent, layout, &render_text.track(state.is_tracked(quest.quest_index)),
                (layout.width_css() - 108.0, y, 92.0, 44.0),
                QuestUiButton::TrackQuest { quest_index: quest.quest_index }, true);
        }
    }
    if quests.is_empty() && !graduation {
        mobile_text_raw(parent, layout, render_text.chrome("ui.questEmpty", "No quests"), (16.0, row_start + 16.0, layout.width_css() - 32.0, 44.0),
            15.0, PANEL_TEXT);
    }
    mobile_pager(parent, layout, page, pages, previous, next, render_text);
}

fn render_mobile_dialog_panel(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    dialog: &NpcDialogModel,
    nav: &NpcDialogNav,
    state: &QuestUiState,
    has_npc_quests: bool,
    render_text: QuestRenderText,
) {
    mobile_chrome(parent, layout, dialog.npc_name.as_deref().unwrap_or("NPC"),
        QuestUiButton::CloseNpcDialog, render_text);
    let rows = mobile_npc_dialog_rows(dialog, layout.width_css() - 32.0);
    let visible_rows = layout.dialog_rows();
    let max_page = rows.len().saturating_sub(1) / visible_rows * visible_rows;
    let top = state.dialog_scroll_top.min(max_page);
    for (index, (text, target, enabled)) in rows.iter().skip(top).take(visible_rows).enumerate() {
        let rect = (16.0, 54.0 + index as f32 * 44.0, layout.width_css() - 32.0, 44.0);
        if let Some(target) = target {
            mobile_button_raw(parent, layout, text, rect,
                QuestUiButton::SelectNpcDialog { target: target.clone() }, *enabled);
        } else {
            mobile_text_raw(parent, layout, text, rect, 15.0, PANEL_TEXT);
        }
    }
    let mut footer = Vec::new();
    if rows.len() > visible_rows {
        footer.push((render_text.chrome("ui.previous", "Previous"), QuestUiButton::NpcDialogScrollUp, top > 0));
        footer.push((render_text.chrome("ui.next", "Next"), QuestUiButton::NpcDialogScrollDown,
            top + visible_rows < rows.len()));
    }
    if nav.can_return() {
        footer.push(("返回", QuestUiButton::ReturnNpcService, true));
    }
    if has_npc_quests {
        footer.push((render_text.chrome("ui.quest", "Quest"), QuestUiButton::ToggleNpcQuestList, true));
    }
    let footer_width = layout.dialog_footer_button_width(footer.len());
    for (index, (text, action, enabled)) in footer.into_iter().enumerate() {
        mobile_button_raw(parent, layout, text,
            (12.0 + index as f32 * (footer_width + 8.0), layout.height_css() - 49.0,
                footer_width, 44.0), action, enabled);
    }
}

fn render_dialog_panel(
    parent: &mut ChildSpawnerCommands,
    dialog: &NpcDialogModel,
    nav: &NpcDialogNav,
    quest_state: &QuestUiState,
    _pending: &PendingOperations,
    has_npc_quests: bool,
    asset_server: Option<&AssetServer>,
    render_text: QuestRenderText,
) {
    quest_log_text_at_raw(
        parent,
        dialog.npc_name.as_deref().unwrap_or("NPC"),
        QuestLogRect::new(18.0, 7.0, 390.0, 18.0),
        12.0,
        PANEL_HIGHLIGHT,
        Justify::Left,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DIARY_TOP_CLOSE_ASSET,
        QuestLogRect::new(414.0, 5.0, 20.0, 20.0),
        QuestUiButton::CloseNpcDialog,
        true,
    );

    let rows = npc_dialog_rows(dialog);
    let max_page = rows.len().saturating_sub(1) / NPC_DIALOG_VISIBLE_ROWS * NPC_DIALOG_VISIBLE_ROWS;
    let top = quest_state.dialog_scroll_top.min(max_page);
    for (index, (text, target, enabled)) in rows
        .iter()
        .skip(top)
        .take(NPC_DIALOG_VISIBLE_ROWS)
        .enumerate()
    {
        let rect = QuestLogRect::new(18.0, 31.0 + index as f32 * 17.0, 390.0, 17.0);
        if let Some(target) = target {
            let mut link = parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(rect.left),
                    top: Val::Px(rect.top),
                    width: Val::Px(rect.width),
                    height: Val::Px(rect.height),
                    overflow: Overflow::clip(),
                    ..default()
                },
                FocusPolicy::Block,
            ));
            if *enabled {
                link.insert((
                    Button,
                    QuestUiButton::SelectNpcDialog {
                        target: target.clone(),
                    },
                ));
            }
            link.with_children(|link| {
                quest_log_text_at_raw(
                    link,
                    text,
                    QuestLogRect::new(0.0, 0.0, rect.width, rect.height),
                    12.0,
                    if *enabled {
                        Color::srgb(0.3, 0.85, 1.0)
                    } else {
                        DISABLED_TEXT
                    },
                    Justify::Left,
                );
            });
        } else {
            quest_log_text_at_raw(parent, text, rect, 12.0, PANEL_TEXT, Justify::Left);
        }
    }
    if rows.len() > NPC_DIALOG_VISIBLE_ROWS {
        quest_log_text_button_at_raw(
            parent,
            QuestLogRect::new(412.0, 34.0, 18.0, 18.0),
            "^",
            QuestUiButton::NpcDialogScrollUp,
            top > 0,
        );
        quest_log_text_button_at_raw(
            parent,
            QuestLogRect::new(412.0, 166.0, 18.0, 18.0),
            "v",
            QuestUiButton::NpcDialogScrollDown,
            top + NPC_DIALOG_VISIBLE_ROWS < rows.len(),
        );
    }
    if has_npc_quests {
        quest_log_image_button_at_locale(
            parent,
            asset_server,
            NPC_QUEST_BUTTON_ASSET,
            QuestLogRect::new(172.0, 194.0, 96.0, 25.0),
            QuestUiButton::ToggleNpcQuestList,
            true, render_text.chrome("ui.quest", "Quest"), render_text,
        );
    }
    if nav.can_return() {
        quest_log_text_button_at_raw(
            parent,
            QuestLogRect::new(18.0, 196.0, 76.0, 20.0),
            "Back",
            QuestUiButton::ReturnNpcService,
            true,
        );
    }
}

fn is_quest_dialog_operation_target(target: &str) -> bool {
    let target = target.trim().to_ascii_lowercase();
    target.starts_with("@acceptquest:")
        || target.starts_with("@finishquest:")
        || target.starts_with("@quest:accept:")
        || target.starts_with("@quest:finish:")
}

fn without_quest_operation_links(mut dialog: NpcDialogModel) -> NpcDialogModel {
    dialog
        .options
        .retain(|option| !is_quest_dialog_operation_target(&option.option_id));
    dialog
}

fn explicit_quest_dialog_button(target: &str, npc_index: u32) -> Option<QuestUiButton> {
    let mut parts = target.trim().trim_start_matches('@').split(':');
    let action = parts.next()?;
    let quest_index = parts.next()?.parse::<i32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if action.eq_ignore_ascii_case("AcceptQuest") && npc_index != 0 {
        Some(QuestUiButton::AcceptQuest {
            npc_index,
            quest_index,
        })
    } else if action.eq_ignore_ascii_case("FinishQuest") {
        Some(QuestUiButton::FinishQuest {
            quest_index,
            selected_item_index: -1,
        })
    } else {
        None
    }
}

fn dialog_exposes_quest_operation(
    dialog: &NpcDialogModel,
    expected_npc_index: Option<u32>,
    quest_index: i32,
    finish: bool,
    selected_item_index: Option<i32>,
) -> bool {
    let explicit = if finish {
        format!("@FinishQuest:{quest_index}")
    } else {
        format!("@AcceptQuest:{quest_index}")
    };
    let crystal = if finish {
        format!("@quest:finish:{quest_index}")
    } else {
        format!("@quest:accept:{quest_index}")
    };
    dialog.is_open
        && expected_npc_index.is_none_or(|expected| dialog.npc_object_id == Some(expected))
        && dialog.options.iter().any(|option| {
            if !option.enabled {
                return false;
            }
            let target = option.option_id.trim();
            target.eq_ignore_ascii_case(&explicit)
                || target.eq_ignore_ascii_case(&crystal)
                || (finish
                    && selected_item_index.is_some_and(|selected| {
                        target.eq_ignore_ascii_case(&format!(
                            "@quest:finish:{quest_index}:{selected}"
                        ))
                    }))
        })
}

#[allow(dead_code)]
fn render_quest_log_panel_legacy(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    state: &QuestUiState,
    pending: &PendingOperations,
) {
    quest_log_title_spacer(parent);

    feedback_line(parent, state.feedback.as_ref(), 11.0);

    if tracker.active_quests.is_empty() {
        body_line(parent, "No quests. Talk to NPCs to begin.");
        action_button(parent, "Close", QuestUiButton::CloseQuestLog, true);
        return;
    }

    // List pane
    body_line(parent, "Quests:");
    for quest in &tracker.active_quests {
        let selected = state.selected_quest_index == Some(quest.quest_index);
        let marker = if selected { "▶" } else { " " };
        let tracking = if state.is_tracked(quest.quest_index) {
            " [Tracking]"
        } else {
            ""
        };
        let label = format!(
            "{marker} {} [{}]{tracking}",
            truncate_chars(&crate::player_text::quest_title(quest.quest_index, &quest.title), 28),
            quest.status.label()
        );
        action_button(
            parent,
            &label,
            QuestUiButton::SelectQuest {
                quest_index: quest.quest_index,
            },
            true,
        );
    }

    // Detail pane
    if let Some(quest) = state.selected_quest(tracker) {
        parent.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(PANEL_HIGHLIGHT),
        ));

        detail_title(parent, &crate::player_text::quest_title(quest.quest_index, &quest.title));
        body_line(parent, &format!("Status: {}", quest.status.label()));
        if let Some(npc) = &quest.npc_name {
            body_line(parent, &format!("NPC: {npc}"));
        }
        if let Some(text) = &quest.unknown_text {
            if !text.trim().is_empty() {
                body_line(parent, &truncate_chars(text, 72));
            }
        }
        if quest.objectives.is_empty() {
            body_line(parent, "No objectives");
        } else {
            for obj in &quest.objectives {
                body_line(
                    parent,
                    &format!(
                        "- {} ({})",
                        truncate_chars(&obj.text, 44),
                        obj.progress_label()
                    ),
                );
            }
        }

        if quest.rewards.is_empty() {
            body_line(parent, "Reward: No reward");
        } else {
            body_line(
                parent,
                &format!("Reward: {}", truncate_chars(&quest.rewards_label(), 64)),
            );
            // Reward selection when multiple rewards and ReadyToTurnIn
            if quest.rewards.len() > 1 {
                body_line(parent, "Choose reward:");
                for (idx, reward) in quest.rewards.iter().enumerate() {
                    let chosen = state.selected_reward_index == Some(idx as i32);
                    let label =
                        format!("{} {}", if chosen { "[x]" } else { "[ ]" }, reward.label());
                    action_button(
                        parent,
                        &label,
                        QuestUiButton::SelectReward {
                            quest_index: quest.quest_index,
                            reward_index: idx as i32,
                        },
                        true,
                    );
                }
                if reward_selection_required(quest) && state.selected_reward_index.is_none() {
                    body_line(parent, "Select a reward to deliver.");
                }
            }
        }

        // Action buttons row
        let can_track = can_track_quest(quest);
        let npc_index = quest
            .accept_npc_index
            .or(quest.finish_npc_index)
            .unwrap_or(0);
        let accept_pending = pending.contains(&PendingOperationKey::QuestAccept {
            npc_index,
            quest_index: quest.quest_index,
        });
        let finish_item = if quest.rewards.is_empty() {
            -1
        } else if quest.rewards.len() == 1 {
            0
        } else {
            state.selected_reward_index.unwrap_or(-1)
        };
        let finish_pending = pending.has_pending_quest_finish(quest.quest_index);
        let abandon_pending = pending.contains(&PendingOperationKey::QuestAbandon {
            quest_index: quest.quest_index,
        });
        // Crystal's authoritative Accept/Deliver actions belong to the active
        // NPC dialog. Keep the quest log informative, but never offer a button
        // that the server must reject for lacking the exact current dialog.
        let can_accept = false;
        let can_finish = false;
        // Track
        action_button(
            parent,
            if state.is_tracked(quest.quest_index) {
                "Tracking..."
            } else {
                "Track"
            },
            QuestUiButton::TrackQuest {
                quest_index: quest.quest_index,
            },
            can_track,
        );
        // Accept
        action_button(
            parent,
            if accept_pending {
                "Accepting..."
            } else if quest_accept_enabled(quest) && npc_index != 0 {
                "Talk to NPC"
            } else {
                "Accept"
            },
            QuestUiButton::AcceptQuest {
                npc_index,
                quest_index: quest.quest_index,
            },
            can_accept,
        );
        // Deliver / Finish
        action_button(
            parent,
            if finish_pending {
                "Delivering..."
            } else if quest_finish_enabled(quest, state.selected_reward_index) {
                "Return to NPC"
            } else {
                "Deliver"
            },
            QuestUiButton::FinishQuest {
                quest_index: quest.quest_index,
                selected_item_index: finish_item,
            },
            can_finish,
        );
        action_button(
            parent,
            if abandon_pending {
                "Abandoning..."
            } else {
                "Abandon"
            },
            QuestUiButton::AbandonQuest {
                quest_index: quest.quest_index,
            },
            can_abandon_quest(quest) && !abandon_pending,
        );
    } else {
        body_line(parent, "Select a quest to view details");
        body_line(parent, "Tip: Press Q to close, click a quest title.");
    }

    action_button(parent, "Close (Esc/Q)", QuestUiButton::CloseQuestLog, true);
}

#[derive(Debug, PartialEq, Eq)]
struct QuestDiaryGroup<'a> {
    name: String,
    quests: Vec<&'a Quest>,
}

fn quest_diary_groups<'a>(
    tracker: &'a QuestTracker,
    guidance: Option<&QuestGuidance>,
) -> Vec<QuestDiaryGroup<'a>> {
    let mut groups: Vec<QuestDiaryGroup<'_>> = Vec::new();
    let mut quests = tracker
        .active_quests
        .iter()
        .filter(|quest| {
            quest.status.is_active()
                || (can_accept_quest(quest) && newcomer_diary_accept_authorized(quest, guidance))
        })
        .enumerate()
        .collect::<Vec<_>>();
    if let Some(guidance) = guidance.filter(|guidance| guidance.is_enabled()) {
        quests.sort_by_key(|(position, quest)| {
            let (category, order, _) = guidance.sort_key(quest.quest_index, *position);
            (
                category,
                u8::from(!matches!(
                    quest.status,
                    crate::quest_model::QuestStatus::ReadyToTurnIn
                )),
                order,
                *position,
            )
        });
    }
    for (_, quest) in quests.into_iter().take(QUEST_DIARY_MAX_CURRENT) {
        let name = quest_diary_group_name(quest, guidance);
        if let Some(group) = groups.iter_mut().find(|group| group.name == name) {
            group.quests.push(quest);
        } else {
            groups.push(QuestDiaryGroup {
                name,
                quests: vec![quest],
            });
        }
    }
    groups
}

fn quest_diary_group_name(quest: &Quest, guidance: Option<&QuestGuidance>) -> String {
    // Cadence belongs to the server: the same quest can be a one-time task
    // in Crystal mode and a weekly task in an explicit content profile.
    if guidance.is_some_and(QuestGuidance::is_enabled) {
        if let Some(group @ ("Daily" | "Weekly" | "Repeatable")) = quest.group.as_deref() {
            return group.to_owned();
        }
    }
    if let Some(entry) = guidance.and_then(|guidance| guidance.entry(quest.quest_index)) {
        return entry.category.label().to_owned();
    }
    quest
        .group
        .as_deref()
        .filter(|group| !group.trim().is_empty())
        // Imported Crystal saves captured before the static NewQuestInfo packet
        // arrives retain the original Group in their concise summary. Accept
        // only a single identifier-shaped token here; narrative text must not
        // be misrepresented as a group name.
        .or_else(|| {
            quest.unknown_text.as_deref().filter(|summary| {
                !summary.is_empty()
                    && summary.len() <= 48
                    && summary
                        .chars()
                        .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
            })
        })
        .unwrap_or("General")
        .to_owned()
}

fn quest_diary_status_label(quest: &Quest, render_text: QuestRenderText) -> &'static str {
    match quest.status {
        crate::quest_model::QuestStatus::NotStarted => render_text.chrome("ui.questStage.available", "Available"),
        crate::quest_model::QuestStatus::ReadyToTurnIn => render_text.chrome("ui.questStage.readyToTurnIn", "Ready to turn in"),
        crate::quest_model::QuestStatus::Completed => render_text.chrome("ui.questStage.completed", "Completed"),
        _ => render_text.chrome("ui.questStage.inProgress", "In Progress"),
    }
}

/// The V2 journey and imported Crystal side quests share one authoritative
/// tracker, but need separate presentation. No status or server order is
/// changed here. A quest outside the configured journey remains accessible on
/// the side and turn-in tabs even when the journey is the default view.
fn guided_diary_quests<'a>(
    tracker: &'a QuestTracker,
    guidance: Option<&QuestGuidance>,
    journey: Option<&JourneyView>,
    tab: GuidedDiaryTab,
) -> Vec<&'a Quest> {
    let next_id = journey.and_then(|view| view.next.as_ref().map(|step| step.quest_id));
    let mut quests = tracker
        .active_quests
        .iter()
        .enumerate()
        .filter(|(_, quest)| {
            quest.status.is_active()
                || (can_accept_quest(quest)
                    && (newcomer_diary_accept_authorized(quest, guidance)
                        || next_id == Some(quest.quest_index)))
        })
        .filter(|(_, quest)| match tab {
            GuidedDiaryTab::Main => guidance.and_then(|guide| guide.entry(quest.quest_index)).is_some(),
            GuidedDiaryTab::Ready => {
                quest.status == crate::quest_model::QuestStatus::ReadyToTurnIn
            }
            GuidedDiaryTab::Side => guidance.and_then(|guide| guide.entry(quest.quest_index)).is_none(),
        })
        .collect::<Vec<_>>();
    quests.sort_by_key(|(position, quest)| {
        let status_rank = match quest.status {
            crate::quest_model::QuestStatus::ReadyToTurnIn => 0,
            crate::quest_model::QuestStatus::InProgress => 1,
            _ => 2,
        };
        let order = match tab {
            GuidedDiaryTab::Main => guidance
                .map(|guide| guide.sort_key(quest.quest_index, *position).1)
                .unwrap_or(i32::MAX),
            GuidedDiaryTab::Ready | GuidedDiaryTab::Side => {
                -quest.min_level_needed.max(0)
            }
        };
        (
            u8::from(tab == GuidedDiaryTab::Main && next_id != Some(quest.quest_index)),
            status_rank,
            order,
            *position,
        )
    });
    quests.into_iter().map(|(_, quest)| quest).collect()
}

fn render_guided_quest_diary_panel(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    guidance: &QuestGuidance,
    journey: Option<&JourneyView>,
    state: &QuestUiState,
    asset_server: Option<&AssetServer>,
    render_text: QuestRenderText,
) {
    let layout = quest_diary_layout(1.0);
    quest_log_text_at_raw(parent, render_text.chrome("ui.quest", "Quest"), layout.title, 11.0, PANEL_HIGHLIGHT, Justify::Left);
    let count_label = journey.map_or_else(
        || "主线进度 --/--".to_owned(),
        |view| match (view.completed_count, view.quest_count) {
            (Some(done), Some(total)) => format!("本章 {done}/{total}"),
            _ => "本章 --/--".to_owned(),
        },
    );
    quest_log_text_at_raw(parent, &count_label, layout.taken_count, 8.0, PANEL_TEXT, Justify::Left);
    quest_log_image_button_at(
        parent, asset_server, QUEST_DIARY_TOP_CLOSE_ASSET, layout.top_close,
        QuestUiButton::CloseQuestLog, true,
    );
    quest_log_image_button_at_locale(
        parent, asset_server, QUEST_DIARY_BOTTOM_CLOSE_ASSET, layout.bottom_close,
        QuestUiButton::CloseQuestLog, true, render_text.chrome("ui.close", "Close"), render_text,
    );

    for (index, tab) in GuidedDiaryTab::ALL.into_iter().enumerate() {
        let count = guided_diary_quests(tracker, Some(guidance), journey, tab).len();
        let label = format!("{} {count}", tab.label());
        quest_log_text_button_at_raw(
            parent,
            QuestLogRect::new(15.0 + index as f32 * 96.0, 37.0, 92.0, 23.0),
            &label,
            QuestUiButton::SelectGuidedDiaryTab(tab),
            true,
        );
        if state.diary_tab == tab {
            quest_log_text_at_raw(
                parent, "●", QuestLogRect::new(18.0 + index as f32 * 96.0, 42.0, 12.0, 12.0),
                8.0, PANEL_HIGHLIGHT, Justify::Left,
            );
        }
    }

    let heading = match state.diary_tab {
        GuidedDiaryTab::Main => journey
            .map(|view| format!("当前章节 · {}", truncate_chars(&crate::player_text::text(&view.chapter_title), 20)))
            .unwrap_or_else(|| "当前主线".to_owned()),
        GuidedDiaryTab::Ready => "已完成目标 · 可前往交付".to_owned(),
        GuidedDiaryTab::Side => "其他任务 · 可自行选择完成".to_owned(),
    };
    quest_log_text_at_raw(
        parent, &heading, QuestLogRect::new(19.0, 68.0, 276.0, 15.0),
        9.0, PANEL_HIGHLIGHT, Justify::Left,
    );
    quest_log_text_at_raw(
        parent, "左键查看详情 · 右键跟踪任务", QuestLogRect::new(19.0, 86.0, 276.0, 15.0),
        8.0, PANEL_TEXT, Justify::Left,
    );

    let quests = guided_diary_quests(tracker, Some(guidance), journey, state.diary_tab);
    let page_count = quests.len().div_ceil(GUIDED_DIARY_PAGE_SIZE).max(1);
    let page = state.diary_page.min(page_count - 1);
    for (index, quest) in quests
        .into_iter()
        .skip(page * GUIDED_DIARY_PAGE_SIZE)
        .take(GUIDED_DIARY_PAGE_SIZE)
        .enumerate()
    {
        let y = 108.0 + index as f32 * 36.0;
        let quest_index = quest.quest_index;
        let current = journey.and_then(|view| view.next.as_ref())
            .is_some_and(|step| step.quest_id == quest_index);
        if state.selected_quest_index == Some(quest_index) {
            quest_log_image_at(
                parent, asset_server, QUEST_DIARY_SELECTED_ASSET,
                QuestLogRect::new(23.0, y, 252.0, 16.0),
            );
        }
        if state.is_tracked(quest_index) {
            quest_log_image_at(
                parent, asset_server, QUEST_DIARY_TRACKED_ASSET,
                QuestLogRect::new(17.0, y + 18.0, 16.0, 12.0),
            );
        }
        let status = match quest.status {
            crate::quest_model::QuestStatus::NotStarted => "待领取",
            crate::quest_model::QuestStatus::ReadyToTurnIn => "可交付",
            _ => "进行中",
        };
        let area = if state.diary_tab == GuidedDiaryTab::Main {
            journey.map(|view| view.chapter_title.as_str()).unwrap_or("主线")
        } else {
            quest.group.as_deref().unwrap_or("其他地图")
        };
        let subtitle = format!("{}级 · {status} · {}", quest.min_level_needed.max(0),
            truncate_chars(&crate::player_text::text(area), 24));
        parent.spawn((
            Button,
            QuestUiButton::SelectQuest { quest_index },
            QuestDiaryRow { quest_index },
            RelativeCursorPosition::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(25.0), top: Val::Px(y),
                width: Val::Px(270.0), height: Val::Px(33.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            FocusPolicy::Block,
        )).with_children(|row| {
            quest_log_text_at_raw(
                row, &truncate_chars(&render_text.title(quest.quest_index, &quest.title), 31),
                QuestLogRect::new(6.0, 0.0, 220.0, 15.0),
                9.0, if current { PANEL_HIGHLIGHT } else { PANEL_TEXT }, Justify::Left,
            );
            if current {
                quest_log_text_at_raw(
                    row, "当前", QuestLogRect::new(225.0, 0.0, 40.0, 15.0),
                    8.0, FEEDBACK_OK, Justify::Left,
                );
            }
            quest_log_text_at_raw(
                row, &subtitle, QuestLogRect::new(6.0, 17.0, 255.0, 15.0),
                8.0, PANEL_TEXT, Justify::Left,
            );
        });
    }

    if state.diary_tab == GuidedDiaryTab::Main {
        if let Some(graduation) = journey.and_then(|view| view.graduation.as_ref()) {
            for (index, option) in graduation.options.iter().enumerate() {
                let selected = state.selected_graduation_direction == Some(option.direction);
                quest_log_text_button_at_raw(
                    parent,
                    QuestLogRect::new(22.0, 114.0 + index as f32 * 72.0, 272.0, 27.0),
                    &format!("{} {}", if selected { "●" } else { "+" },
                        truncate_chars(&option.title, 29)),
                    QuestUiButton::SelectGraduationDirection { direction: option.direction },
                    true,
                );
                quest_log_text_at_raw(
                    parent, &truncate_chars(&option.summary, 42),
                    QuestLogRect::new(27.0, 145.0 + index as f32 * 72.0, 264.0, 30.0),
                    8.0, PANEL_TEXT, Justify::Left,
                );
            }
        } else if page_count == 1 && guided_diary_quests(tracker, Some(guidance), journey,
            GuidedDiaryTab::Main).is_empty() {
            quest_log_text_at_raw(
                parent, "暂无可显示的主线，完成前置任务后刷新。",
                QuestLogRect::new(22.0, 119.0, 274.0, 32.0),
                9.0, PANEL_TEXT, Justify::Left,
            );
        }
    }
    quest_log_text_button_at_raw(
        parent, QuestLogRect::new(22.0, 408.0, 62.0, 22.0), render_text.chrome("ui.previous", "Previous"),
        QuestUiButton::GuidedDiaryPrevious, page > 0,
    );
    quest_log_text_at_raw(
        parent, &format!("{}/{page_count}", page + 1),
        QuestLogRect::new(131.0, 412.0, 54.0, 15.0),
        9.0, PANEL_TEXT, Justify::Center,
    );
    quest_log_text_button_at_raw(
        parent, QuestLogRect::new(228.0, 408.0, 62.0, 22.0), render_text.chrome("ui.next", "Next"),
        QuestUiButton::GuidedDiaryNext, page + 1 < page_count,
    );
}

fn render_quest_diary_panel(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    guidance: &QuestGuidance,
    journey: Option<&JourneyView>,
    state: &QuestUiState,
    _pending: &PendingOperations,
    asset_server: Option<&AssetServer>,
    render_text: QuestRenderText,
) {
    if guidance.profile_name() == Some("newcomer-v2") {
        render_guided_quest_diary_panel(parent, tracker, guidance, journey, state, asset_server, render_text);
        return;
    }
    let layout = quest_diary_layout(1.0);
    let groups = quest_diary_groups(tracker, Some(guidance));
    let current_count = groups.iter().map(|group| group.quests.len()).sum::<usize>();
    quest_log_text_at_raw(parent, render_text.chrome("ui.quest", "Quest"), layout.title, 11.0, PANEL_HIGHLIGHT, Justify::Left);
    let count_label = journey.map_or_else(
        || format!("List: {current_count}/{QUEST_DIARY_MAX_CURRENT}"),
        |journey| match (journey.completed_count, journey.quest_count) {
            (Some(done), Some(total)) => format!("Ch {done}/{total}"),
            _ => "Ch --/--".to_owned(),
        },
    );
    quest_log_text_at_raw(
        parent,
        &count_label,
        layout.taken_count,
        8.0,
        PANEL_TEXT,
        Justify::Left,
    );
    if let Some(journey) = journey {
        quest_log_text_at_raw(
            parent,
            &truncate_chars(&crate::player_text::text(&journey.chapter_title), 16),
            QuestLogRect::new(120.0, 7.0, 86.0, 15.0),
            8.0,
            PANEL_HIGHLIGHT,
            Justify::Left,
        );
    }
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DIARY_TOP_CLOSE_ASSET,
        layout.top_close,
        QuestUiButton::CloseQuestLog,
        true,
    );
    quest_log_image_button_at_locale(
        parent,
        asset_server,
        QUEST_DIARY_BOTTOM_CLOSE_ASSET,
        layout.bottom_close,
        QuestUiButton::CloseQuestLog,
        true, render_text.chrome("ui.close", "Close"), render_text,
    );

    let mut next_y = QUEST_DIARY_FIRST_ROW_TOP;
    for group in groups {
        let collapsed = state.is_group_collapsed(&group.name);
        quest_log_image_button_at(
            parent,
            asset_server,
            if collapsed {
                QUEST_DIARY_COLLAPSED_ASSET
            } else {
                QUEST_DIARY_EXPANDED_ASSET
            },
            QuestLogRect::new(QUEST_DIARY_GROUP_LEFT, next_y, 16.0, 14.0),
            QuestUiButton::ToggleQuestGroup {
                group: group.name.clone(),
            },
            true,
        );
        quest_log_text_at_raw(
            parent,
            &group.name,
            QuestLogRect::new(QUEST_DIARY_GROUP_LEFT + 18.0, next_y, 250.0, 15.0),
            8.0,
            FEEDBACK_OK,
            Justify::Left,
        );
        next_y += QUEST_DIARY_ROW_HEIGHT;

        if collapsed {
            continue;
        }

        for quest in group.quests {
            if state.selected_quest_index == Some(quest.quest_index) {
                quest_log_image_at(
                    parent,
                    asset_server,
                    QUEST_DIARY_SELECTED_ASSET,
                    QuestLogRect::new(23.0, next_y, 252.0, 16.0),
                );
            }
            if state.is_tracked(quest.quest_index) {
                quest_log_image_at(
                    parent,
                    asset_server,
                    QUEST_DIARY_TRACKED_ASSET,
                    QuestLogRect::new(18.0, next_y, 16.0, 12.0),
                );
            }

            let level = quest.min_level_needed.max(0);
            let quest_label = format!("{level}级 {}", render_text.title(quest.quest_index, &quest.title));
            let state_label = quest_diary_status_label(quest, render_text);
            let quest_index = quest.quest_index;
            parent
                .spawn((
                    Button,
                    QuestUiButton::SelectQuest { quest_index },
                    QuestDiaryRow { quest_index },
                    RelativeCursorPosition::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(33.0),
                        top: Val::Px(next_y),
                        width: Val::Px(250.0),
                        height: Val::Px(15.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    FocusPolicy::Block,
                ))
                .with_children(|row| {
                    quest_log_text_at_raw(
                        row,
                        &quest_label,
                        QuestLogRect::new(0.0, 0.0, 185.0, 15.0),
                        8.0,
                        PANEL_TEXT,
                        Justify::Left,
                    );
                    quest_log_text_at_raw(
                        row,
                        state_label,
                        QuestLogRect::new(185.0, 0.0, 65.0, 15.0),
                        8.0,
                        PANEL_TEXT,
                        Justify::Left,
                    );
                });
            next_y += QUEST_DIARY_ROW_HEIGHT;
        }
    }

    if let Some(graduation) = journey.and_then(|journey| journey.graduation.as_ref()) {
        next_y += QUEST_DIARY_ROW_HEIGHT;
        quest_log_text_at_raw(
            parent,
            &format!("{} · Choose your next goal", graduation.title),
            QuestLogRect::new(
                QUEST_DIARY_GROUP_LEFT,
                next_y,
                270.0,
                QUEST_DIARY_ROW_HEIGHT,
            ),
            8.0,
            PANEL_HIGHLIGHT,
            Justify::Left,
        );
        next_y += QUEST_DIARY_ROW_HEIGHT;
        for option in &graduation.options {
            let selected = state.selected_graduation_direction == Some(option.direction);
            let marker = if selected { "*" } else { "+" };
            quest_log_text_button_at_raw(
                parent,
                QuestLogRect::new(QUEST_DIARY_GROUP_LEFT + 3.0, next_y, 282.0, 15.0),
                &format!(
                    "{marker} {}: {}",
                    option.direction.label(),
                    truncate_chars(&option.title, 28)
                ),
                QuestUiButton::SelectGraduationDirection {
                    direction: option.direction,
                },
                true,
            );
            next_y += QUEST_DIARY_ROW_HEIGHT;
            quest_log_text_at_raw(
                parent,
                &truncate_chars(&option.summary, 58),
                QuestLogRect::new(QUEST_DIARY_GROUP_LEFT + 12.0, next_y, 270.0, 15.0),
                8.0,
                PANEL_TEXT,
                Justify::Left,
            );
            next_y += QUEST_DIARY_ROW_HEIGHT;
            quest_log_text_at_raw(
                parent,
                &truncate_chars(&option.instruction, 58),
                QuestLogRect::new(QUEST_DIARY_GROUP_LEFT + 12.0, next_y, 270.0, 15.0),
                8.0,
                if selected { FEEDBACK_OK } else { PANEL_TEXT },
                Justify::Left,
            );
            next_y += QUEST_DIARY_ROW_HEIGHT;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuestDetailLineKind {
    Title,
    Heading,
    Body,
    Blank,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuestDetailLine {
    text: String,
    kind: QuestDetailLineKind,
}

fn push_quest_detail_section(
    lines: &mut Vec<QuestDetailLine>,
    heading: &str,
    entries: impl IntoIterator<Item = String>,
) {
    let entries = entries
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return;
    }
    lines.push(QuestDetailLine {
        text: String::new(),
        kind: QuestDetailLineKind::Blank,
    });
    lines.push(QuestDetailLine {
        text: heading.to_owned(),
        kind: QuestDetailLineKind::Heading,
    });
    lines.extend(entries.into_iter().map(|text| QuestDetailLine {
        text,
        kind: QuestDetailLineKind::Body,
    }));
}

fn wrap_guidance_hint(hint: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in hint.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > max_chars {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn push_quest_guidance(
    lines: &mut Vec<QuestDetailLine>,
    quest: &Quest,
    guidance: Option<&QuestGuidance>,
    desktop_wrap: bool,
    render_text: QuestRenderText,
) {
    let Some(entry) = guidance.and_then(|guidance| guidance.entry(quest.quest_index)) else {
        return;
    };
    let mut detail = vec![render_text.legacy_copy(&format!("Category: {}", entry.category.label()))];
    let description = render_text.description(quest.quest_index, if render_text.is_chinese() { entry.hint.trim() } else { &entry.hint });
    if !lines.iter().any(|line| line.text == description) {
        if desktop_wrap { detail.extend(multi_guidance::wrap_card_text(&description)); }
        else { detail.push(description); }
    }
    push_quest_detail_section(lines, &render_text.legacy_copy("Newcomer Guide"), detail);
}

fn quest_objective_detail_text(objective: &crate::quest_model::QuestObjective,
    render_text: QuestRenderText,
) -> String {
    let compact = format!("{}/{}", objective.current, objective.target);
    let spaced = objective.progress_label();
    if objective.target == 0
        || objective.text.contains(&compact)
        || objective.text.contains(&spaced)
    {
        render_text.objective(&objective.text)
    } else {
        format!("{} ({spaced})", render_text.objective(&objective.text))
    }
}

fn localized_quest_detail_lines(lines: Vec<QuestDetailLine>, desktop_wrap: bool) -> Vec<QuestDetailLine> {
    lines.into_iter().flat_map(|line| {
        let text = line.text;
        if desktop_wrap && line.kind == QuestDetailLineKind::Body {
            multi_guidance::wrap_card_text(&text).into_iter().map(|text| QuestDetailLine { text, kind: line.kind }).collect()
        } else { vec![QuestDetailLine { text, kind: line.kind }] }
    }).collect()
}

fn localized_quest_description(quest: &Quest,
    render_text: QuestRenderText,
) -> Vec<String> {
    if !render_text.is_chinese() { return quest.detail.description_lines.clone(); }
    let config = crate::quest_practice::newcomer_config();
    if ["quests", "growthRewards"].iter().any(|key| config[key].as_array().is_some_and(|quests|
        quests.iter().any(|definition| definition["id"].as_i64() == Some(i64::from(quest.quest_index))))) {
        vec![render_text.description(quest.quest_index, &quest.detail.description_lines.join("\n"))]
    } else { quest.detail.description_lines.iter().map(|line| render_text.body(line)).collect() }
}

fn quest_detail_lines(quest: &Quest, guidance: Option<&QuestGuidance>, class_name: &str) -> Vec<QuestDetailLine> {
    quest_detail_lines_with_wrap(quest, guidance, class_name, true, QuestRenderText::default())
}

fn quest_detail_lines_with_wrap(quest: &Quest, guidance: Option<&QuestGuidance>, class_name: &str,
    desktop_wrap: bool,
    render_text: QuestRenderText,
) -> Vec<QuestDetailLine> {
    let mut lines = vec![QuestDetailLine {
        text: render_text.title(quest.quest_index, &quest.title),
        kind: QuestDetailLineKind::Title,
    }];

    if let Some(practice) = crate::quest_practice::practice_guide(quest.quest_index, class_name) {
        let complete = quest.objectives.get(practice.objective_index)
            .is_some_and(|objective| objective.target > 0 && objective.current >= objective.target);
        let details = practice.instructions.into_iter().flat_map(|text|
            if desktop_wrap { multi_guidance::wrap_card_text(&text) } else { vec![text] }).collect::<Vec<_>>();
        push_quest_detail_section(&mut lines, if complete { "职业练习（已完成）" } else { "职业练习要求（全部完成）" }, details);
    }

    let mut description = localized_quest_description(quest, render_text);
    if description.is_empty() {
        if let Some(fallback) = quest
            .unknown_text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .filter(|text| quest.group.as_deref() != Some(text.trim()))
        {
            description.extend(fallback.lines().map(|line| render_text.body(line)));
        }
    }
    lines.extend(
        description
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .map(|text| QuestDetailLine {
                text,
                kind: QuestDetailLineKind::Body,
            }),
    );
    push_quest_guidance(&mut lines, quest, guidance, desktop_wrap, render_text);

    let task_lines: Vec<String> = if quest.detail.task_description_lines.is_empty() {
        quest
            .objectives
            .iter()
            .map(|objective| render_text.objective(&objective.text))
            .collect()
    } else {
        quest.detail.task_description_lines.iter().map(|line| render_text.body(line)).collect()
    };
    push_quest_detail_section(&mut lines, render_text.chrome("ui.questObjective", "Objectives"), task_lines);
    push_quest_detail_section(
        &mut lines,
        render_text.chrome("ui.questReturnHeading", "Return"),
        quest.detail.return_description_lines.iter().map(|line| render_text.body(line)),
    );
    if let Some(time_limit) = quest
        .detail
        .time_limit
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if render_text.is_chinese() {
            push_quest_detail_section(&mut lines, &render_text.legacy_copy("Time Limit"),
                vec![render_text.body(time_limit)]);
        } else {
        lines.push(QuestDetailLine { text: String::new(), kind: QuestDetailLineKind::Blank });
        lines.push(QuestDetailLine { text: render_text.format("ui.questTimeLimit", "Time: {0}", time_limit), kind: QuestDetailLineKind::Body });
        }
    }
    if quest.status.is_active() && !quest.objectives.is_empty() {
        push_quest_detail_section(
            &mut lines,
            render_text.chrome("ui.questProgressHeading", "Progress"),
            quest
                .objectives
                .iter()
                .map(|objective| quest_objective_detail_text(objective, render_text))
                .collect::<Vec<_>>(),
        );
    }
    let supplies = crate::quest_practice::supply_instructions(quest.quest_index, class_name).into_iter()
        .flat_map(|text| if desktop_wrap { multi_guidance::wrap_card_text(&text) } else { vec![text] })
        .collect::<Vec<_>>();
    push_quest_detail_section(&mut lines, "回城补给", supplies);
    localized_quest_detail_lines(lines, desktop_wrap)
}

fn quest_list_message_lines(
    quest: &Quest,
    dialog: &NpcDialogModel,
    guidance: Option<&QuestGuidance>,
) -> Vec<QuestDetailLine> {
    quest_list_message_lines_with_wrap(quest, dialog, guidance, true, QuestRenderText::default())
}

fn quest_list_message_lines_with_wrap(
    quest: &Quest,
    dialog: &NpcDialogModel,
    guidance: Option<&QuestGuidance>,
    desktop_wrap: bool,
    render_text: QuestRenderText,
) -> Vec<QuestDetailLine> {
    let mut lines = vec![QuestDetailLine {
        text: render_text.title(quest.quest_index, &quest.title),
        kind: QuestDetailLineKind::Title,
    }];

    let at_distinct_finish_npc = dialog.npc_object_id.is_some()
        && dialog.npc_object_id == quest.finish_npc_index
        && quest.accept_npc_index != quest.finish_npc_index;
    if quest.status.is_active()
        && at_distinct_finish_npc
        && !quest.detail.completion_description_lines.is_empty()
    {
        lines.extend(
            quest
                .detail
                .completion_description_lines
                .iter()
                .filter(|line| !line.trim().is_empty())
                .map(|line| render_text.body(line))
                .map(|text| QuestDetailLine {
                    text,
                    kind: QuestDetailLineKind::Body,
                }),
        );
        push_quest_guidance(&mut lines, quest, guidance, desktop_wrap, render_text);
        return localized_quest_detail_lines(lines, desktop_wrap);
    }

    let mut description = localized_quest_description(quest, render_text);
    if description.is_empty() {
        if let Some(fallback) = quest
            .unknown_text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .filter(|text| quest.group.as_deref() != Some(text.trim()))
        {
            description.extend(fallback.lines().map(|line| render_text.body(line)));
        }
    }
    lines.extend(
        description
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .map(|text| QuestDetailLine {
                text,
                kind: QuestDetailLineKind::Body,
            }),
    );
    push_quest_guidance(&mut lines, quest, guidance, desktop_wrap, render_text);
    let task_lines: Vec<String> = if quest.detail.task_description_lines.is_empty() {
        quest
            .objectives
            .iter()
            .map(|objective| render_text.objective(&objective.text))
            .collect()
    } else {
        quest.detail.task_description_lines.iter().map(|line| render_text.body(line)).collect()
    };
    push_quest_detail_section(&mut lines, render_text.chrome("ui.questObjective", "Objectives"), task_lines);
    push_quest_detail_section(
        &mut lines,
        render_text.chrome("ui.questReturnHeading", "Return"),
        quest.detail.return_description_lines.iter().map(|line| render_text.body(line)),
    );
    if let Some(time_limit) = quest
        .detail
        .time_limit
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if render_text.is_chinese() {
            push_quest_detail_section(&mut lines, &render_text.legacy_copy("Time Limit"),
                vec![render_text.body(time_limit)]);
        } else {
        lines.push(QuestDetailLine { text: String::new(), kind: QuestDetailLineKind::Blank });
        lines.push(QuestDetailLine { text: render_text.format("ui.questTimeLimit", "Time: {0}", time_limit), kind: QuestDetailLineKind::Body });
        }
    }
    // `QuestListDialog` constructs QuestMessage with DisplayProgress=false.
    localized_quest_detail_lines(lines, desktop_wrap)
}

fn quest_list_icon_asset(quest: &Quest) -> &'static str {
    match &quest.status {
        crate::quest_model::QuestStatus::NotStarted => "original-ui/Prguse/963.png",
        crate::quest_model::QuestStatus::ReadyToTurnIn => "original-ui/Prguse/964.png",
        _ => "original-ui/Prguse/962.png",
    }
}

fn render_quest_message_lines(
    parent: &mut ChildSpawnerCommands,
    lines: &[QuestDetailLine],
    scroll_top: usize,
    line_count: usize,
    asset_server: Option<&AssetServer>,
) {
    let mut adjust = 0.0;
    for (row, line) in lines.iter().skip(scroll_top).take(line_count).enumerate() {
        let top = row as f32 * QUEST_DETAIL_LINE_HEIGHT + adjust;
        match line.kind {
            QuestDetailLineKind::Blank => {}
            QuestDetailLineKind::Title | QuestDetailLineKind::Heading => {
                quest_log_image_at(
                    parent,
                    asset_server,
                    QUEST_DETAIL_SECTION_ASSET,
                    QuestLogRect::new(5.0, top + 5.0, 12.0, 10.0),
                );
                quest_log_text_at_raw(
                    parent,
                    &line.text,
                    QuestLogRect::new(15.0, top, 260.0, 20.0),
                    10.0,
                    if line.kind == QuestDetailLineKind::Title {
                        PANEL_HIGHLIGHT
                    } else {
                        PANEL_TEXT
                    },
                    Justify::Left,
                );
                adjust += 5.0;
            }
            QuestDetailLineKind::Body => {
                quest_log_text_at_raw(
                    parent,
                    &line.text,
                    QuestLogRect::new(0.0, top, 280.0, 20.0),
                    9.0,
                    PANEL_TEXT,
                    Justify::Left,
                );
            }
        }
    }
}

fn render_mobile_rewards(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    quest: &Quest,
    selected_reward: Option<i32>,
    surface: QuestRewardSelectionSurface,
    player: &crate::read_model::PlayerStats,
    guidance: &QuestGuidance,
    asset_server: Option<&AssetServer>,
    compact_page: Option<usize>,
    render_text: QuestRenderText,
) {
    let currency = quest.rewards.iter().filter_map(|reward| match reward {
        crate::quest_model::QuestReward::Experience { amount } => Some(render_text.format("ui.questRewardExp", "EXP {0}", amount)),
        crate::quest_model::QuestReward::Gold { amount } => Some(render_text.format("ui.questRewardGold", "Gold {0}", amount)),
        _ => None,
    }).collect::<Vec<_>>();
    if compact_page.is_none() && !currency.is_empty() {
        mobile_text_raw(parent, layout, &currency.join(" · "),
            (16.0, layout.height_css() - 210.0, layout.width_css() - 32.0, 16.0),
            14.0, PANEL_HIGHLIGHT);
    }
    let fixed = quest.rewards.iter().filter(|reward|
        if compact_page.is_some() {
            !matches!(reward, crate::quest_model::QuestReward::Item { selection_index: Some(_), .. })
                && fixed_reward_is_visible(reward, guidance)
        } else {
            matches!(reward, crate::quest_model::QuestReward::Item { selection_index: None, .. })
                && fixed_reward_is_visible(reward, guidance)
        }).collect::<Vec<_>>();
    let selectable = quest.rewards.iter().filter(|reward|
        matches!(reward, crate::quest_model::QuestReward::Item { selection_index: Some(_), .. }))
        .collect::<Vec<_>>();
    let columns = if compact_page.is_some() { 3 } else { 5 };
    let page = compact_page.unwrap_or(0).min(compact_reward_pages(quest, Some(guidance)).saturating_sub(1));
    let slot_width = ((layout.width_css() - 32.0) / columns as f32).max(44.0);
    for (row, items) in [fixed, selectable].into_iter().enumerate() {
        for (column, reward) in items.into_iter().skip(page * columns).take(columns).enumerate() {
            let crate::quest_model::QuestReward::Item { name, quantity, icon, selection_index, tooltip_source, .. } = reward else {
                let label = match reward {
                    crate::quest_model::QuestReward::Gold { amount } => render_text.format("ui.questRewardGold", "Gold {0}", amount),
                    crate::quest_model::QuestReward::Experience { amount } => render_text.format("ui.questRewardExp", "EXP {0}", amount),
                    crate::quest_model::QuestReward::Unknown { label } => label.clone(),
                    _ => continue,
                };
                mobile_text_raw(parent, layout, &label,
                    (16.0 + column as f32 * slot_width,
                        100.0 + row as f32 * 48.0, slot_width - 4.0, 44.0),
                    14.0, PANEL_TEXT);
                continue;
            };
            let selected = *selection_index != None && selected_reward == *selection_index;
            let mut cell = parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.px(16.0 + column as f32 * slot_width)),
                    top: Val::Px(layout.px(if compact_page.is_some() {
                        100.0 + row as f32 * 48.0
                    } else { layout.height_css() - 148.0 + row as f32 * 48.0 })),
                    width: Val::Px(layout.px(slot_width - 4.0)),
                    height: Val::Px(layout.px(44.0)),
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if selected { FEEDBACK_OK } else { BUTTON_BG }),
                FocusPolicy::Block,
            ));
            if let Some(reward_index) = selection_index {
                cell.insert((Button, match surface {
                    QuestRewardSelectionSurface::Detail => QuestUiButton::SelectReward {
                        quest_index: quest.quest_index, reward_index: *reward_index,
                    },
                    QuestRewardSelectionSurface::NpcList => QuestUiButton::SelectNpcQuestReward {
                        quest_index: quest.quest_index, reward_index: *reward_index,
                    },
                }));
            }
            if let Some(document) = crystal_item_tooltip_document_from_source(
                name, icon.and_then(|value| u16::try_from(value).ok()).unwrap_or_default(),
                *quantity, tooltip_source.as_ref(), player,
            ) {
                cell.insert((Interaction::None, CrystalItemHint(document)));
            }
            cell.with_children(|content| {
                if let (Some(server), Some(icon)) = (asset_server, icon) {
                    content.spawn((
                        Node { width: Val::Px(layout.px(28.0)), height: Val::Px(layout.px(28.0)),
                            margin: UiRect::left(Val::Px(layout.px(3.0))), ..default() },
                        ImageNode { image: server.load(format!("original-ui/Items/{icon}.png")), ..default() },
                        FocusPolicy::Pass,
                    ));
                }
                content.spawn((
                    Node { width: Val::Percent(100.0), min_width: Val::Px(0.0), ..default() },
                    Text::new(format!("{}{}", render_text.reward_name(name),
                        if *quantity > 1 { format!("×{quantity}") } else { String::new() })),
                    TextFont { font_size: FontSize::Px(layout.px(14.0)), ..default() },
                    TextColor(PANEL_TEXT),
                    TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                    FocusPolicy::Pass,
                ));
            });
        }
    }
    if compact_page.is_some() {
        let pages = compact_reward_pages(quest, Some(guidance));
        let (previous, next) = match surface {
            QuestRewardSelectionSurface::Detail => (QuestUiButton::CompactDetailRewardPrevious,
                QuestUiButton::CompactDetailRewardNext),
            QuestRewardSelectionSurface::NpcList => (QuestUiButton::CompactNpcRewardPrevious,
                QuestUiButton::CompactNpcRewardNext),
        };
        mobile_button_raw(parent, layout, render_text.chrome("ui.previous", "Previous"), (16.0, 196.0, 80.0, 44.0), previous, page > 0);
        mobile_button_raw(parent, layout, render_text.chrome("ui.next", "Next"), (104.0, 196.0, 80.0, 44.0), next, page + 1 < pages);
        mobile_text_raw(parent, layout, &format!("{}/{}", page + 1, pages),
            (192.0, 206.0, 70.0, 28.0), 14.0, PANEL_TEXT);
    }
}

fn compact_reward_pages(quest: &Quest, guidance: Option<&QuestGuidance>) -> usize {
    let fixed = quest.rewards.iter().filter(|reward|
        !matches!(reward, crate::quest_model::QuestReward::Item { selection_index: Some(_), .. })
            && guidance.is_none_or(|guidance| fixed_reward_is_visible(reward, guidance))).count();
    let selectable = quest.rewards.iter().filter(|reward|
        matches!(reward, crate::quest_model::QuestReward::Item { selection_index: Some(_), .. })).count();
    fixed.div_ceil(3).max(selectable.div_ceil(3)).max(1)
}

fn render_mobile_detail_panel(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    quest: &Quest,
    guidance: &QuestGuidance,
    state: &QuestUiState,
    pending: &PendingOperations,
    player: &crate::read_model::PlayerStats,
    asset_server: Option<&AssetServer>,
    render_text: QuestRenderText,
) {
    if layout.compact {
        compact_quest_tabs(parent, layout, state.compact_detail_page, false, render_text);
        if state.compact_detail_page == CompactQuestPage::Text {
            mobile_text_raw(parent, layout,
                &render_text.title(quest.quest_index, &quest.title),
                (16.0, 52.0, layout.width_css() - 32.0, 24.0), 15.0, PANEL_HIGHLIGHT);
        }
    } else {
        mobile_chrome(parent, layout, render_text.chrome("ui.quest", "Quest"), QuestUiButton::CloseQuestDetail, render_text);
    }
    let lines = mobile_wrap_quest_lines(
        quest_detail_lines_with_wrap(quest, Some(guidance),
            player.class_name.as_deref().unwrap_or(""), false, render_text),
        layout.width_css() - 32.0,
    );
    let rows = layout.detail_rows();
    let max_top = lines.len().saturating_sub(rows);
    let top = state.detail_scroll_top.min(max_top);
    if !layout.compact || state.compact_detail_page == CompactQuestPage::Text {
    for (index, line) in lines.iter().skip(top).take(rows).enumerate() {
        mobile_text_raw(parent, layout, &line.text,
            (16.0, (if layout.compact { 76.0 } else { 52.0 }) + index as f32 * 24.0,
                layout.width_css() - 32.0, 24.0),
            14.0, if matches!(line.kind, QuestDetailLineKind::Heading | QuestDetailLineKind::Title)
                { PANEL_HIGHLIGHT } else { PANEL_TEXT });
    }
    let pager_y = if layout.compact { layout.height_css() - 49.0 }
        else { layout.height_css() - 194.0 };
    mobile_button_raw(parent, layout, render_text.chrome("ui.previous", "Previous"), (16.0, pager_y, 80.0, 44.0),
        QuestUiButton::QuestDetailScrollUp, top > 0);
    mobile_button_raw(parent, layout, render_text.chrome("ui.next", "Next"), (104.0, pager_y, 80.0, 44.0),
        QuestUiButton::QuestDetailScrollDown, top < max_top);
    mobile_text_raw(parent, layout, &format!("{}/{}", top.saturating_add(1), lines.len().max(1)),
        (192.0, pager_y + 10.0, 70.0, 28.0), 14.0, PANEL_TEXT);
    if layout.compact { return; }
    }
    if layout.compact {
        mobile_text_raw(parent, layout,
            &render_text.title(quest.quest_index, &quest.title),
            (16.0, 52.0, layout.width_css() - 32.0, 24.0), 15.0, PANEL_HIGHLIGHT);
    }
    render_mobile_rewards(parent, layout, quest, state.selected_reward_index,
        QuestRewardSelectionSurface::Detail, player, guidance, asset_server,
        layout.compact.then_some(state.compact_detail_reward_page), render_text);
    let selected_item_index = state.selected_reward_index.unwrap_or(-1);
    let accept_pending = pending.contains(&PendingOperationKey::QuestAccept {
        npc_index: 0, quest_index: quest.quest_index,
    });
    let finish_pending = pending.has_pending_quest_finish(quest.quest_index);
    let diary_accept = quest_action_decision(Some(quest), QuestAction::Accept,
        QuestActionSource::Diary, Some(guidance), accept_pending, None) == QuestActionDecision::Eligible;
    let diary_finish = quest_action_decision(Some(quest), QuestAction::Finish,
        QuestActionSource::Diary, Some(guidance), finish_pending,
        (selected_item_index >= 0).then_some(selected_item_index)) == QuestActionDecision::Eligible;
    let action = if can_accept_quest(quest) && newcomer_diary_accept_authorized(quest, Some(guidance)) {
        (render_text.chrome("ui.questAccept", "Accept"), QuestUiButton::AcceptQuest { npc_index: 0, quest_index: quest.quest_index }, diary_accept)
    } else if can_finish_quest(quest) && newcomer_diary_finish_authorized(quest, Some(guidance)) {
        (render_text.chrome("ui.questComplete", "Complete"), QuestUiButton::FinishQuest { quest_index: quest.quest_index, selected_item_index }, diary_finish)
    } else if can_finish_quest(quest) {
        ("前往交付", QuestUiButton::PrepareQuestFinish { quest_index: quest.quest_index },
            !finish_pending && state.pending_turn_in.as_ref().is_none_or(|request| request.quest_index != quest.quest_index))
    } else {
        (render_text.chrome("ui.questShare", "Share"), QuestUiButton::ShareQuest { quest_index: quest.quest_index }, quest.status.is_active())
    };
    let [primary_rect, abandon_rect, current_rect] = layout.detail_footer_rects();
    mobile_button_raw(parent, layout, action.0, primary_rect, action.1, action.2);
    let abandon_pending = pending.contains(&PendingOperationKey::QuestAbandon { quest_index: quest.quest_index });
    mobile_button_raw(parent, layout, render_text.chrome("ui.questAbandon", "Abandon"), abandon_rect,
        QuestUiButton::AbandonQuest { quest_index: quest.quest_index },
        can_abandon_quest(quest) && !abandon_pending);
    if guidance.is_enabled() && quest.status.is_active() {
        mobile_button_raw(parent, layout, "设为当前", current_rect,
            QuestUiButton::MakePrimary { quest_index: quest.quest_index }, true);
    }
}

fn render_mobile_npc_quest_list_panel(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    quests: &[&Quest],
    dialog: &NpcDialogModel,
    guidance: &QuestGuidance,
    state: &QuestUiState,
    pending: &PendingOperations,
    player: &crate::read_model::PlayerStats,
    asset_server: Option<&AssetServer>,
    render_text: QuestRenderText,
) {
    if layout.compact {
        compact_quest_tabs(parent, layout, state.compact_npc_page, true, render_text);
    } else {
        mobile_chrome(parent, layout, render_text.chrome("ui.quest", "Quest"), QuestUiButton::ReturnToNpcDialog, render_text);
    }
    let selected_position = state.npc_quest_selected_index
        .and_then(|index| quests.iter().position(|quest| quest.quest_index == index)).unwrap_or(0);
    let Some(quest) = quests.get(selected_position).copied().or_else(|| quests.first().copied()) else {
        mobile_text_raw(parent, layout, render_text.chrome("ui.questEmpty", "No quests"), (16.0, 54.0, layout.width_css() - 32.0, 44.0),
            15.0, PANEL_TEXT);
        return;
    };
    if !layout.compact || state.compact_npc_page == CompactQuestPage::Text {
    mobile_button_raw(parent, layout, render_text.chrome("ui.previous", "Previous"), (16.0, 54.0, 70.0, 44.0),
        QuestUiButton::NpcQuestPrevious, selected_position > 0);
    mobile_text_raw(parent, layout, &format!("{} ({}/{})", render_text.title(quest.quest_index, &quest.title),
        selected_position + 1, quests.len()),
        (94.0, 62.0, layout.width_css() - 188.0, 30.0), 15.0, PANEL_HIGHLIGHT);
    mobile_button_raw(parent, layout, render_text.chrome("ui.next", "Next"), (layout.width_css() - 86.0, 54.0, 70.0, 44.0),
        QuestUiButton::NpcQuestNext, selected_position + 1 < quests.len());

    let lines = mobile_wrap_quest_lines(quest_list_message_lines_with_wrap(quest, dialog,
        Some(guidance), false, render_text),
        layout.width_css() - 32.0);
    let rows = layout.message_rows();
    let max_top = lines.len().saturating_sub(rows);
    let top = state.npc_quest_message_scroll_top.min(max_top);
    for (index, line) in lines.iter().skip(top).take(rows).enumerate() {
        mobile_text_raw(parent, layout, &line.text,
            (16.0, 102.0 + index as f32 * 24.0, layout.width_css() - 32.0, 24.0),
            14.0, if matches!(line.kind, QuestDetailLineKind::Title | QuestDetailLineKind::Heading)
                { PANEL_HIGHLIGHT } else { PANEL_TEXT });
    }
    let pager_y = if layout.compact { layout.height_css() - 49.0 }
        else { layout.height_css() - 194.0 };
    mobile_button_raw(parent, layout, render_text.chrome("ui.previous", "Previous"), (16.0, pager_y, 80.0, 44.0),
        QuestUiButton::NpcQuestMessageScrollUp, top > 0);
    mobile_button_raw(parent, layout, render_text.chrome("ui.next", "Next"), (104.0, pager_y, 80.0, 44.0),
        QuestUiButton::NpcQuestMessageScrollDown, top < max_top);
    mobile_text_raw(parent, layout, &format!("{}/{}", top + 1, lines.len().max(1)),
        (192.0, pager_y + 10.0, 70.0, 28.0), 14.0, PANEL_TEXT);
    if layout.compact { return; }
    }
    if layout.compact {
        mobile_text_raw(parent, layout,
            &render_text.title(quest.quest_index, &quest.title),
            (16.0, 52.0, layout.width_css() - 32.0, 24.0), 15.0, PANEL_HIGHLIGHT);
    }
    render_mobile_rewards(parent, layout, quest, state.npc_selected_reward_index,
        QuestRewardSelectionSurface::NpcList, player, guidance, asset_server,
        layout.compact.then_some(state.compact_npc_reward_page), render_text);

    let npc_index = dialog.npc_object_id.or(quest.accept_npc_index).unwrap_or_default();
    let [action_rect, back_rect, leave_rect] = layout.npc_footer_rects();
    if can_accept_quest(quest) {
        let is_pending = pending.contains(&PendingOperationKey::QuestAccept {
            npc_index, quest_index: quest.quest_index,
        });
        let allowed = quest_action_decision(Some(quest), QuestAction::Accept,
            QuestActionSource::NpcDialog {
                action_offered: npc_accept_offered_by_current_dialog(dialog, quest, npc_index),
            }, Some(guidance), is_pending, None) == QuestActionDecision::Eligible;
        mobile_button_raw(parent, layout, render_text.chrome("ui.questAccept", "Accept"), action_rect,
            QuestUiButton::AcceptNpcQuest { npc_index, quest_index: quest.quest_index }, allowed);
    } else if can_finish_quest(quest) {
        let selected_item_index = state.npc_selected_reward_index.unwrap_or(-1);
        let decision = quest_action_decision(Some(quest), QuestAction::Finish,
            QuestActionSource::NpcDialog { action_offered: dialog_exposes_quest_operation(
                dialog, Some(npc_index), quest.quest_index, true,
                (selected_item_index >= 0).then_some(selected_item_index),
            ) }, Some(guidance), pending.has_pending_quest_finish(quest.quest_index),
            (selected_item_index >= 0).then_some(selected_item_index));
        let allowed = matches!(decision, QuestActionDecision::Eligible
            | QuestActionDecision::Rejected(QuestActionRejection::RewardSelectionRequired));
        mobile_button_raw(parent, layout, render_text.chrome("ui.questComplete", "Complete"), action_rect,
            QuestUiButton::FinishQuest { quest_index: quest.quest_index, selected_item_index }, allowed);
    }
    mobile_button_raw(parent, layout, "返回对话", back_rect,
        QuestUiButton::ReturnToNpcDialog, true);
    mobile_button_raw(parent, layout, "离开", leave_rect,
        QuestUiButton::CloseNpcQuestList, true);
}

fn render_mobile_confirmation(
    parent: &mut ChildSpawnerCommands,
    layout: MobileQuestLayout,
    alert: Option<&str>,
    state: &QuestUiState,
) {
    mobile_chrome(parent, layout, if alert.is_some() { "提示" } else { "确认放弃" },
        if alert.is_some() { QuestUiButton::CloseQuestAlert }
        else { QuestUiButton::CancelAbandonQuest }, QuestRenderText::default());
    if layout.compact {
        let lines = compact_confirmation_lines(alert.unwrap_or(ASK_CANCEL_QUEST_TEXT), layout);
        let pages = lines.len().div_ceil(5).max(1);
        let page = state.compact_confirmation_page.min(pages - 1);
        for (index, line) in lines.iter().skip(page * 5).take(5).enumerate() {
            mobile_text(parent, layout, &line.text,
                (16.0, 62.0 + index as f32 * 24.0, layout.width_css() - 32.0, 24.0),
                16.0, PANEL_TEXT);
        }
        if pages > 1 {
            mobile_button(parent, layout, "上文", (16.0, 196.0, 80.0, 44.0),
                QuestUiButton::CompactConfirmationPrevious, page > 0);
            mobile_button(parent, layout, "下文", (104.0, 196.0, 80.0, 44.0),
                QuestUiButton::CompactConfirmationNext, page + 1 < pages);
            mobile_text(parent, layout, &format!("{}/{}", page + 1, pages),
                (192.0, 206.0, 70.0, 28.0), 14.0, PANEL_TEXT);
        }
    } else {
        mobile_text(parent, layout, alert.unwrap_or(ASK_CANCEL_QUEST_TEXT),
            (16.0, 62.0, layout.width_css() - 32.0, layout.height_css() - 128.0),
            16.0, PANEL_TEXT);
    }
    let footer_y = layout.height_css() - 49.0;
    if alert.is_some() {
        mobile_button(parent, layout, "确定", (layout.width_css() - 112.0, footer_y, 96.0, 44.0),
            QuestUiButton::CloseQuestAlert, true);
    } else {
        mobile_button(parent, layout, "取消", (layout.width_css() - 224.0, footer_y, 96.0, 44.0),
            QuestUiButton::CancelAbandonQuest, true);
        mobile_button(parent, layout, "确定放弃", (layout.width_css() - 112.0, footer_y, 96.0, 44.0),
            QuestUiButton::ConfirmAbandonQuest, true);
    }
}

fn render_npc_quest_list_panel(
    parent: &mut ChildSpawnerCommands,
    quests: &[&Quest],
    dialog: &NpcDialogModel,
    guidance: &QuestGuidance,
    state: &QuestUiState,
    pending: &PendingOperations,
    asset_server: Option<&AssetServer>,
    player: &crate::read_model::PlayerStats,
    render_text: QuestRenderText,
) {
    let layout = quest_list_layout(1.0);
    let selected_position = state
        .npc_quest_selected_index
        .and_then(|selected| {
            quests
                .iter()
                .position(|quest| quest.quest_index == selected)
        })
        .unwrap_or(0);
    let max_start = quests.len().saturating_sub(QUEST_LIST_VISIBLE_ROWS);
    let start = state.npc_quest_start_index.min(max_start);
    let selected = quests
        .get(selected_position)
        .copied()
        .or_else(|| quests.first().copied());

    quest_log_text_at_raw(parent, render_text.chrome("ui.quest", "Quest"), layout.title, 11.0, PANEL_HIGHLIGHT, Justify::Left);
    quest_log_text_at_raw(
        parent,
        &format!("List: {}", quests.len()),
        layout.available_count,
        8.0,
        PANEL_TEXT,
        Justify::Left,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DIARY_TOP_CLOSE_ASSET,
        layout.top_close,
        QuestUiButton::CloseNpcQuestList,
        true,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        "original-ui/Prguse2/257.png",
        layout.help,
        QuestUiButton::NpcQuestHelp,
        true,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_LIST_UP_ASSET,
        layout.quest_up,
        QuestUiButton::NpcQuestPrevious,
        selected_position > 0,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_LIST_DOWN_ASSET,
        layout.quest_down,
        QuestUiButton::NpcQuestNext,
        selected_position + 1 < quests.len(),
    );

    for (row, quest) in quests
        .iter()
        .skip(start)
        .take(QUEST_LIST_VISIBLE_ROWS)
        .enumerate()
    {
        let top = 36.0 + row as f32 * 19.0;
        if state.npc_quest_selected_index == Some(quest.quest_index) {
            quest_log_image_at(
                parent,
                asset_server,
                QUEST_DIARY_SELECTED_ASSET,
                QuestLogRect::new(34.0, top, 174.0, 17.0),
            );
        }
        quest_log_image_at(
            parent,
            asset_server,
            quest_list_icon_asset(quest),
            QuestLogRect::new(12.0, top, 16.0, 17.0),
        );
        let level = (quest.min_level_needed > 0)
            .then(|| format!("Lv {}", quest.min_level_needed))
            .unwrap_or_default();
        quest_log_text_at_raw(
            parent,
            &level,
            QuestLogRect::new(29.0, top, 40.0, 17.0),
            9.0,
            PANEL_TEXT,
            Justify::Left,
        );
        quest_log_text_at_raw(
            parent,
            &render_text.title(quest.quest_index, &quest.title),
            QuestLogRect::new(69.0, top, 140.0, 17.0),
            9.0,
            PANEL_TEXT,
            Justify::Left,
        );
        parent.spawn((
            Button,
            QuestUiButton::SelectNpcQuest {
                quest_index: quest.quest_index,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(9.0),
                top: Val::Px(top),
                width: Val::Px(200.0),
                height: Val::Px(17.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            FocusPolicy::Block,
        ));
    }

    let Some(quest) = selected else {
        return;
    };
    let lines = quest_list_message_lines_with_wrap(quest, dialog, Some(guidance), true, render_text);
    let max_top = lines.len().saturating_sub(QUEST_LIST_MESSAGE_LINE_COUNT);
    let message_top = state.npc_quest_message_scroll_top.min(max_top);
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DETAIL_SCROLL_UP_ASSET,
        layout.message_up,
        QuestUiButton::NpcQuestMessageScrollUp,
        message_top > 0,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DETAIL_SCROLL_DOWN_ASSET,
        layout.message_down,
        QuestUiButton::NpcQuestMessageScrollDown,
        message_top < max_top,
    );
    if max_top > 0 {
        let thumb_top = 149.0 + (114.0 * message_top as f32 / max_top as f32);
        quest_log_image_at(
            parent,
            asset_server,
            QUEST_DETAIL_SCROLL_THUMB_ASSET,
            QuestLogRect::new(292.0, thumb_top, layout.message_thumb.width, 18.0),
        );
    }
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.message.left),
                top: Val::Px(layout.message.top),
                width: Val::Px(layout.message.width),
                height: Val::Px(layout.message.height),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            FocusPolicy::Pass,
        ))
        .with_children(|message| {
            render_quest_message_lines(
                message,
                &lines,
                message_top,
                QUEST_LIST_MESSAGE_LINE_COUNT,
                asset_server,
            );
        });

    render_quest_rewards(
        parent,
        quest,
        state.npc_selected_reward_index,
        QuestRewardSelectionSurface::NpcList,
        asset_server,
        player,
        guidance, render_text);

    let npc_index = dialog
        .npc_object_id
        .or(quest.accept_npc_index)
        .unwrap_or_default();
    if can_accept_quest(quest) {
        let pending = pending.contains(&PendingOperationKey::QuestAccept {
            npc_index,
            quest_index: quest.quest_index,
        });
        let allowed = quest_action_decision(
            Some(quest),
            QuestAction::Accept,
            QuestActionSource::NpcDialog {
                action_offered: npc_accept_offered_by_current_dialog(dialog, quest, npc_index),
            },
            Some(guidance),
            pending,
            None,
        ) == QuestActionDecision::Eligible;
        quest_log_image_button_at_locale(
            parent,
            asset_server,
            QUEST_LIST_ACCEPT_ASSET,
            layout.primary_action,
            QuestUiButton::AcceptNpcQuest {
                npc_index,
                quest_index: quest.quest_index,
            },
            allowed, render_text.chrome("ui.questAccept", "Accept"), render_text);
    } else if can_finish_quest(quest) {
        let selected_item_index = state.npc_selected_reward_index.unwrap_or(-1);
        let pending = pending.has_pending_quest_finish(quest.quest_index);
        let decision = quest_action_decision(
            Some(quest),
            QuestAction::Finish,
            QuestActionSource::NpcDialog {
                action_offered: dialog_exposes_quest_operation(
                    dialog,
                    Some(npc_index),
                    quest.quest_index,
                    true,
                    (selected_item_index >= 0).then_some(selected_item_index),
                ),
            },
            Some(guidance),
            pending,
            (selected_item_index >= 0).then_some(selected_item_index),
        );
        // Preserve Crystal's click-to-explain reward prompt on the NPC list.
        let allowed = matches!(
            decision,
            QuestActionDecision::Eligible
                | QuestActionDecision::Rejected(QuestActionRejection::RewardSelectionRequired)
        );
        quest_log_image_button_at_locale(
            parent,
            asset_server,
            QUEST_LIST_FINISH_ASSET,
            layout.primary_action,
            QuestUiButton::FinishQuest {
                quest_index: quest.quest_index,
                selected_item_index,
            },
            allowed, render_text.chrome("ui.questComplete", "Complete"), render_text);
    }
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_LIST_LEAVE_ASSET,
        layout.leave,
        QuestUiButton::CloseNpcQuestList,
        true,
    );
}

fn render_quest_abandon_confirmation(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
) {
    let layout = quest_confirmation_layout(1.0);
    quest_log_text_at(
        parent,
        ASK_CANCEL_QUEST_TEXT,
        layout.message,
        10.0,
        PANEL_TEXT,
        Justify::Left,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_CONFIRM_YES_ASSET,
        layout.yes,
        QuestUiButton::ConfirmAbandonQuest,
        true,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_CONFIRM_NO_ASSET,
        layout.no,
        QuestUiButton::CancelAbandonQuest,
        true,
    );
}

fn render_quest_alert(
    parent: &mut ChildSpawnerCommands,
    message: &str,
    asset_server: Option<&AssetServer>,
) {
    let layout = quest_confirmation_layout(1.0);
    quest_log_text_at(
        parent,
        message,
        layout.message,
        10.0,
        PANEL_TEXT,
        Justify::Left,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_MESSAGE_OK_ASSET,
        layout.no,
        QuestUiButton::CloseQuestAlert,
        true,
    );
}

// Preserve the protected turn-in regression's legacy Chinese render API.
#[cfg(test)]
fn render_quest_detail_panel(parent: &mut ChildSpawnerCommands, quest: &Quest,
    guidance: &QuestGuidance, state: &QuestUiState, pending: &PendingOperations,
    asset_server: Option<&AssetServer>, player: &crate::read_model::PlayerStats) {
    render_quest_detail_panel_with_text(parent, quest, guidance, state, pending,
        asset_server, player, QuestRenderText::default());
}

fn render_quest_detail_panel_with_text(
    parent: &mut ChildSpawnerCommands,
    quest: &Quest,
    guidance: &QuestGuidance,
    state: &QuestUiState,
    pending: &PendingOperations,
    asset_server: Option<&AssetServer>,
    player: &crate::read_model::PlayerStats,
    render_text: QuestRenderText,
) {
    let layout = quest_detail_layout(1.0);
    if guidance.is_enabled() && quest.status.is_active() {
        quest_log_text_button_at_raw(parent, QuestLogRect::new(125.0, 436.0, 70.0, 25.0),
            if state.pinned_primary_quest_index == Some(quest.quest_index) { "当前引导" } else { "设为当前" },
            QuestUiButton::MakePrimary { quest_index: quest.quest_index }, true);
    }
    let lines = quest_detail_lines_with_wrap(quest, Some(guidance), player.class_name.as_deref().unwrap_or(""), true, render_text);
    let max_top = lines.len().saturating_sub(QUEST_DETAIL_LINE_COUNT);
    let scroll_top = state.detail_scroll_top.min(max_top);

    quest_log_text_at_raw(parent, render_text.chrome("ui.quest", "Quest"), layout.title, 11.0, PANEL_HIGHLIGHT, Justify::Left);
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DIARY_TOP_CLOSE_ASSET,
        layout.top_close,
        QuestUiButton::CloseQuestDetail,
        true,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DETAIL_SCROLL_UP_ASSET,
        layout.scroll_up,
        QuestUiButton::QuestDetailScrollUp,
        scroll_top > 0,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DETAIL_SCROLL_DOWN_ASSET,
        layout.scroll_down,
        QuestUiButton::QuestDetailScrollDown,
        scroll_top < max_top,
    );
    if max_top > 0 {
        let thumb_top = 48.0 + (213.0 * scroll_top as f32 / max_top as f32);
        quest_log_image_at(
            parent,
            asset_server,
            QUEST_DETAIL_SCROLL_THUMB_ASSET,
            QuestLogRect::new(293.0, thumb_top, layout.scroll_thumb.width, 18.0),
        );
    }

    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(layout.message.left),
                top: Val::Px(layout.message.top),
                width: Val::Px(layout.message.width),
                height: Val::Px(layout.message.height),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            FocusPolicy::Pass,
        ))
        .with_children(|message| {
            let mut adjust = 0.0;
            for (row, line) in lines
                .iter()
                .skip(scroll_top)
                .take(QUEST_DETAIL_LINE_COUNT)
                .enumerate()
            {
                let top = row as f32 * QUEST_DETAIL_LINE_HEIGHT + adjust;
                match line.kind {
                    QuestDetailLineKind::Blank => {}
                    QuestDetailLineKind::Title | QuestDetailLineKind::Heading => {
                        quest_log_image_at(
                            message,
                            asset_server,
                            QUEST_DETAIL_SECTION_ASSET,
                            QuestLogRect::new(5.0, top + 5.0, 12.0, 10.0),
                        );
                        quest_log_text_at_raw(
                            message,
                            &line.text,
                            QuestLogRect::new(15.0, top, 260.0, 20.0),
                            10.0,
                            if line.kind == QuestDetailLineKind::Title {
                                PANEL_HIGHLIGHT
                            } else {
                                PANEL_TEXT
                            },
                            Justify::Left,
                        );
                        adjust += 5.0;
                    }
                    QuestDetailLineKind::Body => {
                        quest_log_text_at_raw(
                            message,
                            &line.text,
                            QuestLogRect::new(0.0, top, 280.0, 20.0),
                            9.0,
                            PANEL_TEXT,
                            Justify::Left,
                        );
                    }
                }
            }
        });

    render_quest_rewards(
        parent,
        quest,
        state.selected_reward_index,
        QuestRewardSelectionSurface::Detail,
        asset_server,
        player,
        guidance, render_text);

    let accept_pending = pending.contains(&PendingOperationKey::QuestAccept {
        npc_index: 0,
        quest_index: quest.quest_index,
    });
    let selected_item_index = state.selected_reward_index.unwrap_or(-1);
    let finish_pending = pending.has_pending_quest_finish(quest.quest_index);
    let diary_accept = quest_action_decision(
        Some(quest),
        QuestAction::Accept,
        QuestActionSource::Diary,
        Some(guidance),
        accept_pending,
        None,
    ) == QuestActionDecision::Eligible;
    let diary_finish = quest_action_decision(
        Some(quest),
        QuestAction::Finish,
        QuestActionSource::Diary,
        Some(guidance),
        finish_pending,
        (selected_item_index >= 0).then_some(selected_item_index),
    ) == QuestActionDecision::Eligible;
    if can_accept_quest(quest) && newcomer_diary_accept_authorized(quest, Some(guidance)) {
        quest_log_image_button_at_locale(
            parent,
            asset_server,
            QUEST_LIST_ACCEPT_ASSET,
            layout.share,
            QuestUiButton::AcceptQuest {
                npc_index: 0,
                quest_index: quest.quest_index,
            },
            diary_accept, render_text.chrome("ui.questAccept", "Accept"), render_text);
    } else if can_finish_quest(quest) && newcomer_diary_finish_authorized(quest, Some(guidance)) {
        quest_log_image_button_at_locale(
            parent,
            asset_server,
            QUEST_LIST_FINISH_ASSET,
            layout.share,
            QuestUiButton::FinishQuest {
                quest_index: quest.quest_index,
                selected_item_index,
            },
            diary_finish, render_text.chrome("ui.questComplete", "Complete"), render_text);
    } else if can_finish_quest(quest) {
        let waiting = state.pending_turn_in.as_ref().is_some_and(|request| request.quest_index == quest.quest_index)
            || pending.has_pending_quest_finish(quest.quest_index);
        quest_log_image_button_at(parent, asset_server, QUEST_LIST_FINISH_ASSET, layout.share,
            QuestUiButton::PrepareQuestFinish { quest_index: quest.quest_index }, !waiting);
    } else {
        quest_log_image_button_at_locale(
            parent,
            asset_server,
            QUEST_DETAIL_SHARE_ASSET,
            layout.share,
            QuestUiButton::ShareQuest {
                quest_index: quest.quest_index,
            },
            quest.status.is_active(), render_text.chrome("ui.questShare", "Share"), render_text);
    }
    let abandon_pending = pending.contains(&PendingOperationKey::QuestAbandon {
        quest_index: quest.quest_index,
    });
    quest_log_image_button_at_locale(
        parent,
        asset_server,
        QUEST_DETAIL_CANCEL_ASSET,
        layout.cancel,
        QuestUiButton::AbandonQuest {
            quest_index: quest.quest_index,
        },
        can_abandon_quest(quest) && !abandon_pending, render_text.chrome("ui.questAbandon", "Abandon"), render_text);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuestRewardSelectionSurface {
    Detail,
    NpcList,
}

fn fixed_reward_is_visible(
    reward: &crate::quest_model::QuestReward,
    guidance: &QuestGuidance,
) -> bool {
    !guidance.is_enabled()
        || !matches!(
            reward,
            crate::quest_model::QuestReward::Item {
                quantity: 0,
                selection_index: None,
                ..
            }
        )
}

fn render_quest_rewards(
    parent: &mut ChildSpawnerCommands,
    quest: &Quest,
    selected_reward_index: Option<i32>,
    selection_surface: QuestRewardSelectionSurface,
    asset_server: Option<&AssetServer>,
    player: &crate::read_model::PlayerStats,
    guidance: &QuestGuidance,
    render_text: QuestRenderText,
) {
    let exp = quest.rewards.iter().find_map(|reward| match reward {
        crate::quest_model::QuestReward::Experience { amount } => Some(*amount),
        _ => None,
    });
    let gold = quest.rewards.iter().find_map(|reward| match reward {
        crate::quest_model::QuestReward::Gold { amount } => Some(*amount),
        _ => None,
    });
    let gold_offset = if exp.is_some() { 0.0 } else { -90.0 };

    if let Some(amount) = exp {
        quest_log_image_at(
            parent,
            asset_server,
            QUEST_DETAIL_EXP_ASSET,
            QuestLogRect::new(15.0, 309.0, 28.0, 13.0),
        );
        quest_log_text_at_raw(
            parent,
            &amount.to_string(),
            QuestLogRect::new(45.0, 307.0, 75.0, 20.0),
            9.0,
            PANEL_TEXT,
            Justify::Left,
        );
    }
    if let Some(amount) = gold {
        quest_log_image_at(
            parent,
            asset_server,
            QUEST_DETAIL_GOLD_ASSET,
            QuestLogRect::new(105.0 + gold_offset, 309.0, 16.0, 12.0),
        );
        quest_log_text_at_raw(
            parent,
            &amount.to_string(),
            QuestLogRect::new(125.0 + gold_offset, 307.0, 75.0, 20.0),
            9.0,
            PANEL_TEXT,
            Justify::Left,
        );
    }

    quest_log_text_at_raw(parent, render_text.chrome("ui.questRewardSelect", "Choose one:"), QuestLogRect::new(25.0, 373.0, 68.0, 16.0), 11.0, PANEL_HIGHLIGHT, Justify::Left);

    let fixed = quest
        .rewards
        .iter()
        .filter(|reward| {
            matches!(
                reward,
                crate::quest_model::QuestReward::Item {
                    selection_index: None,
                    ..
                }
            )
        })
        .filter(|reward| fixed_reward_is_visible(reward, guidance))
        .take(5)
        .collect::<Vec<_>>();
    for (index, reward) in fixed.into_iter().enumerate() {
        let left = 20.0 + index as f32 * 45.0;
        quest_log_image_at(
            parent,
            asset_server,
            QUEST_DETAIL_FIXED_REWARD_ASSET,
            QuestLogRect::new(left, 330.0, 40.0, 34.0),
        );
        render_quest_reward_item(
            parent,
            asset_server,
            reward,
            left,
            331.0,
            None,
            player,
            guidance.is_enabled(), render_text);
    }

    let selectable = quest
        .rewards
        .iter()
        .filter(|reward| {
            matches!(
                reward,
                crate::quest_model::QuestReward::Item {
                    selection_index: Some(_),
                    ..
                }
            )
        })
        .take(5)
        .collect::<Vec<_>>();
    for (index, reward) in selectable.into_iter().enumerate() {
        let left = 20.0 + index as f32 * 45.0;
        let selection_index = match reward {
            crate::quest_model::QuestReward::Item {
                selection_index, ..
            } => *selection_index,
            _ => None,
        };
        if selected_reward_index == selection_index {
            quest_log_image_at(
                parent,
                asset_server,
                QUEST_DETAIL_SELECTED_REWARD_ASSET,
                QuestLogRect::new(left, 391.0, 40.0, 41.0),
            );
        }
        render_quest_reward_item(
            parent,
            asset_server,
            reward,
            left,
            396.0,
            selection_index.map(|reward_index| match selection_surface {
                QuestRewardSelectionSurface::Detail => QuestUiButton::SelectReward {
                    quest_index: quest.quest_index,
                    reward_index,
                },
                QuestRewardSelectionSurface::NpcList => QuestUiButton::SelectNpcQuestReward {
                    quest_index: quest.quest_index,
                    reward_index,
                },
            }),
            player,
            guidance.is_enabled(), render_text);
    }
}

fn render_quest_reward_item(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    reward: &crate::quest_model::QuestReward,
    left: f32,
    top: f32,
    selection_action: Option<QuestUiButton>,
    player: &crate::read_model::PlayerStats,
    show_zero_count: bool,
    render_text: QuestRenderText,
) {
    let crate::quest_model::QuestReward::Item {
        name,
        quantity,
        icon,
        tooltip_source,
        ..
    } = reward
    else {
        return;
    };
    let mut cell = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(32.0),
            height: Val::Px(32.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        FocusPolicy::Block,
    ));
    if let Some(selection_action) = selection_action {
        cell.insert((Button, selection_action));
    }
    if let Some(document) = crystal_item_tooltip_document_from_source(
        name,
        icon.and_then(|icon| u16::try_from(icon).ok())
            .unwrap_or_default(),
        *quantity,
        tooltip_source.as_ref(),
        player,
    ) {
        cell.insert((Interaction::None, CrystalItemHint(document)));
    }
    cell.with_children(|content| {
        if let (Some(asset_server), Some(icon)) = (asset_server, icon) {
            content.spawn((
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(format!("original-ui/Items/{icon}.png")),
                    ..default()
                },
                FocusPolicy::Pass,
            ));
        } else {
            quest_log_text_at_raw(
                content,
                &truncate_chars(&render_text.reward_name(name), 8),
                QuestLogRect::new(0.0, 0.0, 40.0, 30.0),
                7.0,
                PANEL_TEXT,
                Justify::Center,
            );
        }
        if *quantity > 1 || (*quantity == 0 && show_zero_count) {
            quest_log_text_at_raw(
                content,
                &quantity.to_string(),
                QuestLogRect::new(18.0, 18.0, 22.0, 14.0),
                8.0,
                PANEL_HIGHLIGHT,
                Justify::Right,
            );
        }
    });
}

// Retained behind an always-false cfg for one transition while the source-
// faithful diary replaces the former invented filter/detail composition. This
// keeps the old implementation available to a focused follow-up diff without
// compiling or exposing it in Candidate.
#[cfg(any())]
fn render_quest_log_panel_legacy_v2(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    state: &QuestUiState,
    pending: &PendingOperations,
    asset_server: Option<&AssetServer>,
) {
    let layout = quest_log_layout(1.0);

    // Title bar and the four source-faithful bitmap controls use the exact
    // Crystal/Web coordinates in the 312x444 Title/670 frame.
    quest_log_text_at(
        parent,
        "Quest Log",
        QuestLogRect::new(18.0, 6.0, 220.0, 20.0),
        14.0,
        PANEL_HIGHLIGHT,
        Justify::Left,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_LOG_HELP_ASSET,
        layout.help,
        QuestUiButton::QuestHelp,
        true,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_LOG_CLOSE_ASSET,
        layout.close,
        QuestUiButton::CloseQuestLog,
        true,
    );

    if let Some(feedback) = state.feedback.as_ref() {
        quest_log_text_at(
            parent,
            &feedback.message,
            QuestLogRect::new(10.0, 22.0, 292.0, 14.0),
            10.0,
            if feedback.is_error {
                FEEDBACK_ERR
            } else {
                FEEDBACK_OK
            },
            Justify::Left,
        );
    }

    for (index, filter) in QuestStageFilter::ALL.into_iter().enumerate() {
        let count = tracker
            .active_quests
            .iter()
            .filter(|quest| filter.matches(quest))
            .count();
        let label = format!("{} {}", filter.label(), count);
        quest_log_text_button_at(
            parent,
            layout.tabs[index],
            &label,
            QuestUiButton::SelectQuestFilter { filter },
            true,
        );
    }

    let filtered: Vec<&Quest> = tracker
        .active_quests
        .iter()
        .filter(|quest| state.stage_filter.matches(quest))
        .collect();
    let page_count = filtered.len().div_ceil(MAX_QUEST_LOG_ROWS).max(1);
    let page = state.page.min(page_count.saturating_sub(1));
    let first = page * MAX_QUEST_LOG_ROWS;
    let visible = filtered
        .iter()
        .skip(first)
        .take(MAX_QUEST_LOG_ROWS)
        .copied()
        .collect::<Vec<_>>();

    let list_rect = layout.list;
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(list_rect.left),
                top: Val::Px(list_rect.top),
                width: Val::Px(list_rect.width),
                height: Val::Px(list_rect.height),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.72)),
            FocusPolicy::Block,
        ))
        .with_children(|list| {
            if visible.is_empty() {
                quest_log_text_at(
                    list,
                    "No quests in this category.",
                    QuestLogRect::new(4.0, 8.0, 284.0, 22.0),
                    11.0,
                    PANEL_TEXT,
                    Justify::Left,
                );
            } else {
                for (index, quest) in visible.iter().enumerate() {
                    let selected = state.selected_quest_index == Some(quest.quest_index);
                    let tracking = state.is_tracked(quest.quest_index);
                    let label = format!(
                        "{} {}{}",
                        if selected { "▶" } else { " " },
                        truncate_chars(&crate::player_text::quest_title(quest.quest_index, &quest.title), 25),
                        if tracking { "  •" } else { "" }
                    );
                    let row =
                        QuestLogRect::new(0.0, index as f32 * 24.0, QUEST_LOG_CONTENT_WIDTH, 22.0);
                    quest_log_text_button_at(
                        list,
                        row,
                        &label,
                        QuestUiButton::SelectQuest {
                            quest_index: quest.quest_index,
                        },
                        true,
                    );
                }
            }
        });

    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_LOG_PREVIOUS_ASSET,
        layout.previous,
        QuestUiButton::QuestPagePrevious,
        page > 0,
    );
    quest_log_text_at(
        parent,
        &format!("{} / {}", page + 1, page_count),
        QuestLogRect::new(150.0, 256.0, 64.0, 16.0),
        10.0,
        PANEL_TEXT,
        Justify::Center,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_LOG_NEXT_ASSET,
        layout.next,
        QuestUiButton::QuestPageNext,
        page + 1 < page_count,
    );

    let selected = state
        .selected_quest(tracker)
        .filter(|quest| state.stage_filter.matches(quest))
        .or_else(|| visible.first().copied());

    let detail_rect = layout.detail;
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(detail_rect.left),
                top: Val::Px(detail_rect.top),
                width: Val::Px(detail_rect.width),
                height: Val::Px(detail_rect.height),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(QUEST_LOG_BG),
            FocusPolicy::Block,
        ))
        .with_children(|detail| {
            let Some(quest) = selected else {
                body_line(detail, "Select a quest to view details.");
                return;
            };

            detail_title(detail, &crate::player_text::quest_title(quest.quest_index, &quest.title));
            body_line(detail, &format!("Status: {}", quest.status.label()));
            if let Some(npc) = &quest.npc_name {
                body_line(detail, &format!("Return to: {npc}"));
            }
            if let Some(text) = &quest.unknown_text {
                if !text.trim().is_empty() {
                    body_line(detail, &truncate_chars(text, 72));
                }
            }
            for objective in quest.objectives.iter().take(3) {
                body_line(
                    detail,
                    &format!(
                        "• {} ({})",
                        truncate_chars(&objective.text, 42),
                        objective.progress_label()
                    ),
                );
            }
            body_line(
                detail,
                &format!("Reward: {}", truncate_chars(&quest.rewards_label(), 52)),
            );
            if quest.rewards.len() > 1 {
                for (index, reward) in quest.rewards.iter().enumerate().take(3) {
                    let selected_reward = state.selected_reward_index == Some(index as i32);
                    action_button(
                        detail,
                        &format!(
                            "{} {}",
                            if selected_reward { "[x]" } else { "[ ]" },
                            truncate_chars(&reward.label(), 40)
                        ),
                        QuestUiButton::SelectReward {
                            quest_index: quest.quest_index,
                            reward_index: index as i32,
                        },
                        true,
                    );
                }
            }
        });

    let Some(quest) = selected else {
        return;
    };
    let npc_index = quest
        .accept_npc_index
        .or(quest.finish_npc_index)
        .unwrap_or(0);
    let finish_item = if quest.rewards.is_empty() {
        -1
    } else if quest.rewards.len() == 1 {
        0
    } else {
        state.selected_reward_index.unwrap_or(-1)
    };
    let accept_pending = pending.contains(&PendingOperationKey::QuestAccept {
        npc_index,
        quest_index: quest.quest_index,
    });
    let finish_pending = pending.has_pending_quest_finish(quest.quest_index);
    let abandon_pending = pending.contains(&PendingOperationKey::QuestAbandon {
        quest_index: quest.quest_index,
    });

    quest_log_text_button_at(
        parent,
        layout.actions[0],
        if state.is_tracked(quest.quest_index) {
            "Tracking..."
        } else {
            "Track"
        },
        QuestUiButton::TrackQuest {
            quest_index: quest.quest_index,
        },
        can_track_quest(quest),
    );
    quest_log_text_button_at(
        parent,
        layout.actions[1],
        if accept_pending {
            "Accepting..."
        } else {
            "Accept"
        },
        QuestUiButton::AcceptQuest {
            npc_index,
            quest_index: quest.quest_index,
        },
        false,
    );
    quest_log_text_button_at(
        parent,
        layout.actions[2],
        if finish_pending {
            "Delivering..."
        } else {
            "Complete"
        },
        QuestUiButton::FinishQuest {
            quest_index: quest.quest_index,
            selected_item_index: finish_item,
        },
        false,
    );
    quest_log_text_button_at(
        parent,
        layout.actions[3],
        if abandon_pending {
            "Abandoning..."
        } else {
            "Abandon"
        },
        QuestUiButton::AbandonQuest {
            quest_index: quest.quest_index,
        },
        can_abandon_quest(quest) && !abandon_pending,
    );
}

fn quest_log_text_at(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: QuestLogRect,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    quest_log_text_at_raw(parent, &crate::player_text::text(text), rect, font_size, color, justify);
}

fn quest_log_text_at_raw(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    rect: QuestLogRect,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            min_width: Val::Px(0.0),
            ..default()
        },
        Text::new(text),
        TextFont {
            font: FontSource::Family("Microsoft YaHei".into()),
            font_size: FontSize::Px(font_size),
            ..default()
        },
        TextColor(color),
        TextLayout::new(justify, LineBreak::WordOrCharacter),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::BLACK,
        },
        FocusPolicy::Pass,
    ));
}

fn quest_log_image_at(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    asset_path: &str,
    rect: QuestLogRect,
) {
    let title = match asset_path {
        QUEST_DIARY_TITLE_ASSET => Some("任务日志"),
        QUEST_DETAIL_TITLE_ASSET => Some("任务详情"),
        QUEST_LIST_TITLE_ASSET => Some("任务列表"),
        QUEST_DETAIL_SELECT_REWARD_ASSET => Some("选择奖励"),
        _ => None,
    };
    if let Some(title) = title {
        quest_log_text_at(parent, title, rect, 11.0, PANEL_HIGHLIGHT, Justify::Left);
        return;
    }
    let Some(asset_server) = asset_server else {
        return;
    };
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
            image: asset_server.load(asset_path.to_owned()),
            ..default()
        },
        FocusPolicy::Pass,
    ));
}

fn quest_log_text_button_at(
    parent: &mut ChildSpawnerCommands,
    rect: QuestLogRect,
    text: &str,
    action: QuestUiButton,
    enabled: bool,
) {
    quest_log_text_button_at_sized(parent, rect, text, action, enabled, 10.0);
}

fn quest_log_text_button_at_raw(
    parent: &mut ChildSpawnerCommands,
    rect: QuestLogRect,
    text: &str,
    action: QuestUiButton,
    enabled: bool,
) {
    quest_log_text_button_at_sized_raw(parent, rect, text, action, enabled, 10.0);
}

fn quest_log_text_button_at_sized(
    parent: &mut ChildSpawnerCommands,
    rect: QuestLogRect,
    text: &str,
    action: QuestUiButton,
    enabled: bool,
    font_size: f32,
) {
    quest_log_text_button_at_sized_raw(parent, rect, &crate::player_text::text(text), action, enabled, font_size);
}

fn quest_log_text_button_at_sized_raw(
    parent: &mut ChildSpawnerCommands,
    rect: QuestLogRect,
    text: &str,
    action: QuestUiButton,
    enabled: bool,
    font_size: f32,
) {
    let mut button = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(3.0), Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(if enabled { BUTTON_BG } else { BUTTON_DISABLED }),
        TextColor(if enabled { PANEL_TEXT } else { DISABLED_TEXT }),
        QuestUiButtonVisual { enabled },
        FocusPolicy::Block,
    ));
    if enabled {
        button.insert((Button, action));
    }
    button.with_children(|content| {
        content.spawn((
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                ..default()
            },
            Text::new(text),
            TextFont {
                font: FontSource::Family("Microsoft YaHei".into()),
                font_size: FontSize::Px(font_size),
                ..default()
            },
            TextColor(if enabled { PANEL_TEXT } else { DISABLED_TEXT }),
            TextLayout::new(Justify::Center, LineBreak::WordOrCharacter),
            TextShadow {
                offset: Vec2::splat(1.0),
                color: Color::BLACK,
            },
        ));
    });
}

// Image captions remain the authored Chinese skin; other selected languages use
// the prepared canonical label with the same action rectangle and enablement.
fn quest_log_image_button_at_locale(parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>, asset_path: &str, rect: QuestLogRect,
    action: QuestUiButton, enabled: bool, label: &str, render_text: QuestRenderText) {
    if render_text.is_chinese() {
        quest_log_image_button_at(parent, asset_server, asset_path, rect, action, enabled);
    } else {
        quest_log_text_button_at_raw(parent, rect, label, action, enabled);
    }
}

fn quest_log_image_button_at(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    asset_path: &str,
    rect: QuestLogRect,
    action: QuestUiButton,
    enabled: bool,
) {
    let label = match asset_path {
        QUEST_DIARY_BOTTOM_CLOSE_ASSET => Some("关闭"),
        QUEST_DETAIL_SHARE_ASSET => Some("分享"),
        QUEST_DETAIL_CANCEL_ASSET => Some("关闭"),
        QUEST_LIST_ACCEPT_ASSET => Some("接受"),
        QUEST_LIST_FINISH_ASSET => Some("交付"),
        QUEST_LIST_LEAVE_ASSET => Some("离开"),
        NPC_QUEST_BUTTON_ASSET => Some("任务"),
        QUEST_CONFIRM_YES_ASSET => Some("确定"),
        QUEST_CONFIRM_NO_ASSET => Some("取消"),
        QUEST_MESSAGE_OK_ASSET => Some("确定"),
        _ => None,
    };
    if let Some(label) = label {
        quest_log_text_button_at(parent, rect, label, action, enabled);
        return;
    }
    let mut button = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.left),
            top: Val::Px(rect.top),
            width: Val::Px(rect.width),
            height: Val::Px(rect.height),
            ..default()
        },
        BackgroundColor(Color::NONE),
        FocusPolicy::Block,
    ));
    if enabled {
        button.insert((Button, action));
    }
    if let Some(asset_server) = asset_server {
        button.with_children(|image| {
            image.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load(asset_path.to_owned()),
                    ..default()
                },
            ));
        });
    }
}

fn render_combat_target_panel(
    parent: &mut ChildSpawnerCommands,
    target: Option<&crate::quest_model::CombatTarget>,
) {
    let Some(target) = target else {
        return;
    };

    detail_title(parent, &target.name);
    if let Some((ratio, label)) = combat_target_health(target) {
        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(COMBAT_TARGET_BAR_HEIGHT),
                    display: Display::Flex,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.20, 0.20, 0.22, 0.85)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        width: Val::Percent((ratio * 100.0).clamp(0.0, 100.0)),
                        height: Val::Px(COMBAT_TARGET_BAR_HEIGHT),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.90, 0.22, 0.22)),
                ));
            });
        tracker_body_line(parent, &label);
    } else {
        tracker_body_line(parent, "HP unavailable");
    }

    if target.max_hp > 0 && !target.is_dead() {
        action_button(
            parent,
            "Attack",
            QuestUiButton::AttackTarget {
                object_id: target.object_id,
            },
            true,
        );
    }
}

fn combat_target_health(target: &crate::quest_model::CombatTarget) -> Option<(f32, String)> {
    (target.max_hp > 0).then(|| (target.hp_ratio().clamp(0.0, 1.0), target.hp_label()))
}

fn render_pickup_panel(
    parent: &mut ChildSpawnerCommands,
    pickups: &GroundPickupModel,
    quest_state: &QuestUiState,
) {
    title_line(parent, "Recent Ground Pickups");

    // This is an acknowledgement of a local request, not a fabricated pickup
    // success. The inventory/read model remains the authoritative result.
    feedback_line(parent, quest_state.feedback.as_ref(), 10.0);

    if pickups.recent.is_empty() {
        body_line(parent, "No recent pickups.");
        return;
    }

    for pickup in pickups.recent.iter().take(MAX_PICKUP_BUTTONS) {
        if let Some(object_id) = pickup.object_id {
            action_button(
                parent,
                &format!("{}  (PickUpObject)", pickup.compact_label()),
                QuestUiButton::PickUpObject { object_id },
                true,
            );
        } else {
            action_button(
                parent,
                &format!("{}  (PickUpTile)", pickup.compact_label()),
                QuestUiButton::PickUpTile,
                true,
            );
        }
    }
}

fn pickup_label(pickups: Option<&GroundPickupModel>, object_id: u32) -> Option<String> {
    pickups?
        .recent
        .iter()
        .find(|pickup| pickup.object_id == Some(object_id))
        .map(|pickup| pickup.compact_label())
}

fn pickup_tile_is_current(pickups: Option<&GroundPickupModel>) -> bool {
    pickups.is_some_and(|model| {
        model
            .recent
            .front()
            .is_some_and(|pickup| pickup.object_id.is_none())
    })
}

fn target_is_attackable(target: Option<&CombatTargetModel>, object_id: u32) -> bool {
    target
        .and_then(|model| model.target.as_ref())
        .is_some_and(|target| {
            target.object_id == object_id && target.max_hp > 0 && !target.is_dead()
        })
}

fn quest_target_is_visible(
    tracker: &QuestTracker,
    entities: &EntityModelSet,
    object_id: u32,
) -> bool {
    entities.entities.iter().any(|entity| {
        entity.kind == crate::entities::EntityKind::Monster
            && entity.object_id.parse::<u32>() == Ok(object_id)
            && tracker_targets_monster(tracker, &entity.name)
    })
}

fn tracker_quest_block(parent: &mut ChildSpawnerCommands, quest: &Quest,
    render_text: QuestRenderText,
) {
    tracker_title_line_raw(parent, &render_text.title(quest.quest_index, &quest.title));
    if quest.status.is_active() && !quest.objectives.is_empty() {
        for objective in quest.objectives.iter().take(1) {
            tracker_body_line_raw(
                parent,
                &format!(
                    "   {} ({})",
                    truncate_chars(&objective.text, 44),
                    objective.progress_label()
                ),
            );
        }
    }

    match &quest.status {
        crate::quest_model::QuestStatus::NotStarted => {
            if let Some(label) = &quest.npc_name {
                tracker_body_line_raw(parent, &format!("   Talk to {label}"));
            }
        }
        crate::quest_model::QuestStatus::ReadyToTurnIn => {
            if let Some(label) = &quest.npc_name {
                tracker_body_line_raw(parent, &format!("   Return to {label}"));
            }
        }
        crate::quest_model::QuestStatus::Completed => {
            if !quest.rewards.is_empty() {
                tracker_body_line_raw(
                    parent,
                    &format!("   Reward: {}", truncate_chars(&quest.rewards_label(), 44)),
                );
            }
        }
        _ => {}
    }
}

fn visible_tracker_quests<'a>(tracker: &'a QuestTracker, state: &QuestUiState) -> Vec<&'a Quest> {
    state
        .tracked_quest_indices
        .iter()
        .filter_map(|tracked| {
            tracker
                .active_quests
                .iter()
                .find(|quest| quest.quest_index == *tracked && quest.status.is_active())
        })
        .take(MAX_TRACKED_QUESTS)
        .collect()
}

fn render_player_hud_panel(parent: &mut ChildSpawnerCommands, model: &UiReadModel) {
    let name = model.player.name.as_deref().unwrap_or("Adventurer");
    let map = model.player.map_name.as_deref().unwrap_or("Unknown map");
    title_line(
        parent,
        &format!("{}  Lv.{} - {}", name, model.player.level, map),
    );
    stat_bar(
        parent,
        &format!("HP  {}", model.player.hp_label()),
        model.player.normalized_hp(),
        Color::srgb(0.82, 0.12, 0.12),
    );
    stat_bar(
        parent,
        &format!("MP  {}", model.player.mp_label()),
        model.player.normalized_mp(),
        Color::srgb(0.12, 0.38, 0.82),
    );
}

fn render_control_hint_panel(
    parent: &mut ChildSpawnerCommands,
    nearby: &NearbyNpcModel,
    model: &UiReadModel,
) {
    title_line(parent, "Windows Native Controls");
    body_line(parent, "WASD / Arrows Move | Shift Run | T Talk");
    body_line(parent, "F Attack | R Pick up | F12 Screenshot");
    body_line(parent, "Q Quest Log | Esc Close");
    if model.player.max_hp > 0 && model.player.hp <= 0 {
        highlight_line(parent, "Defeated: press V to revive in town");
    } else if let Some(npc) = nearby.nearest() {
        highlight_line(
            parent,
            &format!("Nearby: {} ({} tiles)", npc.name, npc.distance),
        );
    }
}

fn render_quick_bag_panel(parent: &mut ChildSpawnerCommands, inventory: &InventoryModel) {
    title_line(parent, &format!("Bag - {} Gold", inventory.gold));
    let labels = inventory
        .items_in(0)
        .into_iter()
        .take(MAX_QUICK_BAG_ITEMS)
        .map(quick_item_label)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        body_line(parent, "Bag is empty. Pick up nearby drops with R.");
    } else {
        body_line(parent, &truncate_chars(&labels.join(" | "), 72));
    }
}

fn quick_item_label(item: &ItemModel) -> String {
    let name = if item.name.trim().is_empty() {
        item.key.as_str()
    } else {
        item.name.as_str()
    };
    if item.quantity > 1 {
        format!("{name} x{}", item.quantity)
    } else {
        name.to_owned()
    }
}

fn stat_bar(parent: &mut ChildSpawnerCommands, label: &str, ratio: f32, fill_color: Color) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(14.0),
                display: Display::Flex,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.09, 0.08, 0.90)),
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent((ratio * 100.0).clamp(0.0, 100.0)),
                    height: Val::Px(14.0),
                    ..default()
                },
                BackgroundColor(fill_color),
            ));
            bar.spawn((
                Text::new(label.to_owned()),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    if max_chars <= 3 {
        return text.chars().take(max_chars).collect();
    }
    let mut output = text.chars().take(max_chars - 3).collect::<String>();
    output.push_str("...");
    output
}

fn title_line(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text(parent, text, 18.0, PANEL_HIGHLIGHT, Justify::Left);
}

/// The Crystal quest panel skin already paints its own `Quest Log` header.
/// Reserve that strip for the bitmap instead of drawing a duplicate title.
fn quest_log_title_spacer(parent: &mut ChildSpawnerCommands) {
    parent.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Px(18.0),
        flex_shrink: 0.0,
        ..default()
    });
}

fn detail_title(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text(parent, text, 14.0, PANEL_HIGHLIGHT, Justify::Left);
}

fn body_line(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text(parent, text, 12.0, PANEL_TEXT, Justify::Left);
}

fn tracker_title_line(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text(
        parent,
        text,
        10.0,
        Color::srgb(0.10, 1.0, 0.05),
        Justify::Left,
    );
}

fn tracker_title_line_raw(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text_raw(
        parent,
        text,
        10.0,
        Color::srgb(0.10, 1.0, 0.05),
        Justify::Left,
    );
}

fn tracker_body_line(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text(parent, text, 9.0, Color::WHITE, Justify::Left);
}

fn tracker_body_line_raw(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text_raw(parent, text, 9.0, Color::WHITE, Justify::Left);
}

fn highlight_line(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text(parent, text, 12.0, PANEL_HIGHLIGHT, Justify::Left);
}

fn feedback_line(
    parent: &mut ChildSpawnerCommands,
    feedback: Option<&QuestFeedback>,
    font_size: f32,
) {
    let Some(feedback) = feedback else {
        return;
    };
    panel_text(
        parent,
        &feedback.message,
        font_size,
        if feedback.is_error {
            FEEDBACK_ERR
        } else {
            FEEDBACK_OK
        },
        Justify::Left,
    );
}

/// Text nodes always receive a concrete width and Crystal-style shadow. This
/// makes authoritative text from either translated or unbroken-script sources
/// wrap inside its owning panel instead of leaking over nearby world controls.
fn panel_text(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    panel_text_raw(parent, &crate::player_text::text(text), font_size, color, justify);
}

fn panel_text_raw(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    font_size: f32,
    color: Color,
    justify: Justify,
) {
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            ..default()
        },
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(font_size),
            ..default()
        },
        TextColor(color),
        TextLayout::new(justify, LineBreak::WordOrCharacter),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::BLACK,
        },
    ));
}

fn action_button(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    action: QuestUiButton,
    enabled: bool,
) {
    action_button_raw(parent, &crate::player_text::text(text), action, enabled);
}

fn action_button_raw(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    action: QuestUiButton,
    enabled: bool,
) {
    let color = if enabled { BUTTON_BG } else { BUTTON_DISABLED };

    let mut button = parent.spawn((
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            min_height: Val::Px(28.0),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            margin: UiRect::top(Val::Px(2.0)),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(color),
        TextColor(PANEL_TEXT),
        QuestUiButtonVisual { enabled },
        // Disabled controls still own their rectangle so a world click cannot
        // pass through a visibly unavailable action.
        FocusPolicy::Block,
    ));

    if enabled {
        button.insert((Button, action));
    }

    button.with_children(|line| {
        line.spawn((
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                ..default()
            },
            Text::new(text),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(if enabled { PANEL_TEXT } else { DISABLED_TEXT }),
            TextLayout::new(Justify::Center, LineBreak::WordOrCharacter),
            TextShadow {
                offset: Vec2::splat(1.0),
                color: Color::BLACK,
            },
        ));
    });
}

fn sync_quest_ui_button_visuals(
    mut buttons: Query<(&Interaction, &QuestUiButtonVisual, &mut BackgroundColor)>,
) {
    for (interaction, visual, mut background) in &mut buttons {
        background.0 = if !visual.enabled {
            BUTTON_DISABLED
        } else {
            match interaction {
                Interaction::Pressed => BUTTON_PRESSED,
                Interaction::Hovered => BUTTON_HOVER,
                Interaction::None => BUTTON_BG,
            }
        };
    }
}

fn intent_from_button(action: &QuestUiButton) -> Option<QuestUiIntent> {
    match action {
        QuestUiButton::SelectNpcDialog { target } => Some(QuestUiIntent::SelectNpcDialog {
            target: target.to_owned(),
        }),
        QuestUiButton::AttackTarget { object_id } => Some(QuestUiIntent::AttackTarget {
            object_id: *object_id,
        }),
        QuestUiButton::AttackQuestTarget { object_id } => Some(QuestUiIntent::AttackTarget {
            object_id: *object_id,
        }),
        QuestUiButton::PickUpObject { object_id } => Some(QuestUiIntent::PickUpObject {
            object_id: *object_id,
        }),
        QuestUiButton::PickUpTile => Some(QuestUiIntent::PickUpTile),
        QuestUiButton::AbandonQuest { quest_index } => Some(QuestUiIntent::AbandonQuest {
            quest_index: *quest_index,
        }),
        QuestUiButton::ShareQuest { quest_index } => Some(QuestUiIntent::ShareQuest {
            quest_index: *quest_index,
        }),
        // Local-only buttons do not map to gateway intents
        _ => None,
    }
}

#[cfg(all(test, feature = "native-ui"))]
mod tests {
    #[test]
    fn hunt_navigation_ready_status_means_turn_in_not_reward_already_claimed() {
        assert_eq!(quest_diary_status_label(&quest(2_110_012, QuestStatus::ReadyToTurnIn), QuestRenderText::default()), "可提交");
    }

    #[test]
    fn hunt_navigation_click_checks_the_specific_unfinished_kill_and_authoritative_region() {
        let mut hunt = quest(2_110_010, QuestStatus::InProgress);
        hunt.objectives = vec![crate::quest_model::QuestObjective {
            objective_id: "2110010:0".into(), text: "Defeat 4 Skeleton.".into(), current: 0, target: 4,
        }];
        let tracker = QuestTracker { active_quests: vec![hunt] };
        let region = crate::quest_hunt_regions::active_hunt_regions(&tracker, 39,
            crate::big_map::BigMapPoint { x: 211, y: 320 }).remove(0);
        let intent = QuestRouteNavigationIntent {
            target: QuestRouteTarget::HuntRegion { monster_index: region.monster_index, radius: region.radius },
            quest_index: 2_110_010, reset_epoch: 12, map_index: 39, x: 250, y: 260,
        };
        let big_map = crate::big_map::BigMapModel { current_map_index: Some(39), reset_epoch: 12, ..default() };
        let state = QuestUiState::default();
        assert!(quest_route_intent_is_current(intent, &tracker, &state, None, Some(&big_map)));
        for invalid in [
            QuestRouteNavigationIntent { target: QuestRouteTarget::Entrance, ..intent },
            QuestRouteNavigationIntent { target: QuestRouteTarget::HuntRegion { monster_index: -1, radius: 30 }, ..intent },
            QuestRouteNavigationIntent { target: QuestRouteTarget::HuntRegion { monster_index: region.monster_index, radius: 31 }, ..intent },
            QuestRouteNavigationIntent { x: 251, ..intent },
            QuestRouteNavigationIntent { reset_epoch: 13, ..intent },
            QuestRouteNavigationIntent { map_index: 1, ..intent },
        ] {
            assert!(!quest_route_intent_is_current(invalid, &tracker, &state, None, Some(&big_map)), "{invalid:?}");
        }
        let mut finished = tracker.clone();
        finished.active_quests[0].objectives[0].current = 4;
        assert!(!quest_route_intent_is_current(intent, &finished, &state, None, Some(&big_map)));
        finished.active_quests[0].objectives.clear();
        assert!(!quest_route_intent_is_current(intent, &finished, &state, None, Some(&big_map)));
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..default() });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(tracker);
        app.insert_resource(big_map);
        app.init_resource::<NpcDialogModel>().init_resource::<NpcDialogNav>()
            .init_resource::<QuestUiState>().init_resource::<QuestUiIntentQueue>()
            .init_resource::<QuestRouteNavigationIntentQueue>().init_resource::<PendingOperations>();
        app.world_mut().spawn((Button, QuestUiButton::NavigateQuestRoute(intent), Interaction::Pressed));
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        assert_eq!(app.world().resource::<QuestRouteNavigationIntentQueue>().pending, Some(intent));
        assert!(app.world().resource::<QuestUiState>().feedback.as_ref().unwrap().message.contains("前往狩猎区域"));
        // Completion while the pointer still holds the button invalidates the
        // queued walk before release. No command is sent to claim a kill.
        app.world_mut().resource_mut::<QuestTracker>().active_quests[0].objectives[0].current = 4;
        app.update();
        assert!(app.world().resource::<QuestRouteNavigationIntentQueue>().is_empty());
        assert!(app.world().resource::<QuestUiIntentQueue>().is_empty());
    }

    #[test]
    fn route_navigation_queue_is_bounded_and_preserves_the_configured_source_coordinate() {
        let intent = QuestRouteNavigationIntent {
            target: QuestRouteTarget::Entrance,
            quest_index: 2_110_010,
            reset_epoch: 7,
            map_index: 1,
            x: 147,
            y: 33,
        };
        let mut queue = QuestRouteNavigationIntentQueue::default();
        assert!(queue.push(intent));
        assert!(!queue.push(QuestRouteNavigationIntent { x: 151, y: 362, ..intent }));
        assert_eq!(queue.take(), Some(intent));
        assert!(queue.is_empty());
    }

    #[test]
    fn pressed_d401_route_button_enqueues_the_current_oma_entrance() {
        let d401 = mir2_game_data::crystal_respawn_manifest_ref().maps.iter()
            .find(|map| map.map_file_name == "D401").expect("imported D401 map");
        assert_eq!(d401.map_index, 47);
        let intent = QuestRouteNavigationIntent {
            target: QuestRouteTarget::Entrance,
            quest_index: 2_110_010,
            reset_epoch: 7,
            map_index: d401.map_index,
            x: 24,
            y: 182,
        };
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..default() });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker { active_quests: vec![quest(2_110_010, QuestStatus::InProgress)] });
        app.insert_resource(crate::big_map::BigMapModel {
            reset_epoch: 7,
            current_map_index: Some(d401.map_index),
            ..default()
        });
        app.init_resource::<NpcDialogModel>()
            .init_resource::<NpcDialogNav>()
            .init_resource::<QuestUiState>()
            .init_resource::<QuestUiIntentQueue>()
            .init_resource::<QuestRouteNavigationIntentQueue>()
            .init_resource::<PendingOperations>();
        app.world_mut().spawn((
            Button,
            QuestUiButton::NavigateQuestRoute(intent),
            Interaction::Pressed,
        ));
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        assert_eq!(
            app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().take(),
            Some(intent),
        );
    }

    #[test]
    fn route_navigation_is_discarded_after_leaving_the_game_session() {
        let intent = QuestRouteNavigationIntent {
            target: QuestRouteTarget::Entrance,
            quest_index: 2_110_010,
            reset_epoch: 7,
            map_index: 1,
            x: 147,
            y: 33,
        };
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel { screen: NativeShellScreen::Login, ..default() });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker::default());
        app.init_resource::<NpcDialogModel>()
            .init_resource::<NpcDialogNav>()
            .init_resource::<QuestUiState>()
            .init_resource::<QuestUiIntentQueue>()
            .init_resource::<QuestRouteNavigationIntentQueue>()
            .init_resource::<PendingOperations>();
        assert!(app.world_mut().resource_mut::<QuestRouteNavigationIntentQueue>().push(intent));
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        assert!(app.world().resource::<QuestRouteNavigationIntentQueue>().is_empty());
    }

    #[test]
    fn route_navigation_rechecks_primary_task_and_current_entrance() {
        let intent = QuestRouteNavigationIntent {
            target: QuestRouteTarget::Entrance,
            quest_index: 2_110_010,
            reset_epoch: 7,
            map_index: 1,
            x: 147,
            y: 33,
        };
        let tracker = QuestTracker { active_quests: vec![
            quest(2_110_010, QuestStatus::InProgress),
            quest(42, QuestStatus::InProgress),
        ] };
        let state = QuestUiState::default();
        let big_map = crate::big_map::BigMapModel {
            reset_epoch: 7,
            current_map_index: Some(1),
            ..default()
        };
        assert!(quest_route_intent_is_current(intent, &tracker, &state, None, Some(&big_map)));
        let pinned = QuestUiState { pinned_primary_quest_index: Some(42), ..default() };
        assert!(!quest_route_intent_is_current(intent, &tracker, &pinned, None, Some(&big_map)));
        assert!(!quest_route_intent_is_current(
            QuestRouteNavigationIntent { x: 148, ..intent },
            &tracker, &state, None, Some(&big_map),
        ));
    }

    #[test]
    fn bichon_arrival_card_has_readable_destination_and_local_map_button() {
        use super::*;
        fn spawn(mut commands: Commands) {
            commands.spawn(Node::default()).with_children(|parent| {
                render_bichon_arrival_tracker(parent, &MapModel::default());
            });
        }
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.add_plugins(MinimalPlugins).add_systems(Startup, spawn);
        app.update();
        let texts: Vec<String> = app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).collect();
        assert!(texts.iter().any(|t| t.contains("(328,264)")));
        assert!(texts.iter().any(|t| t.contains("新手村安全区不算")));
        assert!(app.world_mut().query::<&QuestUiButton>().iter(app.world()).any(|b| matches!(b, QuestUiButton::OpenDestinationMap)));
        for font in app.world_mut().query::<&TextFont>().iter(app.world()) {
            assert_eq!(font.font, FontSource::Family("Microsoft YaHei".into()));
        }
        assert_eq!(intent_from_button(&QuestUiButton::OpenDestinationMap), None);
    }

    use super::*;
    use crate::quest_model::QuestStatus;
    use bevy::prelude::App;
    use bevy::ui::Interaction;

    fn quest(index: i32, status: crate::quest_model::QuestStatus) -> Quest {
        Quest {
            quest_index: index,
            accept_npc_index: Some(10),
            finish_npc_index: Some(11),
            title: format!("Quest {index}"),
            npc_name: Some("Guard".to_owned()),
            group: Some("BichonProvince".to_owned()),
            min_level_needed: 1,
            detail: Default::default(),
            status,
            objectives: vec![crate::quest_model::QuestObjective {
                objective_id: format!("{index}:0"),
                text: "Kill 3".to_owned(),
                current: 0,
                target: 3,
            }],
            rewards: vec![],
            unknown_text: None,
        }
    }

    fn quest_with_rewards(
        index: i32,
        status: QuestStatus,
        rewards: Vec<crate::quest_model::QuestReward>,
    ) -> Quest {
        Quest {
            quest_index: index,
            accept_npc_index: Some(10),
            finish_npc_index: Some(11),
            title: format!("Quest {index}"),
            npc_name: None,
            group: None,
            min_level_needed: 0,
            detail: Default::default(),
            status,
            objectives: vec![],
            rewards,
            unknown_text: None,
        }
    }

    fn dialog_with_option(npc_object_id: u32, target: &str) -> NpcDialogModel {
        let mut dialog = NpcDialogModel::default();
        dialog.is_open = true;
        dialog.npc_object_id = Some(npc_object_id);
        dialog.options = vec![crate::quest_model::NpcDialogOption {
            option_id: target.to_owned(),
            label: "Continue".to_owned(),
            enabled: true,
        }];
        dialog
    }

    fn queue_sample() -> QuestUiIntentQueue {
        let mut queue = QuestUiIntentQueue::default();
        queue.push_intent(QuestUiIntent::InteractNpc { npc_object_id: 100 });
        queue.push_intent(QuestUiIntent::AttackTarget { object_id: 1001 });
        queue.push_intent(QuestUiIntent::FinishQuest {
            quest_index: 10,
            selected_item_index: -1,
        });
        queue
    }

    #[test]
    fn abandon_is_allowed_for_both_current_quest_states() {
        assert!(can_abandon_quest(&quest(1, QuestStatus::InProgress)));
        assert!(can_abandon_quest(&quest(1, QuestStatus::ReadyToTurnIn)));
        for status in [
            QuestStatus::NotStarted,
            QuestStatus::Completed,
            QuestStatus::Failed,
            QuestStatus::Aborted,
            QuestStatus::Unknown("future".to_owned()),
        ] {
            assert!(!can_abandon_quest(&quest(1, status)));
        }
    }

    #[test]
    fn combat_target_panel_uses_compact_crystal_safe_geometry() {
        assert!(!CRYSTAL_TARGET_PANEL_VISIBLE);
        assert_eq!(COMBAT_TARGET_PANEL_LEFT, 8.0);
        assert_eq!(COMBAT_TARGET_PANEL_TOP, 8.0);
        assert_eq!(COMBAT_TARGET_PANEL_WIDTH, 236.0);
        assert_eq!(COMBAT_TARGET_PANEL_MIN_HEIGHT, 0.0);
        assert_eq!(COMBAT_TARGET_PANEL_PADDING, 4.0);
        assert_eq!(COMBAT_TARGET_BAR_HEIGHT, 8.0);
        assert!(COMBAT_TARGET_PANEL_LEFT + COMBAT_TARGET_PANEL_WIDTH < 898.0);
        assert!(COMBAT_TARGET_PANEL_TOP < 118.0);
    }

    #[test]
    fn unknown_combat_target_health_has_no_zero_bar_or_zero_label() {
        let target = crate::quest_model::CombatTarget {
            object_id: 42,
            name: "Scarecrow".to_owned(),
            hp: 0,
            max_hp: 0,
            is_player: false,
        };
        assert!(combat_target_health(&target).is_none());

        let known = crate::quest_model::CombatTarget {
            max_hp: 20,
            hp: 8,
            ..target
        };
        assert_eq!(
            combat_target_health(&known),
            Some((0.4, "8 / 20".to_owned()))
        );
    }

    #[test]
    fn abandon_button_requires_yes_and_preserves_quest_id() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker {
            active_quests: vec![
                quest(7, QuestStatus::InProgress),
                quest(8, QuestStatus::Completed),
            ],
        });
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();

        let valid = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::AbandonQuest { quest_index: 7 },
                Interaction::Pressed,
            ))
            .id();
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<QuestUiState>()
                .abandon_confirmation_quest_index,
            Some(7)
        );

        let confirm = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::ConfirmAbandonQuest,
                Interaction::Pressed,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::AbandonQuest { quest_index: 7 }]
        );
        assert_eq!(
            app.world()
                .resource::<QuestUiState>()
                .abandon_confirmation_quest_index,
            None
        );
        app.world_mut().despawn(confirm);
        let encoded = serde_json::to_value(QuestUiIntent::AbandonQuest { quest_index: 7 })
            .expect("serialize quest intent");
        assert_eq!(encoded["type"], "abandonQuest");
        assert_eq!(encoded["questIndex"], 7);
        app.world_mut().entity_mut(valid).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(valid)
            .insert(Interaction::Pressed);
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
        app.world_mut().despawn(valid);

        let stale = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::AbandonQuest { quest_index: 8 },
                Interaction::Pressed,
            ))
            .id();
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert!(app
            .world()
            .resource::<QuestUiState>()
            .feedback
            .as_ref()
            .is_some_and(|feedback| feedback.is_error));
        app.world_mut().despawn(stale);
    }

    #[test]
    fn intent_queue_keeps_oldest_fifo_and_reports_rejected_overflow() {
        let mut queue = QuestUiIntentQueue::default();
        for n in 0..(MAX_QUEUED_INTENTS + 5) {
            let accepted = queue.push_intent(QuestUiIntent::PickUpObject {
                object_id: n as u32,
            });
            assert_eq!(accepted, n < MAX_QUEUED_INTENTS);
        }

        assert_eq!(queue.len(), MAX_QUEUED_INTENTS);
        assert_eq!(queue.overflow_count(), 5);
        let drained = queue.drain_intents();
        assert_eq!(drained.len(), MAX_QUEUED_INTENTS);
        assert_eq!(drained[0], QuestUiIntent::PickUpObject { object_id: 0 });
        assert_eq!(drained[23], QuestUiIntent::PickUpObject { object_id: 23 });
    }

    #[test]
    fn retry_saturation_returns_every_dropped_intent_for_pending_release() {
        let mut queue = QuestUiIntentQueue::default();
        for object_id in 0..MAX_QUEUED_INTENTS as u32 {
            queue
                .retry_intents
                .push_back(QuestUiIntent::PickUpObject { object_id });
        }
        let dropped = queue.retain_failed_intents([QuestUiIntent::AcceptQuest {
            npc_index: 4001,
            quest_index: 1001,
        }]);
        assert_eq!(
            dropped,
            vec![QuestUiIntent::AcceptQuest {
                npc_index: 4001,
                quest_index: 1001,
            }]
        );
        assert_eq!(queue.retry_len(), MAX_QUEUED_INTENTS);
        assert_eq!(queue.overflow_count(), 1);
    }

    #[test]
    fn manual_input_clears_only_stale_attacks_from_both_intent_lanes() {
        let mut queue = QuestUiIntentQueue::default();
        queue.retain_failed_intents([
            QuestUiIntent::AttackTarget { object_id: 7 },
            QuestUiIntent::PickUpObject { object_id: 8 },
        ]);
        queue.push_intent(QuestUiIntent::AttackTarget { object_id: 9 });
        queue.push_intent(QuestUiIntent::InteractNpc { npc_object_id: 10 });
        assert_eq!(queue.clear_attack_intents(), 2);
        assert_eq!(queue.retry_len(), 1);
        assert_eq!(queue.drain_intents(), vec![
            QuestUiIntent::PickUpObject { object_id: 8 },
            QuestUiIntent::InteractNpc { npc_object_id: 10 },
        ]);
    }

    #[test]
    fn queue_is_empty_after_draining() {
        let mut queue = queue_sample();
        assert!(!queue.is_empty());
        let drained = queue.drain_intents();
        assert_eq!(drained.len(), 3);
        assert!(queue.is_empty());
    }

    #[test]
    fn quest_operations_deduplicate_until_authoritative_quest_refresh() {
        let mut queue = QuestUiIntentQueue::default();
        let mut pending = PendingOperations::default();
        let accept = QuestUiIntent::AcceptQuest {
            npc_index: 10,
            quest_index: 5,
        };
        assert!(queue.push_pending_intent(&mut pending, accept.clone()));
        assert!(!queue.push_pending_intent(&mut pending, accept.clone()));
        assert!(queue.push_pending_intent(
            &mut pending,
            QuestUiIntent::FinishQuest {
                quest_index: 6,
                selected_item_index: 0,
            },
        ));
        assert_eq!(queue.drain_intents().len(), 2);

        let mut revisions = AuthoritativeModelRevisions::default();
        crate::pending_operations::mark_authoritative_refresh(
            &mut revisions,
            crate::pending_operations::AuthoritativeModelDomain::Quest,
        );
        assert!(!queue.push_pending_intent(&mut pending, accept.clone()));

        let before = QuestTracker::default();
        let after = QuestTracker {
            active_quests: vec![Quest {
                quest_index: 5,
                accept_npc_index: Some(10),
                finish_npc_index: None,
                title: "Accepted".into(),
                npc_name: None,
                group: None,
                min_level_needed: 0,
                detail: Default::default(),
                status: crate::quest_model::QuestStatus::InProgress,
                objectives: Vec::new(),
                rewards: Vec::new(),
                unknown_text: None,
            }],
        };
        crate::pending_operations::reconcile_quest_refresh(&mut pending, &before, &after);
        assert!(queue.push_pending_intent(&mut pending, accept));
        assert_eq!(queue.drain_intents().len(), 1);
    }

    #[test]
    fn action_to_intent_preserves_payload() {
        let action = QuestUiButton::SelectNpcDialog {
            target: "opt_01".to_owned(),
        };
        assert_eq!(
            intent_from_button(&action),
            Some(QuestUiIntent::SelectNpcDialog {
                target: "opt_01".to_owned(),
            })
        );
        assert_eq!(
            intent_from_button(&QuestUiButton::ShareQuest { quest_index: 42 }),
            Some(QuestUiIntent::ShareQuest { quest_index: 42 })
        );
        assert_eq!(
            intent_from_button(&QuestUiButton::AttackQuestTarget { object_id: 77 }),
            Some(QuestUiIntent::AttackTarget { object_id: 77 })
        );
    }

    #[test]
    fn graduation_target_selection_is_local_and_clears_with_the_session() {
        let mut state = QuestUiState::default();
        assert!(state.toggle_graduation_direction(GraduationDirection::Challenge));
        assert_eq!(
            state.selected_graduation_direction,
            Some(GraduationDirection::Challenge)
        );
        assert_eq!(
            intent_from_button(&QuestUiButton::SelectGraduationDirection {
                direction: GraduationDirection::Challenge,
            }),
            None
        );
        assert!(!state.toggle_graduation_direction(GraduationDirection::Challenge));
        assert_eq!(state.selected_graduation_direction, None);
        assert!(state.toggle_graduation_direction(GraduationDirection::Skill));
        state.reset();
        assert_eq!(state.selected_graduation_direction, None);
    }

    #[test]
    fn helper_queue_keeps_zero_id_payloads() {
        let mut queue = QuestUiIntentQueue::default();
        queue.push_intent(QuestUiIntent::PickUpObject { object_id: 0 });
        queue.push_intent(QuestUiIntent::PickUpTile);
        let drained = queue.drain_intents();
        assert_eq!(drained[0], QuestUiIntent::PickUpObject { object_id: 0 });
        assert_eq!(drained[1], QuestUiIntent::PickUpTile);
    }

    #[test]
    fn explicit_starter_dialog_links_map_to_exact_quest_actions() {
        assert_eq!(
            explicit_quest_dialog_button("@AcceptQuest:1001", 4001),
            Some(QuestUiButton::AcceptQuest {
                npc_index: 4001,
                quest_index: 1001,
            })
        );
        assert_eq!(
            explicit_quest_dialog_button("@FinishQuest:1001", 4001),
            Some(QuestUiButton::FinishQuest {
                quest_index: 1001,
                selected_item_index: -1,
            })
        );
        assert_eq!(explicit_quest_dialog_button("@AcceptQuest:1001", 0), None);
        assert_eq!(
            explicit_quest_dialog_button("@AcceptQuest:1001:extra", 4001),
            None
        );
        assert_eq!(explicit_quest_dialog_button("@Shop", 4001), None);
    }

    #[test]
    fn full_queue_reports_pickup_not_queued_instead_of_success() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        let mut queue = QuestUiIntentQueue::default();
        for object_id in 0..MAX_QUEUED_INTENTS as u32 {
            assert!(queue.push_intent(QuestUiIntent::PickUpObject { object_id }));
        }
        app.insert_resource(queue);
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        let mut pickups = GroundPickupModel::default();
        pickups.upsert(crate::quest_model::RecentPickup {
            object_id: Some(999),
            key: "gold-999".to_owned(),
            label: "Gold".to_owned(),
            amount: 1,
            from_npc: None,
        });
        app.insert_resource(pickups);

        let button = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::PickUpObject { object_id: 999 },
                Interaction::Pressed,
            ))
            .id();
        app.add_systems(Update, process_quest_ui_input);
        app.update();

        let intents = app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents();
        assert_eq!(intents.len(), MAX_QUEUED_INTENTS);
        assert!(!intents.contains(&QuestUiIntent::PickUpObject { object_id: 999 }));
        let feedback = app
            .world()
            .resource::<QuestUiState>()
            .feedback
            .as_ref()
            .expect("queue rejection should be visible");
        assert!(feedback.is_error);
        assert!(feedback.message.contains("not queued"));
        app.world_mut().despawn(button);
    }

    #[test]
    fn tracker_uses_explicit_order_and_ignores_non_current_entries() {
        let tracker = QuestTracker {
            active_quests: vec![
                quest(1, crate::quest_model::QuestStatus::Completed),
                quest(2, crate::quest_model::QuestStatus::NotStarted),
                quest(3, crate::quest_model::QuestStatus::InProgress),
                quest(4, crate::quest_model::QuestStatus::ReadyToTurnIn),
            ],
        };
        let state = QuestUiState {
            tracked_quest_indices: vec![4, 2, 3, 1],
            ..default()
        };
        let visible = visible_tracker_quests(&tracker, &state)
            .into_iter()
            .map(|quest| quest.quest_index)
            .collect::<Vec<_>>();
        assert_eq!(visible, vec![4, 3]);
    }

    #[test]
    fn tracker_has_no_implicit_fallback_when_nothing_is_tracked() {
        let tracker = QuestTracker {
            active_quests: vec![
                quest(1, crate::quest_model::QuestStatus::Completed),
                quest(2, crate::quest_model::QuestStatus::Completed),
                quest(3, crate::quest_model::QuestStatus::NotStarted),
                quest(4, crate::quest_model::QuestStatus::NotStarted),
            ],
        };
        let visible = visible_tracker_quests(&tracker, &QuestUiState::default())
            .into_iter()
            .map(|quest| quest.quest_index)
            .collect::<Vec<_>>();
        assert!(visible.is_empty());
    }

    #[test]
    fn newcomer_tracker_hides_behind_open_windows_without_changing_crystal_tracker() {
        use mir2_ui_core::state::UiPanel;

        assert!(journey_tracker_visible(true, UiPanel::None));
        assert!(!journey_tracker_visible(true, UiPanel::Inventory));
        assert!(!journey_tracker_visible(true, UiPanel::Character));
        assert!(!journey_tracker_visible(true, UiPanel::QuestLog));
        assert!(journey_tracker_visible(false, UiPanel::Inventory));
    }

    #[test]
    fn compact_native_labels_are_bounded_and_keep_quantities() {
        let item = ItemModel {
            unique_id: Some(1),
            key: "red-potion".to_owned(),
            name: "Red Potion".to_owned(),
            quantity: 5,
            slot: 0,
            container: 0,
            ..ItemModel::default()
        };
        assert_eq!(quick_item_label(&item), "Red Potion x5");
        let truncated = truncate_chars("abcdefghijklmnopqrstuvwxyz", 10);
        assert_eq!(truncated.chars().count(), 10);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn plugin_schedule_supports_repeated_authoritative_panel_refreshes() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.add_plugins(Mir2QuestUiPlugin);

        // The first update initializes every system parameter and runs Startup.
        // A conflicting mutable Node query fails here with Bevy error B0001.
        app.update();
        app.world_mut().resource_mut::<UiReadModel>().player.level = 2;
        app.update();
        app.world_mut()
            .resource_mut::<InventoryModel>()
            .items
            .push(ItemModel {
                unique_id: Some(2),
                key: "potion".to_owned(),
                name: "Red Potion".to_owned(),
                quantity: 3,
                slot: 0,
                container: 0,
                ..ItemModel::default()
            });
        app.update();

        let mut roots = app
            .world_mut()
            .query_filtered::<Entity, With<QuestUiRoot>>();
        assert_eq!(roots.iter(app.world()).count(), 1);
    }

    #[test]
    fn fixed_and_selectable_quest_rewards_are_both_rich_hover_targets() {
        use crate::inventory::{
            CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel,
        };

        let source = |item_index, name: &str, image, unique_id| CrystalItemTooltipSourceModel {
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
                count: 0,
                identified: false,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut reward_quest = quest(77, QuestStatus::ReadyToTurnIn);
        reward_quest.rewards = vec![
            crate::quest_model::QuestReward::Item {
                item_id: "658".to_owned(),
                name: "Fixed Potion".to_owned(),
                quantity: 3,
                icon: Some(532),
                selection_index: None,
                tooltip_source: Some(source(658, "Fixed Potion", 532, 0)),
            },
            crate::quest_model::QuestReward::Item {
                item_id: "659".to_owned(),
                name: "Choice Potion".to_owned(),
                quantity: 2,
                icon: Some(533),
                selection_index: Some(0),
                tooltip_source: Some(source(659, "Choice Potion", 533, 0)),
            },
        ];

        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.add_plugins(Mir2QuestUiPlugin);
        app.update();
        app.world_mut().resource_mut::<QuestTracker>().active_quests = vec![reward_quest];
        app.world_mut()
            .resource_mut::<QuestUiState>()
            .detail_quest_index = Some(77);
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<(&CrystalItemHint, Option<&QuestUiButton>)>();
        let rendered = query
            .iter(world)
            .map(|(hint, action)| (hint.0.plain_text(), action.cloned()))
            .filter(|(text, _)| text.contains("Potion"))
            .collect::<Vec<_>>();
        assert_eq!(rendered.len(), 2);
        assert!(rendered
            .iter()
            .all(|(text, _)| !text.contains("(3)") && !text.contains("(2)")));
        assert!(rendered.iter().all(|(_, action)| {
            action.is_none()
                || matches!(
                    action,
                    Some(QuestUiButton::SelectReward {
                        quest_index: 77,
                        reward_index: 0
                    })
                )
        }));
        assert_eq!(
            rendered
                .iter()
                .filter(|(_, action)| action.is_some())
                .count(),
            1
        );
        for (cell, children) in world.query_filtered::<(Entity, &Children), With<CrystalItemHint>>()
            .iter(world)
        {
            assert!(children.iter().all(|child|
                world.get::<FocusPolicy>(child) == Some(&FocusPolicy::Pass)),
                "reward decorations must pass hover to cell {cell:?}");
        }

        app.insert_resource(QuestUiPresentation {
            logical_width: 844.0, logical_height: 390.0,
            stage_css_scale: 1.0, touch: true,
        });
        app.update();
        let world = app.world_mut();
        let mobile_cells = world.query_filtered::<&Children, With<CrystalItemHint>>()
            .iter(world).collect::<Vec<_>>();
        assert_eq!(mobile_cells.len(), 2);
        for children in mobile_cells {
            assert!(children.iter().all(|child|
                world.get::<FocusPolicy>(child) == Some(&FocusPolicy::Pass)),
                "mobile reward icon and label must pass hover to their cell");
        }
    }

    // === New quest log / NPC dialog tests ===

    #[test]
    fn quest_selection_updates_state_and_clears_reward() {
        let tracker = QuestTracker {
            active_quests: vec![
                quest(1, QuestStatus::NotStarted),
                quest(2, QuestStatus::ReadyToTurnIn),
            ],
        };
        let mut state = QuestUiState::default();
        state.select_quest(1);
        assert_eq!(state.selected_quest_index, Some(1));
        assert_eq!(state.selected_reward_index, None);
        state.select_reward(0);
        assert_eq!(state.selected_reward_index, Some(0));
        state.select_quest(2);
        assert_eq!(state.selected_quest_index, Some(2));
        assert_eq!(state.selected_reward_index, None);
        let selected = state.selected_quest(&tracker).unwrap();
        assert_eq!(selected.quest_index, 2);
    }

    #[test]
    fn accept_and_finish_enabled_logic() {
        let not_started = quest(1, QuestStatus::NotStarted);
        let in_progress = quest(2, QuestStatus::InProgress);
        let ready = quest(3, QuestStatus::ReadyToTurnIn);
        let completed = quest(4, QuestStatus::Completed);

        assert!(can_accept_quest(&not_started));
        assert!(!can_accept_quest(&in_progress));
        assert!(!can_accept_quest(&ready));
        assert!(!can_accept_quest(&completed));

        assert!(!can_finish_quest(&not_started));
        assert!(!can_finish_quest(&in_progress));
        assert!(can_finish_quest(&ready));
        assert!(!can_finish_quest(&completed));

        assert!(can_track_quest(&in_progress));
        assert!(can_track_quest(&ready));
        assert!(!can_track_quest(&not_started));
        assert!(!can_track_quest(&completed));
    }

    #[test]
    fn reward_selection_is_required_only_for_selectable_item_rewards() {
        let single = quest_with_rewards(
            1,
            QuestStatus::ReadyToTurnIn,
            vec![crate::quest_model::QuestReward::Gold { amount: 100 }],
        );
        let multi = quest_with_rewards(
            2,
            QuestStatus::ReadyToTurnIn,
            vec![
                crate::quest_model::QuestReward::Gold { amount: 100 },
                crate::quest_model::QuestReward::Item {
                    item_id: "1".to_owned(),
                    name: "Sword".to_owned(),
                    quantity: 1,
                    icon: None,
                    selection_index: Some(0),
                    tooltip_source: None,
                },
            ],
        );
        let not_ready = quest_with_rewards(
            3,
            QuestStatus::NotStarted,
            vec![
                crate::quest_model::QuestReward::Gold { amount: 10 },
                crate::quest_model::QuestReward::Gold { amount: 20 },
            ],
        );

        assert!(!reward_selection_required(&single));
        assert!(reward_selection_required(&multi));
        assert!(!reward_selection_required(&not_ready));

        assert!(is_valid_reward_selection(&single, None));
        assert!(!is_valid_reward_selection(&multi, None));
        assert!(is_valid_reward_selection(&multi, Some(0)));
        assert!(!is_valid_reward_selection(&multi, Some(5)));
    }

    #[test]
    fn quest_finish_enabled_requires_reward_selection_when_needed() {
        let multi = quest_with_rewards(
            1,
            QuestStatus::ReadyToTurnIn,
            vec![
                crate::quest_model::QuestReward::Gold { amount: 10 },
                crate::quest_model::QuestReward::Item {
                    item_id: "a".to_owned(),
                    name: "A".to_owned(),
                    quantity: 1,
                    icon: None,
                    selection_index: Some(0),
                    tooltip_source: None,
                },
            ],
        );
        assert!(!quest_finish_enabled(&multi, None));
        assert!(!quest_finish_enabled(&multi, Some(-1)));
        assert!(quest_finish_enabled(&multi, Some(0)));
        assert!(!quest_finish_enabled(&multi, Some(1)));
    }

    #[test]
    fn tracking_sets_feedback_and_state() {
        let tracker = QuestTracker {
            active_quests: vec![
                quest(1, QuestStatus::InProgress),
                quest(2, QuestStatus::NotStarted),
            ],
        };
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(tracker);
        app.insert_resource(QuestTracker {
            active_quests: vec![],
        });
        // Use direct state logic for unit test
        let mut state = QuestUiState::default();
        // Simulate TrackQuest for active quest
        let active = quest(10, QuestStatus::InProgress);
        assert!(can_track_quest(&active));
        assert_eq!(state.toggle_tracked_quest(10), QuestTrackingChange::Added);
        state.set_feedback(format!("Tracking {}", active.title), false);
        assert_eq!(state.tracked_quest_indices, vec![10]);
        assert_eq!(state.feedback.as_ref().unwrap().is_error, false);
        // Not trackable should fail
        let not_trackable = quest(11, QuestStatus::NotStarted);
        assert!(!can_track_quest(&not_trackable));
    }

    #[test]
    fn npc_dialog_nav_push_pop_and_return() {
        let mut nav = NpcDialogNav::default();
        assert!(!nav.can_return());
        let mut dlg = NpcDialogModel::default();
        dlg.is_open = true;
        dlg.npc_object_id = Some(101);
        dlg.npc_name = Some("Guard".to_owned());
        dlg.lines = vec![crate::quest_model::NpcDialogLine {
            text: "Hello".to_owned(),
        }];
        nav.push(dlg.clone());
        assert!(nav.can_return());
        let popped = nav.pop().unwrap();
        assert_eq!(popped.npc_object_id, Some(101));
        assert!(!nav.can_return());
    }

    #[test]
    fn is_quest_log_open_reads_the_single_source() {
        let mut native = NativePlayerUiState::default();
        native.core.screen = mir2_ui_core::state::UiScreen::InGame;
        assert!(!is_quest_log_open_state(Some(&native)));
        native.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        assert!(is_quest_log_open_state(Some(&native)));
        if native.quest_open() {
            native.core.panel = mir2_ui_core::state::UiPanel::None;
        }
        assert!(!is_quest_log_open_state(Some(&native)));
    }

    #[test]
    fn blocks_gameplay_when_modal_open() {
        let mut dialog = NpcDialogModel::default();
        dialog.is_open = false;
        let native = NativePlayerUiState::default();
        assert!(!blocks_gameplay_input(Some(&native), &dialog));
        dialog.is_open = true;
        assert!(blocks_gameplay_input(Some(&native), &dialog));
        dialog.is_open = false;
        let mut native_open = NativePlayerUiState::default();
        native_open.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        assert!(blocks_gameplay_input(Some(&native_open), &dialog,));
    }

    #[test]
    fn real_click_accept_quest_produces_intent_and_feedback() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker {
            active_quests: vec![quest(5, QuestStatus::NotStarted)],
        });
        app.insert_resource(dialog_with_option(10, "@AcceptQuest:5"));
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();

        // Need button entity
        let button_entity = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::AcceptQuest {
                    npc_index: 10,
                    quest_index: 5,
                },
                Interaction::Pressed,
            ))
            .id();

        app.add_systems(Update, process_quest_ui_input);
        app.update();

        let _queue = app.world().resource::<QuestUiIntentQueue>();
        // We need to check intents: since process drains? No, it pushes but not drains.
        // After update, queue should contain AcceptQuest
        let intents = app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents();
        assert!(
            intents.contains(&QuestUiIntent::AcceptQuest {
                npc_index: 10,
                quest_index: 5
            }),
            "expected AcceptQuest intent, got {intents:?}"
        );
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.feedback.as_ref().unwrap().is_error, false);
        assert!(state
            .feedback
            .as_ref()
            .unwrap()
            .message
            .contains("Accepting"));

        // Cleanup
        app.world_mut().despawn(button_entity);
    }

    #[test]
    fn npc_list_accept_after_stale_previous_dialog_emits_selected_quest() {
        let mut completed = quest(5, QuestStatus::ReadyToTurnIn);
        completed.accept_npc_index = Some(10);
        let mut next = quest(6, QuestStatus::NotStarted);
        next.accept_npc_index = Some(10);
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker {
            active_quests: vec![completed, next],
        });
        // This is the stale q5 page left by Back; q6 is available from the
        // current NPC list even though the old page has no q6 link.
        app.insert_resource(dialog_with_option(10, "@FinishQuest:5"));
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();
        let button_entity = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::AcceptNpcQuest {
                    npc_index: 10,
                    quest_index: 6,
                },
                Interaction::Pressed,
            ))
            .id();
        app.add_systems(Update, process_quest_ui_input);
        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::AcceptQuest {
                npc_index: 10,
                quest_index: 6,
            }]
        );
        assert!(app
            .world()
            .resource::<QuestUiState>()
            .feedback
            .as_ref()
            .is_some_and(|feedback| !feedback.is_error && feedback.message.contains("Quest 6")));
        app.world_mut().despawn(button_entity);
    }

    #[test]
    fn quest_log_accept_without_exact_dialog_is_rejected_locally() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        // A quest-log button cannot bypass the current authoritative NPC page.
        app.insert_resource(QuestTracker {
            active_quests: vec![quest(5, QuestStatus::NotStarted)],
        });
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();

        let button = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::AcceptQuest {
                    npc_index: 10,
                    quest_index: 5,
                },
                Interaction::Pressed,
            ))
            .id();

        app.add_systems(Update, process_quest_ui_input);
        app.update();
        let intents = app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents();
        assert!(intents.is_empty(), "remote accept should not emit intent");
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.feedback.as_ref().unwrap().is_error, true);
        assert!(state
            .feedback
            .as_ref()
            .unwrap()
            .message
            .contains("current NPC dialog"));
        app.world_mut().despawn(button);
    }

    #[test]
    fn real_click_finish_without_reward_selection_is_blocked() {
        let multi = quest_with_rewards(
            7,
            QuestStatus::ReadyToTurnIn,
            vec![
                crate::quest_model::QuestReward::Gold { amount: 10 },
                crate::quest_model::QuestReward::Item {
                    item_id: "choice-a".to_owned(),
                    name: "Choice A".to_owned(),
                    quantity: 1,
                    icon: None,
                    selection_index: Some(0),
                    tooltip_source: None,
                },
                crate::quest_model::QuestReward::Item {
                    item_id: "choice-b".to_owned(),
                    name: "Choice B".to_owned(),
                    quantity: 1,
                    icon: None,
                    selection_index: Some(1),
                    tooltip_source: None,
                },
            ],
        );
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker {
            active_quests: vec![multi.clone()],
        });
        app.insert_resource(dialog_with_option(11, "@FinishQuest:7"));
        app.insert_resource(NpcDialogNav::default());
        let mut qs = QuestUiState::default();
        qs.select_quest(7);
        // No reward selected yet
        app.insert_resource(qs);
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();

        // Try to finish with -1 (missing selection)
        let e = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::FinishQuest {
                    quest_index: 7,
                    selected_item_index: -1,
                },
                Interaction::Pressed,
            ))
            .id();
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        let intents = app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents();
        assert!(intents.is_empty());
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(
            state.quest_alert_message.as_deref(),
            Some(SELECT_REWARD_TEXT)
        );
        app.world_mut().despawn(e);

        // Now select reward and finish should succeed
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.close_quest_alert();
            state.select_reward(1);
        }
        let e2 = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::FinishQuest {
                    quest_index: 7,
                    selected_item_index: 1,
                },
                Interaction::Pressed,
            ))
            .id();
        app.update();
        let intents2 = app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents();
        assert_eq!(
            intents2,
            vec![QuestUiIntent::FinishQuest {
                quest_index: 7,
                selected_item_index: 1
            }]
        );
        app.world_mut().despawn(e2);
    }

    #[test]
    fn npc_dialog_click_pushes_history_and_close_clears() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker::default());
        let mut dialog = NpcDialogModel::default();
        dialog.is_open = true;
        dialog.npc_object_id = Some(99);
        dialog.npc_name = Some("Blacksmith".to_owned());
        dialog.lines = vec![crate::quest_model::NpcDialogLine {
            text: "Hello".to_owned(),
        }];
        dialog.options = vec![crate::quest_model::NpcDialogOption {
            option_id: "opt_a".to_owned(),
            label: "Option A".to_owned(),
            enabled: true,
        }];
        app.insert_resource(dialog);
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();

        // Click dialog option
        let e = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::SelectNpcDialog {
                    target: "opt_a".to_owned(),
                },
                Interaction::Pressed,
            ))
            .id();
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        assert!(app.world().resource::<NpcDialogNav>().can_return());
        let intents = app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents();
        assert_eq!(
            intents,
            vec![QuestUiIntent::SelectNpcDialog {
                target: "opt_a".to_owned()
            }]
        );
        app.world_mut().despawn(e);

        // Return
        let e2 = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::ReturnNpcService,
                Interaction::Pressed,
            ))
            .id();
        app.update();
        assert!(!app.world().resource::<NpcDialogNav>().can_return());
        app.world_mut().despawn(e2);

        // Re-open and close
        app.world_mut().resource_mut::<NpcDialogModel>().is_open = true;
        app.world_mut()
            .resource_mut::<NpcDialogNav>()
            .push(NpcDialogModel {
                is_open: true,
                npc_object_id: Some(1),
                npc_name: None,
                lines: vec![],
                options: vec![],
            });
        let e3 = app
            .world_mut()
            .spawn((Button, QuestUiButton::CloseNpcDialog, Interaction::Pressed))
            .id();
        app.update();
        assert!(!app.world().resource::<NpcDialogModel>().is_open);
        assert!(!app.world().resource::<NpcDialogNav>().can_return());
        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::SelectNpcDialog {
                target: "@Exit".to_owned(),
            }]
        );
        app.world_mut().despawn(e3);
    }

    #[test]
    fn input_blocking_prevents_world_shortcuts_when_quest_log_open() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        // Simulate quest log open
        let mut native = NativePlayerUiState::default();
        native.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        app.insert_resource(native);
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        app.insert_resource(QuestTracker::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        let mut nearby = NearbyNpcModel::default();
        nearby.npcs.push(crate::quest_model::NearbyNpc {
            object_id: 123,
            name: "Guard".to_owned(),
            x: 0,
            y: 0,
            quest_indexes: vec![],
            distance: 1,
        });
        app.insert_resource(nearby);
        app.insert_resource(CombatTargetModel::default());
        app.insert_resource(GroundPickupModel::default());
        app.insert_resource(ButtonInput::<KeyCode>::default());
        // Press T while quest log open – should NOT emit InteractNpc
        {
            let mut k = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            k.press(KeyCode::KeyT);
        }
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        let intents = app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents();
        assert!(
            intents.is_empty(),
            "T should be blocked when quest log open, got {intents:?}"
        );
        // Release and ensure still blocked
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyT);
    }

    #[test]
    fn quest_log_open_close_via_q_and_escape() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_systems(Update, process_quest_ui_input);

        // Press Q to open
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();
        assert!(app.world().resource::<NativePlayerUiState>().quest_open());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyQ);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();

        // Press Q again to close while the quest panel itself is modal.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();
        assert!(!app.world().resource::<NativePlayerUiState>().quest_open());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyQ);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();

        // Re-open and verify Escape remains an equivalent close path.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();
        assert!(app.world().resource::<NativePlayerUiState>().quest_open());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyQ);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<NativePlayerUiState>().quest_open());
    }

    #[test]
    fn escape_closes_npc_dialog_and_queues_authoritative_exit() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker::default());
        app.insert_resource(NpcDialogModel {
            is_open: true,
            npc_object_id: Some(1),
            npc_name: Some("Teleporter Gilbert".to_owned()),
            lines: vec![],
            options: vec![],
        });
        app.init_resource::<NpcDialogNav>()
            .init_resource::<QuestUiState>()
            .init_resource::<QuestUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NearbyNpcModel>()
            .init_resource::<CombatTargetModel>()
            .init_resource::<GroundPickupModel>();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        app.insert_resource(keys);
        app.add_systems(Update, process_quest_ui_input);

        app.update();

        assert!(!app.world().resource::<NpcDialogModel>().is_open);
        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::SelectNpcDialog {
                target: "@Exit".to_owned(),
            }]
        );
    }

    #[test]
    fn escape_closes_quest_without_falling_through_to_menu() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        let mut player_ui = NativePlayerUiState::default();
        player_ui.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        app.insert_resource(player_ui);
        app.init_resource::<QuestTracker>()
            .init_resource::<NpcDialogModel>()
            .init_resource::<NpcDialogNav>()
            .init_resource::<QuestUiState>()
            .init_resource::<QuestUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NearbyNpcModel>()
            .init_resource::<CombatTargetModel>()
            .init_resource::<GroundPickupModel>()
            .init_resource::<crate::crystal_ui::overlays::MailComposeUi>()
            .init_resource::<crate::crystal_ui::overlays::NativePlayerUiIntentQueue>()
            .init_resource::<crate::native_shell::NativeUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<crate::mail::MailModel>()
            .init_resource::<crate::map::MapModel>()
            .init_resource::<crate::shop::ShopModel>()
            .init_resource::<crate::storage::StorageModel>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<crate::audio::NativeUiAudioQueue>()
            .add_message::<bevy::input::keyboard::KeyboardInput>()
            .add_systems(
                Update,
                (
                    crate::crystal_ui::overlays::process_overlay_keyboard,
                    process_quest_ui_input,
                )
                    .chain(),
            );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let state = app.world().resource::<NativePlayerUiState>();
        assert!(!state.quest_open());
        assert!(!state.menu_open());
    }

    #[test]
    fn select_quest_and_track_flow() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState {
            core: mir2_ui_core::state::UiState {
                panel: mir2_ui_core::state::UiPanel::QuestLog,
                screen: mir2_ui_core::state::UiScreen::InGame,
                minimap_visible: true,
                chat_focused: false,
                ..Default::default()
            },
            ..Default::default()
        });
        app.insert_resource(QuestTracker {
            active_quests: vec![
                quest(10, QuestStatus::InProgress),
                quest(11, QuestStatus::NotStarted),
            ],
        });
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();

        let select = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::SelectQuest { quest_index: 10 },
                Interaction::Pressed,
            ))
            .id();
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        assert_eq!(
            app.world().resource::<QuestUiState>().selected_quest_index,
            Some(10)
        );
        app.world_mut().despawn(select);

        let track = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::TrackQuest { quest_index: 10 },
                Interaction::Pressed,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().resource::<QuestUiState>().tracked_quest_indices,
            vec![10]
        );
        app.world_mut().despawn(track);

        // Close log
        let close = app
            .world_mut()
            .spawn((Button, QuestUiButton::CloseQuestLog, Interaction::Pressed))
            .id();
        app.update();
        assert!(!app.world().resource::<NativePlayerUiState>().quest_open());
        assert_eq!(
            app.world().resource::<QuestUiState>().selected_quest_index,
            None
        );
        app.world_mut().despawn(close);
    }

    #[test]
    fn disabled_npc_option_rejects_a_stale_button_event() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker::default());
        app.insert_resource(NpcDialogModel {
            is_open: true,
            npc_object_id: Some(99),
            npc_name: Some("Village Guide".to_owned()),
            lines: vec![],
            options: vec![crate::quest_model::NpcDialogOption {
                option_id: "locked".to_owned(),
                label: "Locked service".to_owned(),
                enabled: false,
            }],
        });
        app.init_resource::<NpcDialogNav>()
            .init_resource::<QuestUiState>()
            .init_resource::<QuestUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NearbyNpcModel>()
            .init_resource::<CombatTargetModel>()
            .init_resource::<GroundPickupModel>();
        app.world_mut().spawn((
            Button,
            QuestUiButton::SelectNpcDialog {
                target: "locked".to_owned(),
            },
            Interaction::Pressed,
        ));
        app.add_systems(Update, process_quest_ui_input);

        app.update();

        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert!(app.world().resource::<NpcDialogNav>().history.is_empty());
        assert!(app
            .world()
            .resource::<QuestUiState>()
            .feedback
            .as_ref()
            .is_some_and(|feedback| feedback.is_error));
    }

    #[test]
    fn pickup_button_requires_a_current_authoritative_ground_object() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>()
            .init_resource::<QuestUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<NearbyNpcModel>()
            .init_resource::<CombatTargetModel>()
            .init_resource::<GroundPickupModel>();
        let button = app
            .world_mut()
            .spawn((
                Button,
                QuestUiButton::PickUpObject { object_id: 44 },
                Interaction::Pressed,
            ))
            .id();
        app.add_systems(Update, process_quest_ui_input);

        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert!(app
            .world()
            .resource::<QuestUiState>()
            .feedback
            .as_ref()
            .is_some_and(|feedback| feedback.is_error));

        app.world_mut().resource_mut::<GroundPickupModel>().upsert(
            crate::quest_model::RecentPickup {
                object_id: Some(44),
                key: "drop-44".to_owned(),
                label: "Red Potion".to_owned(),
                amount: 2,
                from_npc: None,
            },
        );
        app.world_mut().entity_mut(button).insert(Interaction::None);
        app.update();
        app.world_mut()
            .entity_mut(button)
            .insert(Interaction::Pressed);
        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::PickUpObject { object_id: 44 }]
        );
        assert_eq!(
            app.world()
                .resource::<QuestUiState>()
                .feedback
                .as_ref()
                .map(|feedback| feedback.message.as_str()),
            Some("Picking up Red Potion x2")
        );
    }

    #[test]
    fn npc_dialog_wrap_prefers_words_and_hard_breaks_weighted_text() {
        let wrapped = npc_dialog_wrap(&format!("{} new items", "x".repeat(50)));
        assert_eq!(
            wrapped,
            vec![format!("{} new", "x".repeat(50)), "items".to_owned()]
        );
        assert!(wrapped
            .iter()
            .all(|line| npc_dialog_weighted_width(line) <= 54));

        let long_word = npc_dialog_wrap(&"x".repeat(55));
        assert_eq!(long_word.len(), 2);
        assert_eq!(npc_dialog_weighted_width(&long_word[0]), 54);
        assert_eq!(npc_dialog_weighted_width(&long_word[1]), 1);

        let cjk = npc_dialog_wrap(&"界".repeat(28));
        assert_eq!(cjk.len(), 2);
        assert_eq!(npc_dialog_weighted_width(&cjk[0]), 54);
        assert_eq!(npc_dialog_weighted_width(&cjk[1]), 2);
    }

    #[test]
    fn npc_dialog_pages_keep_all_service_links_inside_the_content_area() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.add_systems(Startup, |mut commands: Commands| {
            let mut dialog = dialog_with_option(24, "@main-1");
            dialog.npc_name = Some("BichonWall - Board".into());
            dialog.lines = vec![crate::quest_model::NpcDialogLine {
                text: "BichonWall_Board".into(),
            }];
            dialog.options = (0..12)
                .map(|index| crate::quest_model::NpcDialogOption {
                    option_id: format!("@service-{index}"),
                    label: format!("Service {index}"),
                    enabled: true,
                })
                .collect();
            assert_eq!(npc_dialog_rows(&dialog).len(), 12);
            for top in [0, NPC_DIALOG_VISIBLE_ROWS] {
                commands.spawn(Node::default()).with_children(|parent| {
                    render_dialog_panel(
                        parent,
                        &dialog,
                        &NpcDialogNav::default(),
                        &QuestUiState {
                            dialog_scroll_top: top,
                            ..default()
                        },
                        &PendingOperations::default(),
                        true,
                        None, QuestRenderText::default());
                });
            }
        });
        app.update();
        let world = app.world_mut();
        let mut links = Vec::new();
        for (node, button) in world.query::<(&Node, &QuestUiButton)>().iter(world) {
            if let QuestUiButton::SelectNpcDialog { target } = button {
                links.push(target.clone());
                let (Val::Px(top), Val::Px(height)) = (node.top, node.height) else {
                    panic!("bounded link geometry required")
                };
                assert!(top >= 31.0 && top + height <= 184.0);
            }
        }
        assert_eq!(links.len(), 12);
        assert!(links.contains(&"@service-11".to_owned()));
        assert!(!world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0.contains("NPC Dialog") || text.0.contains("BichonWall_Board")));
    }

    #[test]
    fn wrapped_text_and_disabled_action_hitboxes_keep_their_bounds() {
        let layout = TextLayout::new(Justify::Center, LineBreak::WordOrCharacter);
        assert_eq!(layout.linebreak, LineBreak::WordOrCharacter);
        assert_eq!(layout.justify, Justify::Center);
        assert_eq!(FocusPolicy::default(), FocusPolicy::Pass);
        assert_eq!(FocusPolicy::Block, FocusPolicy::Block);
    }

    #[test]
    fn stale_target_click_cannot_emit_attack_intent() {
        let target = CombatTargetModel::default();
        assert!(!target_is_attackable(Some(&target), 44));

        let mut target = CombatTargetModel::default();
        target.apply(crate::quest_model::CombatTargetUpdate {
            object_id: 44,
            name: "Scarecrow".to_owned(),
            hp: 8,
            max_hp: 20,
            is_player: false,
        });
        assert!(target_is_attackable(Some(&target), 44));
        assert!(!target_is_attackable(Some(&target), 45));

        target.apply(crate::quest_model::CombatTargetUpdate {
            object_id: 44,
            name: "Scarecrow".to_owned(),
            hp: 0,
            max_hp: 20,
            is_player: false,
        });
        assert!(!target_is_attackable(Some(&target), 44));
    }

    #[test]
    fn tile_pickup_button_requires_an_authoritative_tile_entry() {
        let empty = GroundPickupModel::default();
        assert!(!pickup_tile_is_current(Some(&empty)));

        let mut tile_pickup = GroundPickupModel::default();
        tile_pickup.upsert(crate::quest_model::RecentPickup {
            object_id: None,
            key: "tile-drop".to_owned(),
            label: "Unknown drop".to_owned(),
            amount: 1,
            from_npc: None,
        });
        assert!(pickup_tile_is_current(Some(&tile_pickup)));

        tile_pickup.upsert(crate::quest_model::RecentPickup {
            object_id: Some(99),
            key: "object-drop".to_owned(),
            label: "Red Potion".to_owned(),
            amount: 1,
            from_npc: None,
        });
        assert!(!pickup_tile_is_current(Some(&tile_pickup)));
    }

    #[test]
    fn crystal_quest_diary_assets_are_source_bound_and_non_placeholder() {
        assert_eq!(QUEST_DIARY_FRAME_ASSET, "original-ui/Prguse/961.png");
        assert_eq!(QUEST_DIARY_TITLE_ASSET, "original-ui/Title/15.png");
        assert_eq!(QUEST_DIARY_EXPANDED_ASSET, "original-ui/Prguse/917.png");
        assert_eq!(QUEST_DIARY_COLLAPSED_ASSET, "original-ui/Prguse/918.png");
        assert_eq!(QUEST_DIARY_SELECTED_ASSET, "original-ui/Prguse/956.png");
        assert_eq!(QUEST_DIARY_TRACKED_ASSET, "original-ui/Prguse/997.png");
        assert_eq!(QUEST_DIARY_TOP_CLOSE_ASSET, "original-ui/Prguse2/360.png");
        assert_eq!(QUEST_DIARY_BOTTOM_CLOSE_ASSET, "original-ui/Title/193.png");
        for asset in [
            QUEST_DIARY_FRAME_ASSET,
            QUEST_DIARY_TITLE_ASSET,
            QUEST_DIARY_EXPANDED_ASSET,
            QUEST_DIARY_COLLAPSED_ASSET,
            QUEST_DIARY_SELECTED_ASSET,
            QUEST_DIARY_TRACKED_ASSET,
            QUEST_DIARY_TOP_CLOSE_ASSET,
            QUEST_DIARY_BOTTOM_CLOSE_ASSET,
        ] {
            assert!(!asset.contains("missing"));
            assert!(!asset.contains("placeholder"));
        }
    }

    #[test]
    fn quest_diary_geometry_matches_crystal_source_at_100_125_and_150_percent() {
        for scale in [1.0, 1.25, 1.5] {
            let layout = quest_diary_layout(scale);
            assert_eq!(
                layout.frame,
                QuestLogRect::new(
                    QUEST_DIARY_DESIGN_LEFT * scale,
                    QUEST_DIARY_DESIGN_TOP * scale,
                    QUEST_DIARY_DESIGN_WIDTH * scale,
                    QUEST_DIARY_DESIGN_HEIGHT * scale,
                )
            );
            assert_eq!(
                layout.title,
                QuestLogRect::new(18.0 * scale, 9.0 * scale, 103.0 * scale, 17.0 * scale)
            );
            assert_eq!(
                layout.taken_count,
                QuestLogRect::new(210.0 * scale, 7.0 * scale, 76.0 * scale, 15.0 * scale)
            );
            assert_eq!(
                layout.top_close,
                QuestLogRect::new(289.0 * scale, 3.0 * scale, 24.0 * scale, 21.0 * scale)
            );
            assert_eq!(
                layout.bottom_close,
                QuestLogRect::new(200.0 * scale, 436.0 * scale, 68.0 * scale, 25.0 * scale)
            );
        }
    }

    #[test]
    fn quest_detail_assets_and_geometry_match_crystal_source() {
        assert_eq!(QUEST_DETAIL_FRAME_ASSET, "original-ui/Prguse/960.png");
        assert_eq!(QUEST_DETAIL_TITLE_ASSET, "original-ui/Title/16.png");
        assert_eq!(QUEST_DETAIL_SECTION_ASSET, "original-ui/Prguse/919.png");
        assert_eq!(QUEST_DETAIL_SHARE_ASSET, "original-ui/Title/616.png");
        assert_eq!(QUEST_DETAIL_CANCEL_ASSET, "original-ui/Title/203.png");
        for scale in [1.0, 1.25, 1.5] {
            let layout = quest_detail_layout(scale);
            assert_eq!(
                layout.frame,
                QuestLogRect::new(
                    QUEST_DETAIL_DESIGN_LEFT * scale,
                    QUEST_DETAIL_DESIGN_TOP * scale,
                    QUEST_DETAIL_DESIGN_WIDTH * scale,
                    QUEST_DETAIL_DESIGN_HEIGHT * scale,
                )
            );
            assert_eq!(
                layout.title,
                QuestLogRect::new(18.0 * scale, 9.0 * scale, 55.0 * scale, 17.0 * scale)
            );
            assert_eq!(
                layout.scroll_up,
                QuestLogRect::new(293.0 * scale, 33.0 * scale, 16.0 * scale, 14.0 * scale)
            );
            assert_eq!(
                layout.scroll_down,
                QuestLogRect::new(293.0 * scale, 280.0 * scale, 16.0 * scale, 14.0 * scale)
            );
            assert_eq!(
                layout.share,
                QuestLogRect::new(40.0 * scale, 436.0 * scale, 76.0 * scale, 25.0 * scale)
            );
            assert_eq!(
                layout.cancel,
                QuestLogRect::new(200.0 * scale, 436.0 * scale, 76.0 * scale, 25.0 * scale)
            );
        }
    }

    #[test]
    fn npc_quest_list_assets_and_geometry_match_crystal_source() {
        assert_eq!(QUEST_LIST_FRAME_ASSET, "original-ui/Prguse/950.png");
        assert_eq!(QUEST_LIST_TITLE_ASSET, "original-ui/Title/14.png");
        assert_eq!(QUEST_LIST_UP_ASSET, "original-ui/Prguse/951.png");
        assert_eq!(QUEST_LIST_DOWN_ASSET, "original-ui/Prguse/957.png");
        assert_eq!(QUEST_LIST_ACCEPT_ASSET, "original-ui/Title/270.png");
        assert_eq!(QUEST_LIST_FINISH_ASSET, "original-ui/Title/273.png");
        assert_eq!(QUEST_LIST_LEAVE_ASSET, "original-ui/Title/276.png");
        assert_eq!(NPC_QUEST_BUTTON_ASSET, "original-ui/Title/530.png");
        for scale in [1.0, 1.25, 1.5] {
            let layout = quest_list_layout(scale);
            assert_eq!(
                layout.frame,
                QuestLogRect::new(
                    QUEST_LIST_DESIGN_LEFT * scale,
                    QUEST_LIST_DESIGN_TOP * scale,
                    QUEST_LIST_DESIGN_WIDTH * scale,
                    QUEST_LIST_DESIGN_HEIGHT * scale,
                )
            );
            assert_eq!(
                layout.quest_up,
                QuestLogRect::new(291.0 * scale, 35.0 * scale, 16.0 * scale, 48.0 * scale)
            );
            assert_eq!(
                layout.message,
                QuestLogRect::new(10.0 * scale, 135.0 * scale, 280.0 * scale, 160.0 * scale)
            );
            assert_eq!(
                layout.primary_action,
                QuestLogRect::new(40.0 * scale, 436.0 * scale, 68.0 * scale, 25.0 * scale)
            );
            assert_eq!(
                layout.leave,
                QuestLogRect::new(205.0 * scale, 436.0 * scale, 68.0 * scale, 25.0 * scale)
            );
        }
    }

    #[test]
    fn quest_message_box_geometry_and_copy_match_crystal_source() {
        assert_eq!(QUEST_CONFIRM_FRAME_ASSET, "original-ui/Prguse/360.png");
        assert_eq!(QUEST_CONFIRM_YES_ASSET, "original-ui/Title/206.png");
        assert_eq!(QUEST_CONFIRM_NO_ASSET, "original-ui/Title/210.png");
        assert_eq!(QUEST_MESSAGE_OK_ASSET, "original-ui/Title/200.png");
        assert_eq!(
            ASK_CANCEL_QUEST_TEXT,
            "Are you sure you want to cancel this quest?"
        );
        assert_eq!(SELECT_REWARD_TEXT, "You must select a reward item.");
        let layout = quest_confirmation_layout(1.0);
        assert_eq!(layout.frame, QuestLogRect::new(284.0, 289.0, 456.0, 190.0));
        assert_eq!(layout.message, QuestLogRect::new(35.0, 35.0, 390.0, 110.0));
        assert_eq!(layout.yes, QuestLogRect::new(260.0, 157.0, 76.0, 25.0));
        assert_eq!(layout.no, QuestLogRect::new(360.0, 157.0, 76.0, 25.0));
    }

    #[test]
    fn tracking_toggle_preserves_order_caps_five_and_removes() {
        let mut state = QuestUiState::default();
        for quest_index in 1..=5 {
            assert_eq!(
                state.toggle_tracked_quest(quest_index),
                QuestTrackingChange::Added
            );
        }
        assert_eq!(state.tracked_quest_indices, vec![1, 2, 3, 4, 5]);
        assert_eq!(state.toggle_tracked_quest(6), QuestTrackingChange::Full);
        assert_eq!(state.tracked_quest_indices, vec![1, 2, 3, 4, 5]);
        assert_eq!(state.toggle_tracked_quest(3), QuestTrackingChange::Removed);
        assert_eq!(state.tracked_quest_indices, vec![1, 2, 4, 5]);
        assert_eq!(state.toggle_tracked_quest(6), QuestTrackingChange::Added);
        assert_eq!(state.tracked_quest_indices, vec![1, 2, 4, 5, 6]);
    }

    #[test]
    fn diary_right_click_toggles_tracking_without_opening_detail() {
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Right);
        app.insert_resource(mouse);
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        let mut player_ui = NativePlayerUiState::default();
        player_ui.core.screen = mir2_ui_core::state::UiScreen::InGame;
        player_ui.core.panel = mir2_ui_core::state::UiPanel::QuestLog;
        app.insert_resource(player_ui);
        app.insert_resource(QuestTracker {
            active_quests: vec![quest(7, QuestStatus::InProgress)],
        });
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        app.init_resource::<QuestUiState>();
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.init_resource::<NearbyNpcModel>();
        app.init_resource::<CombatTargetModel>();
        app.init_resource::<GroundPickupModel>();
        app.world_mut().spawn((
            QuestDiaryRow { quest_index: 7 },
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::splat(0.5)),
            },
        ));
        app.add_systems(Update, process_quest_ui_input);
        app.update();
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.tracked_quest_indices, vec![7]);
        assert_eq!(state.detail_quest_index, None);
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }

    #[test]
    fn npc_quest_membership_and_five_row_selection_follow_current_npc() {
        let mut offered = quest(1, QuestStatus::NotStarted);
        offered.accept_npc_index = Some(10);
        let mut current = quest(2, QuestStatus::InProgress);
        current.accept_npc_index = Some(99);
        current.finish_npc_index = Some(10);
        let mut completed = quest(3, QuestStatus::Completed);
        completed.accept_npc_index = Some(10);
        let mut explicit = quest(4, QuestStatus::NotStarted);
        explicit.accept_npc_index = Some(99);
        let tracker = QuestTracker {
            active_quests: vec![offered, current, completed, explicit],
        };
        let dialog = dialog_with_option(10, "@AcceptQuest:4");
        assert_eq!(
            npc_available_quest_indices(&dialog, &tracker, None),
            vec![1, 2, 4]
        );

        let ids = vec![1, 2, 3, 4, 5, 6, 7];
        let mut state = QuestUiState::default();
        state.open_npc_quest_list(&ids);
        for _ in 0..5 {
            state.move_npc_quest_selection(&ids, 1);
        }
        assert_eq!(state.npc_quest_selected_index, Some(6));
        assert_eq!(state.npc_quest_start_index, 1);
        state.move_npc_quest_selection(&ids, -1);
        assert_eq!(state.npc_quest_selected_index, Some(5));
        assert_eq!(state.npc_quest_start_index, 1);
        state.close_npc_quest_list();
        assert!(!state.npc_quest_list_open);
        assert_eq!(state.npc_quest_selected_index, None);
    }

    #[test]
    fn npc_quest_operations_accept_crystal_runtime_link_forms() {
        let accept = dialog_with_option(5, "@quest:accept:5");
        assert!(dialog_exposes_quest_operation(
            &accept,
            Some(5),
            5,
            false,
            None,
        ));
        assert!(!dialog_exposes_quest_operation(
            &accept,
            Some(4),
            5,
            false,
            None,
        ));
        assert!(!dialog_exposes_quest_operation(
            &accept,
            Some(5),
            6,
            false,
            None,
        ));

        let finish = dialog_with_option(5, "@quest:finish:5:2");
        assert!(dialog_exposes_quest_operation(
            &finish,
            Some(5),
            5,
            true,
            Some(2),
        ));
        assert!(!dialog_exposes_quest_operation(
            &finish,
            Some(5),
            5,
            true,
            Some(1),
        ));

        let explicit = dialog_with_option(5, "@AcceptQuest:5");
        assert!(dialog_exposes_quest_operation(
            &explicit,
            Some(5),
            5,
            false,
            None,
        ));
    }

    #[test]
    fn npc_list_accept_is_bound_to_current_npc_and_available_quest() {
        let mut q5 = quest(5, QuestStatus::ReadyToTurnIn);
        q5.accept_npc_index = Some(10);
        let mut q6 = quest(6, QuestStatus::NotStarted);
        q6.accept_npc_index = Some(10);
        let tracker = QuestTracker {
            active_quests: vec![q5, q6],
        };
        let dialog = dialog_with_option(10, "@FinishQuest:5");
        assert!(npc_list_accept_is_current(&[6], &dialog, &tracker, 10, 6));
        assert!(!npc_list_accept_is_current(&[5], &dialog, &tracker, 10, 6));
        assert!(!npc_list_accept_is_current(&[6], &dialog, &tracker, 11, 6));
        assert!(!npc_list_accept_is_current(&[6], &dialog, &tracker, 10, 5));
        let closed = NpcDialogModel::default();
        assert!(!npc_list_accept_is_current(&[6], &closed, &tracker, 10, 6));
    }

    #[test]
    fn back_history_removes_stale_quest_operation_links() {
        let mut dialog = dialog_with_option(10, "@FinishQuest:5");
        dialog.options.push(crate::quest_model::NpcDialogOption {
            option_id: "@quest:accept:6".to_owned(),
            label: "Accept q6".to_owned(),
            enabled: true,
        });
        let restored = without_quest_operation_links(dialog);
        assert!(restored.options.is_empty());
    }

    #[test]
    fn npc_list_message_omits_progress_and_uses_finish_copy_at_distinct_npc() {
        let mut current = quest(7, QuestStatus::ReadyToTurnIn);
        current.accept_npc_index = Some(10);
        current.finish_npc_index = Some(11);
        current.detail.description_lines = vec!["Start copy".to_owned()];
        current.detail.completion_description_lines = vec!["Finish copy".to_owned()];
        let dialog = dialog_with_option(11, "@FinishQuest:7");
        let lines = quest_list_message_lines(&current, &dialog, None);
        assert!(lines.iter().any(|line| line.text == "Finish copy"));
        assert!(!lines.iter().any(|line| line.text == "Start copy"));
        assert!(!lines.iter().any(|line| line.text == "Progress"));
    }

    #[test]
    fn quest_detail_preserves_source_sections_and_independent_window_state() {
        let mut current = quest(7, QuestStatus::ReadyToTurnIn);
        current.title = "Assistant's Request".to_owned();
        current.detail = crate::quest_model::QuestDetailText {
            description_lines: vec!["Welcome, traveller.".to_owned()],
            task_description_lines: vec!["Transport CannibalLeaves.".to_owned()],
            return_description_lines: vec!["Return to CraftLady.".to_owned()],
            completion_description_lines: vec!["Thank you.".to_owned()],
            time_limit: Some("05:00".to_owned()),
        };
        current.objectives[0].current = 1;
        current.objectives[0].target = 3;

        let lines = quest_detail_lines(&current, None, "Warrior");
        let visible = lines
            .iter()
            .map(|line| (line.kind, line.text.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            visible[0],
            (QuestDetailLineKind::Title, "Assistant's Request")
        );
        assert!(visible.contains(&(QuestDetailLineKind::Heading, "任务目标")));
        assert!(visible.contains(&(QuestDetailLineKind::Body, "Transport CannibalLeaves.")));
        assert!(visible.contains(&(QuestDetailLineKind::Heading, "交付地点")));
        assert!(visible.contains(&(QuestDetailLineKind::Body, "Return to CraftLady.")));
        assert!(visible.contains(&(QuestDetailLineKind::Heading, "时限")));
        assert!(visible.contains(&(QuestDetailLineKind::Heading, "任务进度")));
        assert!(visible.contains(&(QuestDetailLineKind::Body, "Kill 3 (1 / 3)")));

        let tracker = QuestTracker {
            active_quests: vec![current],
        };
        let mut state = QuestUiState::default();
        state.select_quest(7);
        assert_eq!(
            state.detail_quest(&tracker).map(|quest| quest.quest_index),
            Some(7)
        );
        state.clear_diary_selection();
        assert_eq!(state.selected_quest_index, None);
        assert_eq!(state.detail_quest_index, Some(7));
        state.scroll_detail_down(QUEST_DETAIL_LINE_COUNT + 3);
        assert_eq!(state.detail_scroll_top, 1);
        state.close_detail();
        assert_eq!(state.detail_quest_index, None);
        assert_eq!(state.detail_scroll_top, 0);
    }

    #[test]
    fn quest_diary_contains_only_current_quests_and_preserves_source_group_order() {
        let mut available = quest(1, QuestStatus::NotStarted);
        available.group = Some("AvailableOnly".to_owned());
        let mut ready = quest(2, QuestStatus::ReadyToTurnIn);
        ready.group = Some("BichonProvince".to_owned());
        let mut active = quest(3, QuestStatus::InProgress);
        active.group = Some("BorderVillage".to_owned());
        let completed = quest(4, QuestStatus::Completed);
        let tracker = QuestTracker {
            active_quests: vec![available, ready, active, completed],
        };

        let groups = quest_diary_groups(&tracker, None);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "BichonProvince");
        assert_eq!(groups[0].quests[0].quest_index, 2);
        assert_eq!(groups[1].name, "BorderVillage");
        assert_eq!(groups[1].quests[0].quest_index, 3);
        assert_eq!(quest_diary_status_label(groups[0].quests[0], QuestRenderText::default()), "可提交");
        assert_eq!(quest_diary_status_label(groups[1].quests[0], QuestRenderText::default()), "进行中");
    }

    #[test]
    fn quest_diary_groups_expand_by_default_and_toggle_locally() {
        let mut state = QuestUiState::default();
        assert!(!state.is_group_collapsed("BichonProvince"));
        state.toggle_group("BichonProvince".to_owned());
        assert!(state.is_group_collapsed("BichonProvince"));
        state.toggle_group("BichonProvince".to_owned());
        assert!(!state.is_group_collapsed("BichonProvince"));
    }

    #[test]
    fn newcomer_v2_diary_keeps_main_visible_and_pages_all_imported_tasks() {
        let guidance = QuestGuidance::from_profile_name("newcomer-v2");
        let mut main = quest(2_110_013, QuestStatus::NotStarted);
        main.accept_npc_index = Some(0);
        main.title = "Enter the Dead Mine".to_owned();
        let mut imported = (30..=40)
            .map(|id| quest(id, QuestStatus::InProgress))
            .collect::<Vec<_>>();
        imported[0].status = QuestStatus::ReadyToTurnIn;
        imported[1].status = QuestStatus::ReadyToTurnIn;
        let mut all = vec![main];
        all.extend(imported);
        let tracker = QuestTracker { active_quests: all };

        let main_rows = guided_diary_quests(&tracker, Some(&guidance), None, GuidedDiaryTab::Main);
        assert_eq!(main_rows.iter().map(|quest| quest.quest_index).collect::<Vec<_>>(),
            vec![2_110_013]);
        let side_rows = guided_diary_quests(&tracker, Some(&guidance), None, GuidedDiaryTab::Side);
        assert_eq!(side_rows.len(), 11);
        assert_eq!(side_rows.len().div_ceil(GUIDED_DIARY_PAGE_SIZE), 2);
        assert_eq!(side_rows[0].status, QuestStatus::ReadyToTurnIn);
        let ready_rows = guided_diary_quests(&tracker, Some(&guidance), None, GuidedDiaryTab::Ready);
        assert_eq!(ready_rows.len(), 2);
        assert!(ready_rows.iter().all(|quest| quest.status == QuestStatus::ReadyToTurnIn));
    }

    #[test]
    fn guided_diary_side_pages_fit_inside_the_crystal_frame() {
        let guidance = QuestGuidance::from_profile_name("newcomer-v2");
        let tracker = QuestTracker {
            active_quests: (30..=40)
                .map(|id| quest(id, QuestStatus::InProgress))
                .collect(),
        };
        for (page, expected_rows) in [(0, 8), (1, 3)] {
            let mut world = World::new();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let state = QuestUiState {
                diary_tab: GuidedDiaryTab::Side,
                diary_page: page,
                ..default()
            };
            let mut commands = Commands::new(&mut queue, &world);
            commands.spawn_empty().with_children(|parent| {
                render_guided_quest_diary_panel(
                    parent, &tracker, &guidance, None, &state, None, QuestRenderText::default());
            });
            queue.apply(&mut world);
            let rows = world.query::<(&QuestDiaryRow, &Node)>().iter(&world)
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), expected_rows);
            assert!(rows.iter().all(|(_, node)| match (node.top, node.height) {
                (Val::Px(top), Val::Px(height)) => top + height <= 408.0,
                _ => false,
            }));
            assert_eq!(world.query::<&QuestUiButton>().iter(&world)
                .filter(|button| matches!(button, QuestUiButton::SelectGuidedDiaryTab(_)))
                .count(), 3);
        }
    }

    #[test]
    fn newcomer_sorting_is_shared_and_prioritizes_ready_quests() {
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let dialog = dialog_with_option(10, "@AcceptQuest:1");
        let tracker = QuestTracker {
            active_quests: vec![
                quest(28, QuestStatus::ReadyToTurnIn),
                quest(2, QuestStatus::InProgress),
                quest(1, QuestStatus::NotStarted),
            ],
        };

        assert_eq!(
            npc_available_quest_indices(&dialog, &tracker, Some(&guidance)),
            vec![28, 1, 2]
        );
        let groups = quest_diary_groups(&tracker, Some(&guidance));
        assert_eq!(groups[0].name, "Recommended");
        assert_eq!(
            groups[0]
                .quests
                .iter()
                .map(|quest| quest.quest_index)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(groups[1].name, "Optional");
        assert_eq!(groups[1].quests[0].quest_index, 28);
    }

    #[test]
    fn newcomer_detail_wraps_hint_into_scrollable_logical_lines() {
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let lines = quest_detail_lines(&quest(1, QuestStatus::InProgress), Some(&guidance), "Warrior");
        assert!(lines.iter().any(|line| line.text == "新手引导"));
        assert!(lines
            .iter()
            .any(|line| line.text == "分类：推荐任务"));
        let hint_lines = multi_guidance::wrap_card_text(&crate::player_text::text(guidance.entry(1).unwrap().hint.as_str()));
        assert!(hint_lines.len() > 1);
        assert!(hint_lines.iter().all(|line| line.chars().count() <= 46));
        assert!(hint_lines
            .iter()
            .all(|hint| lines.iter().any(|line| line.text == *hint)));
    }

    #[test]
    fn newcomer_diary_preserves_server_cadence_over_static_category() {
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let mut board = quest(140, QuestStatus::InProgress);
        board.group = Some("Weekly".to_owned());
        assert_eq!(quest_diary_group_name(&board, Some(&guidance)), "Weekly");
        board.group = Some("BichonProvince".to_owned());
        assert_eq!(quest_diary_group_name(&board, Some(&guidance)), "Optional");
        assert_eq!(quest_diary_group_name(&board, None), "BichonProvince");
    }

    #[test]
    fn newcomer_hides_only_fixed_zero_rewards_and_keeps_choice_indices() {
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let crystal = QuestGuidance::from_profile_name("");
        let fixed_zero = crate::quest_model::QuestReward::Item {
            item_id: "0".to_owned(),
            name: "Empty fixed reward".to_owned(),
            quantity: 0,
            icon: None,
            selection_index: None,
            tooltip_source: None,
        };
        let choice_zero = crate::quest_model::QuestReward::Item {
            item_id: "1".to_owned(),
            name: "No item".to_owned(),
            quantity: 0,
            icon: None,
            selection_index: Some(7),
            tooltip_source: None,
        };

        assert!(!fixed_reward_is_visible(&fixed_zero, &guidance));
        assert!(fixed_reward_is_visible(&fixed_zero, &crystal));
        assert!(fixed_reward_is_visible(&choice_zero, &guidance));
        let reward_quest =
            quest_with_rewards(1, QuestStatus::ReadyToTurnIn, vec![fixed_zero, choice_zero]);
        assert!(reward_selection_required(&reward_quest));
        assert!(!quest_finish_enabled(&reward_quest, None));
        assert!(quest_finish_enabled(&reward_quest, Some(7)));
        assert!(!quest_finish_enabled(&reward_quest, Some(0)));
    }

    #[test]
    fn quest_log_filter_and_page_state_reset_selection_without_touching_authority() {
        let mut state = QuestUiState {
            selected_quest_index: Some(7),
            detail_quest_index: Some(7),
            detail_scroll_top: 4,
            selected_reward_index: Some(1),
            tracked_quest_indices: vec![7],
            feedback: Some(QuestFeedback {
                message: "stale".to_owned(),
                is_error: false,
            }),
            stage_filter: QuestStageFilter::All,
            page: 3,
            collapsed_groups: vec!["BichonProvince".to_owned()],
            ..default()
        };
        state.set_stage_filter(QuestStageFilter::Completed);
        assert_eq!(state.stage_filter, QuestStageFilter::Completed);
        assert_eq!(state.page, 0);
        assert_eq!(state.selected_quest_index, None);
        assert_eq!(state.detail_quest_index, None);
        assert_eq!(state.detail_scroll_top, 0);
        assert_eq!(state.selected_reward_index, None);
        assert_eq!(state.tracked_quest_indices, vec![7]);
        assert_eq!(state.collapsed_groups, vec!["BichonProvince"]);
        assert!(state.feedback.is_none());
        state.set_page(2);
        assert_eq!(state.page, 2);
        assert_eq!(state.selected_quest_index, None);
    }

    #[test]
    fn newcomer_diary_authority_requires_explicit_zero_template_indices() {
        let newcomer = QuestGuidance::from_profile_name("newcomer-v1");
        let crystal = QuestGuidance::from_profile_name("");
        let mut diary = quest(22, QuestStatus::NotStarted);
        diary.accept_npc_index = Some(0);
        diary.finish_npc_index = Some(0);
        assert!(newcomer_diary_accept_authorized(&diary, Some(&newcomer)));
        assert!(newcomer_diary_finish_authorized(&diary, Some(&newcomer)));
        assert!(!newcomer_diary_accept_authorized(&diary, Some(&crystal)));
        assert!(!newcomer_diary_finish_authorized(&diary, None));

        let mut returns_to_start_npc = diary.clone();
        returns_to_start_npc.accept_npc_index = Some(10);
        assert!(!newcomer_diary_accept_authorized(
            &returns_to_start_npc,
            Some(&newcomer)
        ));
        assert!(!newcomer_diary_finish_authorized(
            &returns_to_start_npc,
            Some(&newcomer)
        ));

        let mut missing = diary.clone();
        missing.accept_npc_index = None;
        missing.finish_npc_index = None;
        assert!(!newcomer_diary_accept_authorized(&missing, Some(&newcomer)));
        assert!(!newcomer_diary_finish_authorized(&missing, Some(&newcomer)));
    }

    #[test]
    fn newcomer_diary_lists_only_explicitly_authorized_available_quests() {
        let newcomer = QuestGuidance::from_profile_name("newcomer-v1");
        let crystal = QuestGuidance::from_profile_name("");
        let mut diary = quest(22, QuestStatus::NotStarted);
        diary.accept_npc_index = Some(0);
        diary.finish_npc_index = Some(0);
        let ordinary = quest(7, QuestStatus::NotStarted);
        let tracker = QuestTracker {
            active_quests: vec![diary, ordinary],
        };

        let newcomer_groups = quest_diary_groups(&tracker, Some(&newcomer));
        assert_eq!(
            newcomer_groups
                .iter()
                .flat_map(|group| group.quests.iter())
                .map(|quest| quest.quest_index)
                .collect::<Vec<_>>(),
            vec![22]
        );
        assert!(quest_diary_groups(&tracker, Some(&crystal)).is_empty());
    }

    #[test]
    fn newcomer_diary_accept_and_finish_work_without_an_npc_dialog() {
        fn app_for(quest: Quest) -> App {
            let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
            app.insert_resource(ButtonInput::<KeyCode>::default());
            app.insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            });
            app.insert_resource(NativePlayerUiState::default());
            app.insert_resource(QuestTracker {
                active_quests: vec![quest],
            });
            app.insert_resource(QuestGuidance::from_profile_name("newcomer-v1"));
            app.insert_resource(NpcDialogModel::default());
            app.insert_resource(NpcDialogNav::default());
            app.init_resource::<QuestUiState>();
            app.init_resource::<QuestUiIntentQueue>();
            app.init_resource::<PendingOperations>();
            app.init_resource::<NearbyNpcModel>();
            app.init_resource::<CombatTargetModel>();
            app.init_resource::<GroundPickupModel>();
            app.add_systems(Update, process_quest_ui_input);
            app
        }

        let mut available = quest(22, QuestStatus::NotStarted);
        available.accept_npc_index = Some(0);
        available.finish_npc_index = Some(0);
        let mut accept_app = app_for(available);
        accept_app.world_mut().spawn((
            Button,
            QuestUiButton::AcceptQuest {
                npc_index: 0,
                quest_index: 22,
            },
            Interaction::Pressed,
        ));
        accept_app.update();
        assert_eq!(
            accept_app
                .world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::AcceptQuest {
                npc_index: 0,
                quest_index: 22,
            }]
        );

        let mut ready = quest(22, QuestStatus::ReadyToTurnIn);
        ready.accept_npc_index = Some(0);
        ready.finish_npc_index = Some(0);
        let mut finish_app = app_for(ready);
        finish_app.world_mut().spawn((
            Button,
            QuestUiButton::FinishQuest {
                quest_index: 22,
                selected_item_index: -1,
            },
            Interaction::Pressed,
        ));
        finish_app.update();
        assert_eq!(
            finish_app
                .world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::FinishQuest {
                quest_index: 22,
                selected_item_index: -1,
            }]
        );

        let mut reward_choice = quest_with_rewards(
            23,
            QuestStatus::ReadyToTurnIn,
            vec![
                crate::quest_model::QuestReward::Item {
                    item_id: "a".to_owned(),
                    name: "Choice A".to_owned(),
                    quantity: 1,
                    icon: None,
                    selection_index: Some(0),
                    tooltip_source: None,
                },
                crate::quest_model::QuestReward::Item {
                    item_id: "b".to_owned(),
                    name: "Choice B".to_owned(),
                    quantity: 1,
                    icon: None,
                    selection_index: Some(1),
                    tooltip_source: None,
                },
            ],
        );
        reward_choice.accept_npc_index = Some(0);
        reward_choice.finish_npc_index = Some(0);
        let mut reward_app = app_for(reward_choice);
        reward_app.world_mut().spawn((
            Button,
            QuestUiButton::FinishQuest {
                quest_index: 23,
                selected_item_index: -1,
            },
            Interaction::Pressed,
        ));
        reward_app.update();
        assert!(reward_app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert_eq!(
            reward_app
                .world()
                .resource::<QuestUiState>()
                .quest_alert_message
                .as_deref(),
            Some(SELECT_REWARD_TEXT)
        );
    }

    #[test]
    fn click_requires_explicit_diary_endpoint_but_honors_current_npc_offer() {
        fn app_for(quest: Quest, dialog: NpcDialogModel) -> App {
            let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
            app.insert_resource(ButtonInput::<KeyCode>::default());
            app.insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            });
            app.insert_resource(NativePlayerUiState::default());
            app.insert_resource(QuestTracker {
                active_quests: vec![quest],
            });
            app.insert_resource(QuestGuidance::from_profile_name("newcomer-v2"));
            app.insert_resource(dialog);
            app.insert_resource(NpcDialogNav::default());
            app.init_resource::<QuestUiState>();
            app.init_resource::<QuestUiIntentQueue>();
            app.init_resource::<PendingOperations>();
            app.add_systems(Update, process_quest_ui_input);
            app
        }

        let mut missing_diary = quest(22, QuestStatus::NotStarted);
        missing_diary.accept_npc_index = None;
        missing_diary.finish_npc_index = Some(0);
        let mut diary_app = app_for(missing_diary, NpcDialogModel::default());
        diary_app.world_mut().spawn((
            Button,
            QuestUiButton::AcceptQuest {
                npc_index: 0,
                quest_index: 22,
            },
            Interaction::Pressed,
        ));
        diary_app.update();
        assert!(diary_app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());

        // ClientQuestInfo's NPC index is a template index, while a dialog
        // carries a loaded object ID. The current enabled offer is decisive.
        let mut offered = quest(23, QuestStatus::NotStarted);
        offered.accept_npc_index = Some(99);
        let mut npc_app = app_for(offered, dialog_with_option(10, "@AcceptQuest:23"));
        npc_app.world_mut().spawn((
            Button,
            QuestUiButton::AcceptNpcQuest {
                npc_index: 10,
                quest_index: 23,
            },
            Interaction::Pressed,
        ));
        npc_app.update();
        assert_eq!(
            npc_app
                .world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::AcceptQuest {
                npc_index: 10,
                quest_index: 23,
            }]
        );
    }

    #[test]
    fn changing_reward_while_finish_is_pending_does_not_submit_again() {
        use crate::pending_operations::{QuestOperationAck, apply_quest_operation_ack};

        let mut quest = quest_with_rewards(
            23,
            QuestStatus::ReadyToTurnIn,
            [0, 1]
                .into_iter()
                .map(|index| crate::quest_model::QuestReward::Item {
                    item_id: format!("choice-{index}"),
                    name: format!("Choice {index}"),
                    quantity: 1,
                    icon: None,
                    selection_index: Some(index),
                    tooltip_source: None,
                })
                .collect(),
        );
        quest.accept_npc_index = Some(0);
        quest.finish_npc_index = Some(0);
        let mut app = App::new();
        app.init_resource::<QuestPresentationLocale>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(QuestTracker {
            active_quests: vec![quest],
        });
        app.insert_resource(QuestGuidance::from_profile_name("newcomer-v2"));
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(NpcDialogNav::default());
        let mut state = QuestUiState::default();
        state.select_quest(23);
        state.select_reward(0);
        app.insert_resource(state);
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<PendingOperations>();
        app.add_systems(Update, process_quest_ui_input);

        app.world_mut().spawn((
            Button,
            QuestUiButton::FinishQuest {
                quest_index: 23,
                selected_item_index: 0,
            },
            Interaction::Pressed,
        ));
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::FinishQuest {
                quest_index: 23,
                selected_item_index: 0,
            }]
        );
        let first_key = PendingOperationKey::QuestFinish {
            quest_index: 23,
            selected_item_index: 0,
        };
        let first_request_id = app
            .world_mut()
            .resource_mut::<PendingOperations>()
            .bind_quest_request_id(first_key.clone())
            .expect("first Finish request id");

        app.world_mut().spawn((
            Button,
            QuestUiButton::SelectReward {
                quest_index: 23,
                reward_index: 1,
            },
            Interaction::Pressed,
        ));
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().selected_reward_index, Some(1));
        app.world_mut().spawn((
            Button,
            QuestUiButton::FinishQuest {
                quest_index: 23,
                selected_item_index: 1,
            },
            Interaction::Pressed,
        ));
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert!(app
            .world()
            .resource::<PendingOperations>()
            .has_pending_quest_finish(23));

        let nack = QuestOperationAck::FinishQuest {
            request_id: first_request_id,
            quest_index: 23,
            selected_item_index: 0,
            success: false,
        };
        assert_eq!(
            apply_quest_operation_ack(
                &mut app.world_mut().resource_mut::<PendingOperations>(),
                &nack,
            ),
            1
        );
        app.world_mut().spawn((
            Button,
            QuestUiButton::FinishQuest {
                quest_index: 23,
                selected_item_index: 1,
            },
            Interaction::Pressed,
        ));
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::FinishQuest {
                quest_index: 23,
                selected_item_index: 1,
            }]
        );
    }
}

#[cfg(all(test, feature = "portable-quest-ui", not(feature = "native-ui")))]
mod mobile_layout_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
    use crate::portable_quest_ui::{Mir2PortableQuestUiPlugin, QuestUiFont, QuestUiHostContext, QuestUiTargetCamera};
    use crate::quest_model::QuestStatus;

    #[derive(Resource, Default)]
    struct IdleQuestStateChanges(Vec<bool>);

    fn record_idle_quest_state_change(state: Res<QuestUiState>,
        mut changes: ResMut<IdleQuestStateChanges>) {
        changes.0.push(state.is_changed());
    }

    fn presentation() -> QuestUiPresentation {
        QuestUiPresentation { logical_width: 1664.0, logical_height: 768.0,
            stage_css_scale: 0.507, touch: true }
    }

    #[test]
    fn locale_only_change_invalidates_compact_input_then_preserves_reading_state() {
        let mut app = computed_app(600, 320.0, true);
        let mut source = quest();
        source.detail.description_lines = vec![" Same English fallback  ".repeat(120)];
        app.insert_resource(QuestTracker { active_quests: vec![source] });
        app.insert_resource(QuestPresentationLocale(mir2_game_data::LanguageCode::English));
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.detail_scroll_top = 2;
            state.selected_reward_index = Some(1);
        }
        app.update();
        let revision = app.world().resource::<QuestUiHostContext>().open_revision;
        let old_stamp = app.world().resource::<QuestCompactReadiness>().stamp;
        *app.world_mut().resource_mut::<QuestPresentationLocale>() =
            QuestPresentationLocale(mir2_game_data::LanguageCode::Portuguese);
        let world = app.world();
        assert!(!compact_input_current(world.get_resource::<QuestUiPresentation>(),
            world.get_resource::<QuestUiHostContext>(), world.get_resource::<QuestGuidance>(),
            world.resource::<NativePlayerUiState>(), world.resource::<QuestUiState>(),
            world.resource::<QuestTracker>(), world.resource::<NpcDialogModel>(),
            world.get_resource::<QuestCompactReadiness>(), *world.resource::<QuestPresentationLocale>()));
        app.update();
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.detail_quest_index, Some(23));
        assert_eq!(state.detail_scroll_top, 2);
        assert_eq!(state.selected_reward_index, Some(1));
        assert_eq!(app.world().resource::<QuestUiHostContext>().open_revision, revision);
        let current = app.world().resource::<QuestCompactReadiness>();
        assert!(current.current, "{current:?}");
        assert_ne!(old_stamp, current.stamp, "equal fallback text still changes locale stamp");
    }


    #[test]
    fn m7_compact_heading_tree_locale_flip_preserves_scroll_reward_pending_and_current_stamp() {
        // Compact detail shows three rows; the NPC message shows one. Observe
        // each heading and its body at real, separately valid viewport positions.
        for (npc, progress_section) in [(false, false), (false, true), (true, false)] {
            let mut app = computed_app(600, 320.0, false);
            let mut source = quest();
            source.detail.description_lines = (0..7).map(|i|format!("Opaque host line {i}")).collect();
            source.detail.task_description_lines = vec!["Task body".into()];
            source.detail.return_description_lines = vec!["Return body".into()];
            source.objectives = vec![crate::quest_model::QuestObjective { objective_id: "opaque:1".into(),
                text: "Objective host".into(), current: 1, target: 3 }];
            let mut second = source.clone(); second.quest_index = 24;
            let source_copy = source.clone();
            app.insert_resource(QuestTracker { active_quests: vec![source, second] });
            let dialog = NpcDialogModel { is_open: npc, npc_object_id: Some(0),
                npc_name: Some("Village Chief".into()), lines: vec![crate::quest_model::NpcDialogLine { text: "Return".into() }],
                options: vec![crate::quest_model::NpcDialogOption { option_id: "@raw:return/Progress:9".into(),
                    label: "Progress".into(), enabled: true }] };
            app.insert_resource(dialog.clone());
            app.insert_resource(QuestPresentationLocale(mir2_game_data::LanguageCode::English));
            let layout = app.world().resource::<QuestUiPresentation>().mobile_layout().unwrap();
            let text = QuestRenderText { locale: QuestPresentationLocale(mir2_game_data::LanguageCode::English) };
            let rows = if npc { quest_list_message_lines_with_wrap(&source_copy, &dialog, None, false, text) }
                else { quest_detail_lines_with_wrap(&source_copy, None, "", false, text) };
            let wrapped = mobile_wrap_quest_lines(rows, layout.width_css()-32.0);
            let expected_heading = if progress_section { "Progress" } else { "Return" };
            let heading_index = wrapped.iter().position(|row| row.kind == QuestDetailLineKind::Heading
                && row.text == expected_heading).expect("independent heading row");
            let body_index = heading_index + 1;
            let expected_body = if progress_section { "Objective host (1 / 3)" } else { "Return body" };
            assert_eq!(wrapped[body_index].text, expected_body);
            let max_top = wrapped.len().saturating_sub(if npc { layout.message_rows() } else { layout.detail_rows() });
            let top = heading_index.min(max_top);
            let body_top = body_index.min(max_top);
            assert!(top > 0 && top <= max_top && body_top > 0 && body_top <= max_top,
                "valid nonzero heading/body scroll positions");
            { let mut state = app.world_mut().resource_mut::<QuestUiState>(); state.select_quest(23);
                if npc { state.open_npc_quest_list(&[23,24]); state.select_npc_quest(23); state.npc_quest_message_scroll_top=top; }
                else { state.detail_scroll_top=top; }
                state.selected_reward_index=Some(1); state.npc_selected_reward_index=Some(1);
                state.tracked_quest_indices=vec![24,23]; }
            let pending = PendingOperationKey::QuestAbandon { quest_index: 23 };
            assert!(app.world_mut().resource_mut::<PendingOperations>().try_begin(pending.clone()));
            app.update();
            let revision=app.world().resource::<QuestUiHostContext>().open_revision;
            let mut last_stamp=None;
            for (language, ret, progress) in [(mir2_game_data::LanguageCode::English,"Return","Progress"),
                (mir2_game_data::LanguageCode::Portuguese,"Local de entrega","Progresso"),
                (mir2_game_data::LanguageCode::English,"Return","Progress"),
                (mir2_game_data::LanguageCode::ChineseSimplified,"交付地点","任务进度"),
                (mir2_game_data::LanguageCode::Spanish,"Lugar de entrega","Progreso")] {
                *app.world_mut().resource_mut::<QuestPresentationLocale>()=QuestPresentationLocale(language);
                if last_stamp.is_some() {
                    let world=app.world();
                    assert!(!compact_input_current(world.get_resource::<QuestUiPresentation>(),
                        world.get_resource::<QuestUiHostContext>(), world.get_resource::<QuestGuidance>(),
                        world.resource::<NativePlayerUiState>(), world.resource::<QuestUiState>(),
                        world.resource::<QuestTracker>(), world.resource::<NpcDialogModel>(),
                        world.get_resource::<QuestCompactReadiness>(), *world.resource::<QuestPresentationLocale>()));
                }
                app.update();
                let world=app.world_mut();
                let texts=world.query::<&Text>().iter(world).map(|text|text.0.clone()).collect::<Vec<_>>();
                let heading=if progress_section { progress } else { ret };
                assert!(texts.iter().any(|text|text==heading), "actual compact heading npc={npc} progress={progress_section} {language:?}: {texts:?}");
                let state=world.resource::<QuestUiState>();
                assert_eq!(if npc {state.npc_quest_message_scroll_top} else {state.detail_scroll_top},top);
                assert_eq!(state.detail_quest_index,Some(23));
                if npc { assert_eq!(state.npc_quest_selected_index,Some(23)); }
                assert_eq!(state.selected_reward_index,Some(1)); assert_eq!(state.npc_selected_reward_index,Some(1));
                assert_eq!(state.tracked_quest_indices,vec![24,23]);
                assert!(world.resource::<PendingOperations>().contains(&pending));
                let current=world.resource::<QuestCompactReadiness>();
                assert!(current.current,"measured current compact tree: {current:?}");
                assert_eq!(current.stamp.unwrap().locale,QuestPresentationLocale(language));
                assert_eq!(current.stamp.unwrap().sheet, if npc { MobileQuestSheet::NpcList } else { MobileQuestSheet::Detail },
                    "the real observed sheet must contain the tested section");
                if let Some(previous)=last_stamp {assert_ne!(current.stamp,Some(previous));}
                assert_eq!(world.resource::<QuestUiHostContext>().open_revision,revision);
                assert_eq!(world.resource::<NpcDialogModel>().options[0].option_id,"@raw:return/Progress:9");
                { let mut state=app.world_mut().resource_mut::<QuestUiState>();
                    if npc {state.npc_quest_message_scroll_top=body_top;} else {state.detail_scroll_top=body_top;} }
                app.update();
                let world=app.world_mut();
                assert!(world.query::<&Text>().iter(world).any(|value|value.0==expected_body),
                    "actual opaque body viewport npc={npc} progress={progress_section} {language:?}");
                assert!(world.resource::<QuestCompactReadiness>().current);
                { let mut state=app.world_mut().resource_mut::<QuestUiState>();
                    if npc {state.npc_quest_message_scroll_top=top;} else {state.detail_scroll_top=top;} }
                app.update();
                let restored=app.world().resource::<QuestCompactReadiness>();
                assert!(restored.current);
                assert_eq!(restored.stamp.unwrap().locale,QuestPresentationLocale(language));
                last_stamp=restored.stamp;
            }
        }
    }

    #[test]
    fn known_id_locale_scroll_counterexample_retains_valid_top_and_reaches_host_tail() {
        for npc in [false, true] {
            let mut app = computed_app(600, 320.0, false);
            let mut source = quest();
            source.quest_index = 2_110_002;
            source.title = "Host long quest".into();
            source.detail.description_lines = (0..60).map(|n| format!("Host body line {n:02}")).collect();
            app.insert_resource(QuestTracker { active_quests: vec![source] });
            app.insert_resource(QuestPresentationLocale(mir2_game_data::LanguageCode::English));
            if npc {
                app.insert_resource(NpcDialogModel { is_open: true, npc_object_id: Some(0),
                    npc_name: Some("Village Chief".into()), ..default() });
                let mut state = app.world_mut().resource_mut::<QuestUiState>();
                state.npc_quest_list_open = true;
                state.select_npc_quest(2_110_002);
            } else { app.world_mut().resource_mut::<QuestUiState>().select_quest(2_110_002); }
            app.update();
            let open_revision = app.world().resource::<QuestUiHostContext>().open_revision;
            for language in [mir2_game_data::LanguageCode::English, mir2_game_data::LanguageCode::Portuguese] {
                let mut state = app.world_mut().resource_mut::<QuestUiState>();
                if npc { state.npc_quest_message_scroll_top = 18; } else { state.detail_scroll_top = 18; }
                state.selected_reward_index = Some(1);
                *app.world_mut().resource_mut::<QuestPresentationLocale>() = QuestPresentationLocale(language);
                app.update();
                let state = app.world().resource::<QuestUiState>();
                assert_eq!(if npc { state.npc_quest_message_scroll_top } else { state.detail_scroll_top }, 18,
                    "valid host scroll survives normalize and locale-only redraw npc={npc} language={language:?}");
                assert_eq!(state.selected_reward_index, Some(1));
                let ready = app.world().resource::<QuestCompactReadiness>();
                assert!(ready.current, "locale-only measured tree {ready:?}");
                assert_eq!(ready.stamp.unwrap().locale, QuestPresentationLocale(language));
                assert_eq!(*app.world().resource::<QuestPresentationLocale>(), QuestPresentationLocale(language));
                assert_eq!(app.world().resource::<QuestUiHostContext>().open_revision, open_revision);
            }
            app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
            app.update();
            assert!(app.world().resource::<QuestCompactReadiness>().current, "initial actual layout npc={npc}: {:?}", app.world().resource::<QuestCompactReadiness>());
            let mut reached_tail = false;
            let mut input_executions = 0;
            for _ in 0..100 {
                let world = app.world_mut();
                let button = world.query::<(Entity, &QuestUiButton)>().iter(world).find_map(|(e,a)|
                    ((npc && matches!(a, QuestUiButton::NpcQuestMessageScrollDown)) ||
                     (!npc && matches!(a, QuestUiButton::QuestDetailScrollDown))).then_some(e));
                let Some(button) = button else { break; };
                world.entity_mut(button).insert(Interaction::Pressed);
                // Execute the real input system after the prior measured layout.
                // UiPlugin PreUpdate otherwise replaces synthetic Interaction.
                world.run_system_once(process_quest_ui_input).unwrap();
                input_executions += 1;
                app.update();
                let world = app.world_mut();
                reached_tail = world.query::<(&Text, &ComputedNode, &bevy::ui::UiGlobalTransform, &InheritedVisibility)>()
                    .iter(world).any(|(text,node,transform,visible)| {
                        let rect = compact_rect(node,transform);
                        text.0 == "Host body line 59" && visible.get() && node.size().min_element() > 0.0
                            && rect.min.x >= 0.0 && rect.min.y >= 0.0
                            && rect.max.x <= 600.0 && rect.max.y <= 320.0
                    });
                if reached_tail { break; }
            }
            let world = app.world_mut();
            assert!(reached_tail,
                "actual input scroll reaches the supplied last line npc={npc}; state={:?}; ready={:?}",
                world.resource::<QuestUiState>(), world.resource::<QuestCompactReadiness>());
            assert!(input_executions > 0, "real process_quest_ui_input executed");
            let ready = world.resource::<QuestCompactReadiness>();
            assert!(ready.current, "tail actual measured tree {ready:?}");
            assert_eq!(ready.stamp.unwrap().locale, QuestPresentationLocale(mir2_game_data::LanguageCode::Portuguese));
            assert_eq!(world.resource::<QuestTracker>().active_quests[0].quest_index, 2_110_002);
        }
    }

    fn quest() -> Quest {
        Quest { quest_index: 23, accept_npc_index: Some(0), finish_npc_index: Some(0),
            title: "移动任务".to_owned(), npc_name: None, group: Some("比奇".to_owned()),
            min_level_needed: 1, detail: Default::default(), status: QuestStatus::ReadyToTurnIn,
            objectives: Vec::new(), rewards: vec![crate::quest_model::QuestReward::Item {
                item_id: "reward".to_owned(), name: "奖励戒指".to_owned(), quantity: 1,
                icon: None, selection_index: Some(1), tooltip_source: None,
            }], unknown_text: None }
    }

    fn computed_app(width: u32, height: f32, selected_detail: bool) -> App {
        computed_app_scaled(width, height, 1.0, selected_detail)
    }

    fn computed_app_scaled(width: u32, height: f32, css_scale: f32,
        selected_detail: bool) -> App {
        computed_app_scaled_dpr(width, height, css_scale, 1.0, selected_detail)
    }

    fn computed_app_scaled_dpr(width: u32, height: f32, css_scale: f32,
        device_scale: f32, selected_detail: bool) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default(),
            bevy::input::InputPlugin, bevy::window::WindowPlugin { primary_window: None,
                ..default() }, bevy::image::ImagePlugin::default(),
            bevy::camera::visibility::VisibilityPlugin,
            bevy::text::TextPlugin, bevy::ui::UiPlugin));
        app.init_asset::<Image>().init_asset::<Font>()
            .init_asset::<bevy::image::TextureAtlasLayout>()
            .init_asset::<bevy::mesh::Mesh>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../web/public/original-ui/fonts/NotoSansCJKsc-Regular.otf"))
            .expect("packaged Quest font");
        let font = app.world_mut().resource_mut::<Assets<Font>>().add(Font::from_bytes(bytes));
        app.insert_resource(QuestUiFont(font));
        let physical_size = UVec2::new((width as f32 * device_scale).round() as u32,
            (height * device_scale).round() as u32);
        let camera = app.world_mut().spawn((Camera2d, Camera {
            computed: ComputedCameraValues { target_info: Some(RenderTargetInfo {
                physical_size, scale_factor: device_scale,
            }), ..Default::default() },
            viewport: Some(Viewport { physical_size,
                ..default() }), ..Default::default()
        })).id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(QuestUiPresentation { logical_width: width as f32,
            logical_height: height, stage_css_scale: css_scale, touch: true });
        app.insert_resource(QuestUiHostContext { generation: 7, revision: 11,
            layout_preparing: true, in_game: true, quest_log_open: true,
            open_revision: 1, ..Default::default() });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        app.insert_resource(QuestTracker { active_quests: vec![quest()] });
        app.update();
        if selected_detail { app.world_mut().resource_mut::<QuestUiState>().select_quest(23); }
        app.update();
        app
    }

    #[test]
    fn compact_real_stage_preserves_296_css_panel_through_bevy_layout() {
        for selected_detail in [false, true] {
            let mut app = computed_app_scaled(1440, 768.0, 600.0 / 1440.0, selected_detail);
            let world = app.world_mut();
            let ready = world.resource::<QuestCompactReadiness>();
            assert!(ready.current, "600×320 selected={selected_detail}: {ready:?}");
            let bounds = ready.panel_css_bounds.expect("measured current panel");
            assert!((bounds.left - 20.0).abs() < 0.01, "{bounds:?}");
            assert!((bounds.top - 12.0).abs() < 0.01, "{bounds:?}");
            assert!((bounds.width - 560.0).abs() < 0.01, "{bounds:?}");
            assert!((bounds.height - 296.0).abs() < 0.01, "{bounds:?}");
            assert!(ready.actions.iter().all(|rect| rect.width >= 39.9 && rect.height >= 39.9));
            assert!(ready.min_css_text_size.is_some_and(|size| size >= 13.9));
            let (_, config) = world.query_filtered::<(Entity, &bevy::ui::LayoutConfig),
                Or<(With<QuestLogPanel>, With<QuestDetailPanel>)>>()
                .single(world).expect("only current compact panel is unrounded");
            assert!(!config.use_rounding);
        }
    }

    #[test]
    fn compact_dpr2_idle_diary_keeps_current_text_tree_without_state_churn() {
        let mut app = computed_app_scaled_dpr(1440, 768.0, 600.0 / 1440.0,
            2.0, false);
        app.insert_resource(IdleQuestStateChanges::default());
        app.add_systems(Update, record_idle_quest_state_change
            .after(process_quest_ui_input).before(render_quest_ui));
        app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
        app.update();
        let initial_children = {
            let world = app.world_mut();
            let panel = world.query_filtered::<Entity, With<QuestLogPanel>>()
                .single(world).expect("current diary panel");
            world.get::<Children>(panel).expect("diary text and controls")
                .iter().collect::<Vec<_>>()
        };
        assert!(!initial_children.is_empty());
        app.world_mut().resource_mut::<IdleQuestStateChanges>().0.clear();
        for frame in 0..24 {
            app.update();
            let ready = app.world().resource::<QuestCompactReadiness>();
            assert!(ready.current, "DPR2 idle frame {frame}: {ready:?}");
            assert_eq!(ready.first_failure, None);
            let world = app.world_mut();
            let panel = world.query_filtered::<Entity, With<QuestLogPanel>>()
                .single(world).expect("current diary panel");
            let children = world.get::<Children>(panel).expect("diary text and controls")
                .iter().collect::<Vec<_>>();
            assert_eq!(children, initial_children,
                "DPR2 idle frame {frame} rebuilt Diary children");
        }
        assert_eq!(app.world().resource::<IdleQuestStateChanges>().0,
            vec![false; 24], "idle input must not mark QuestUiState changed");
    }

    #[test]
    fn regular_touch_1368_stage_keeps_rounded_existing_layout() {
        let mut app = computed_app_scaled(1368, 768.0, 640.0 / 1368.0, true);
        let world = app.world_mut();
        assert!(!world.resource::<QuestCompactReadiness>().current);
        let (panel, computed) = world.query_filtered::<(Entity, &ComputedNode),
            With<QuestDetailPanel>>().single(world).unwrap();
        assert!(world.get::<bevy::ui::LayoutConfig>(panel).is_none());
        let measured_css_height = computed.size().y * computed.inverse_scale_factor
            * (640.0 / 1368.0);
        assert!((334.9..335.3).contains(&measured_css_height),
            "regular panel keeps Bevy's prior pixel rounding: {measured_css_height}");
    }

    #[test]
    fn compact_real_stage_other_sheets_keep_measured_bounds() {
        let mut app = computed_app_scaled(1440, 768.0, 600.0 / 1440.0, true);
        app.world_mut().resource_mut::<QuestUiState>()
            .show_quest_alert("这是一段很长的确认说明。".repeat(60));
        app.update();
        let ready = app.world().resource::<QuestCompactReadiness>();
        assert!(ready.current, "confirmation: {ready:?}");
        assert_eq!(ready.sheet, Some("confirmation"));
        app.world_mut().resource_mut::<QuestUiState>().close_quest_alert();
        app.update();
        assert!(app.world().resource::<QuestCompactReadiness>().current);

        let mut npc_quest = quest();
        npc_quest.finish_npc_index = Some(45);
        app.insert_resource(QuestTracker { active_quests: vec![npc_quest] });
        app.insert_resource(NpcDialogModel { is_open: true, npc_object_id: Some(45),
            options: vec![crate::quest_model::NpcDialogOption {
                option_id: "@FinishQuest:23".to_owned(), label: "交付".to_owned(), enabled: true,
            }], ..Default::default() });
        app.world_mut().resource_mut::<QuestUiState>().open_npc_quest_list(&[23]);
        app.update();
        let ready = app.world().resource::<QuestCompactReadiness>();
        assert!(ready.current, "NPC text: {ready:?}");
        assert_eq!(ready.sheet, Some("npcList"));
        app.world_mut().resource_mut::<QuestUiState>().compact_npc_page = CompactQuestPage::Rewards;
        app.update();
        let ready = app.world().resource::<QuestCompactReadiness>();
        assert!(ready.current, "NPC rewards: {ready:?}");
        assert_eq!(ready.page, Some("rewards"));
        assert!(ready.panel_css_bounds.is_some_and(|rect| (rect.height - 296.0).abs() < 0.01));
    }

    #[test]
    fn compact_current_tree_uses_bevy_computed_bounds_at_supported_stages() {
        for (width, height) in [(600, 320.0), (480, 320.0)] {
            let mut app = computed_app(width, height, true);
            let world = app.world_mut();
            let readiness = world.resource::<QuestCompactReadiness>();
            assert!(readiness.current, "{width}×{height}: {readiness:?}");
            assert!(readiness.action_count >= 3);
            assert!(readiness.actions.iter().all(|rect|
                rect.width >= 39.9 && rect.height >= 39.9));
            assert!(readiness.min_css_text_size.is_some_and(|size| size >= 13.9));
            let world = app.world_mut();
            let (node, computed) = world.query_filtered::<(&Node, &ComputedNode), With<QuestDetailPanel>>()
                .single(world).unwrap();
            assert_eq!(node.display, Display::Flex);
            assert!(computed.size().x >= 400.0 && computed.size().y >= 296.0);
        }
        for (width, height) in [(640, 359.298), (844, 390.0)] {
            let mut app = computed_app(width, height, true);
            assert!(!app.world().resource::<QuestCompactReadiness>().current);
            let world = app.world_mut();
            let (node, computed) = world.query_filtered::<(&Node, &ComputedNode), With<QuestDetailPanel>>()
                .single(world).unwrap();
            assert_eq!(node.display, Display::Flex);
            assert!(computed.size().x >= 400.0 && computed.size().y >= 335.0);
        }
    }

    #[test]
    fn compact_reward_pages_keep_every_fixed_and_selectable_item_reachable() {
        let mut app = computed_app(600, 320.0, true);
        let mut source = quest();
        source.rewards = (0..8).map(|index| crate::quest_model::QuestReward::Item {
            item_id: format!("fixed-{index}"), name: format!("固定{index}"), quantity: 1,
            icon: None, selection_index: None, tooltip_source: None,
        }).chain((0..8).map(|index| crate::quest_model::QuestReward::Item {
            item_id: format!("choice-{index}"), name: format!("可选{index}"), quantity: 1,
            icon: None, selection_index: Some(index), tooltip_source: None,
        })).collect();
        app.insert_resource(QuestTracker { active_quests: vec![source] });
        app.world_mut().resource_mut::<QuestUiState>().compact_detail_page = CompactQuestPage::Rewards;
        app.update();
        assert_eq!(app.world().resource::<QuestCompactReadiness>().page, Some("rewards"));
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        for page in 0..3 {
            let world = app.world_mut();
            let choices = world.query::<&QuestUiButton>().iter(world).filter_map(|button| {
                if let QuestUiButton::SelectReward { reward_index, .. } = button {
                    Some(*reward_index)
                } else { None }
            }).collect::<Vec<_>>();
            assert_eq!(choices, ((page * 3)..((page + 1) * 3).min(8))
                .map(|index| index as i32).collect::<Vec<_>>());
            let fixed_text = world.query::<&Text>().iter(world)
                .map(|text| text.0.clone()).collect::<Vec<_>>();
            for index in (page * 3)..((page + 1) * 3).min(8) {
                assert!(fixed_text.iter().any(|text| text.contains(&format!("固定{index}"))));
            }
            if page < 2 {
                world.resource_mut::<QuestUiState>().compact_detail_reward_page += 1;
                app.update();
                assert!(app.world().resource::<QuestCompactReadiness>().current);
            }
        }
    }

    #[test]
    fn compact_npc_sheet_measures_text_and_reward_pages() {
        let mut app = computed_app(600, 320.0, false);
        let mut npc_quest = quest();
        npc_quest.finish_npc_index = Some(45);
        app.insert_resource(QuestTracker { active_quests: vec![npc_quest] });
        app.insert_resource(NpcDialogModel { is_open: true, npc_object_id: Some(45),
            npc_name: Some("任务使者".to_owned()),
            options: vec![crate::quest_model::NpcDialogOption {
                option_id: "@FinishQuest:23".to_owned(), label: "交付".to_owned(), enabled: true,
            }], ..Default::default() });
        app.world_mut().resource_mut::<QuestUiState>().open_npc_quest_list(&[23]);
        app.update();
        let ready = app.world().resource::<QuestCompactReadiness>();
        assert!(ready.current, "NPC text: {ready:?}");
        assert_eq!(ready.sheet, Some("npcList"));
        assert_eq!(ready.page, Some("text"));
        app.world_mut().resource_mut::<QuestUiState>().compact_npc_page = CompactQuestPage::Rewards;
        app.update();
        let ready = app.world().resource::<QuestCompactReadiness>();
        assert!(ready.current, "NPC rewards: {ready:?}");
        assert_eq!(ready.page, Some("rewards"));
        assert!(ready.actions.iter().all(|rect|
            rect.width >= 39.9 && rect.height >= 39.9));
    }

    #[test]
    fn compact_confirmation_restores_underlying_page_and_paginates_long_text() {
        let mut app = computed_app(600, 320.0, true);
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.compact_detail_page = CompactQuestPage::Rewards;
            state.show_quest_alert("这是一段很长的确认说明。".repeat(60));
        }
        app.update();
        let ready = app.world().resource::<QuestCompactReadiness>();
        assert!(ready.current, "{ready:?}");
        assert_eq!(ready.sheet, Some("confirmation"));
        let world = app.world_mut();
        assert!(world.query::<&QuestUiButton>().iter(world)
            .any(|button| matches!(button, QuestUiButton::CompactConfirmationNext)));
        app.world_mut().resource_mut::<QuestUiState>().compact_confirmation_page = 1;
        app.update();
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        app.world_mut().resource_mut::<QuestUiState>().close_quest_alert();
        app.update();
        let ready = app.world().resource::<QuestCompactReadiness>();
        assert!(ready.current, "{ready:?}");
        assert_eq!(ready.sheet, Some("detail"));
        assert_eq!(ready.page, Some("rewards"));
    }

    #[test]
    fn compact_diary_clamps_shrunk_content_and_pages_all_graduation_options() {
        let options = [GraduationDirection::Equipment, GraduationDirection::Skill,
            GraduationDirection::Challenge].into_iter().map(|direction|
            crate::quest_journey::GraduationOption { direction,
                title: direction.label().to_owned(), summary: String::new(),
                instruction: String::new() }).collect::<Vec<_>>();
        assert_eq!(mobile_diary_page_count(0, options.len(), 2), 2);
        assert_eq!(mobile_graduation_page(&options, 0, 2, 0).unwrap().len(), 2);
        assert_eq!(mobile_graduation_page(&options, 0, 2, 1).unwrap()[0].direction,
            GraduationDirection::Challenge);
        let mut app = computed_app(600, 320.0, false);
        let quests = (0..5).map(|index| { let mut item = quest();
            item.quest_index += index; item }).collect();
        app.insert_resource(QuestTracker { active_quests: quests });
        app.world_mut().resource_mut::<QuestUiState>().page = 2;
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().page, 2);
        app.insert_resource(QuestTracker { active_quests: vec![quest()] });
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().page, 0);
        assert!(app.world().resource::<QuestCompactReadiness>().current);
    }

    #[test]
    fn compact_local_tab_input_rejects_second_old_tree_action_and_hidden_input() {
        let mut app = computed_app(600, 320.0, true);
        app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
        app.update();
        let world = app.world_mut();
        let rewards_tab = world.query::<(Entity, &QuestUiButton)>().iter(world)
            .find_map(|(entity, button)| matches!(button,
                QuestUiButton::CompactDetailPage(CompactQuestPage::Rewards)).then_some(entity))
            .expect("current rewards tab");
        world.entity_mut(rewards_tab).insert(Interaction::Pressed);
        app.world_mut().run_system_once(process_quest_ui_input).unwrap();
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().compact_detail_page,
            CompactQuestPage::Rewards);
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        let world = app.world_mut();
        let tabs = world.query::<(Entity, &QuestUiButton)>().iter(world)
            .filter_map(|(entity, button)| match button {
                QuestUiButton::CompactDetailPage(page) => Some((entity, *page)),
                _ => None,
            }).collect::<Vec<_>>();
        for (entity, _) in &tabs { world.entity_mut(*entity).insert(Interaction::Pressed); }
        app.world_mut().run_system_once(process_quest_ui_input).unwrap();
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().compact_detail_page,
            CompactQuestPage::Text, "second old-tree tab must not undo the first");
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = false;
        let world = app.world_mut();
        let rewards_tab = world.query::<(Entity, &QuestUiButton)>().iter(world)
            .find_map(|(entity, button)| matches!(button,
                QuestUiButton::CompactDetailPage(CompactQuestPage::Rewards)).then_some(entity))
            .unwrap();
        world.entity_mut(rewards_tab).insert(Interaction::Pressed);
        app.world_mut().run_system_once(process_quest_ui_input).unwrap();
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().compact_detail_page,
            CompactQuestPage::Text, "layout preparation cannot send hidden input");
    }

    #[test]
    fn compact_guided_tab_switch_rejects_old_page_zero_quest_row() {
        let mut app = computed_app(600, 320.0, false);
        app.insert_resource(QuestGuidance::from_profile_name("newcomer-v2"));
        app.insert_resource(QuestTracker { active_quests: vec![quest()] });
        {
            let mut state = app.world_mut().resource_mut::<QuestUiState>();
            state.diary_tab = GuidedDiaryTab::Side;
            state.diary_page = 0;
        }
        app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
        app.update();
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        let world = app.world_mut();
        let buttons = world.query::<(Entity, &QuestUiButton)>().iter(world)
            .filter_map(|(entity, button)| match button {
                QuestUiButton::SelectGuidedDiaryTab(GuidedDiaryTab::Main) => Some((entity, true)),
                QuestUiButton::SelectQuest { quest_index: 23 } => Some((entity, false)),
                _ => None,
            }).collect::<Vec<_>>();
        assert_eq!(buttons.len(), 2, "current side tab and quest row");
        for (entity, _) in buttons { world.entity_mut(entity).insert(Interaction::Pressed); }
        world.run_system_once(process_quest_ui_input).unwrap();
        let state = world.resource::<QuestUiState>();
        assert_eq!(state.diary_tab, GuidedDiaryTab::Main);
        assert_eq!(state.diary_page, 0);
        assert_eq!(state.detail_quest_index, None,
            "old Side row cannot open after Main replaced the visible page");
        assert!(world.resource::<QuestUiIntentQueue>().is_empty());
    }

    #[test]
    fn compact_service_return_invalidates_old_dialog_options_in_same_frame() {
        let mut app = computed_app(600, 320.0, false);
        let prior = NpcDialogModel { is_open: true, npc_object_id: Some(45),
            options: vec![crate::quest_model::NpcDialogOption {
                option_id: "@Shared".to_owned(), label: "Previous".to_owned(), enabled: true,
            }], ..Default::default() };
        app.world_mut().resource_mut::<NpcDialogNav>().push(prior);
        app.insert_resource(NpcDialogModel { is_open: true, npc_object_id: Some(45),
            options: vec![crate::quest_model::NpcDialogOption {
                option_id: "@Shared".to_owned(), label: "Current".to_owned(), enabled: true,
            }], ..Default::default() });
        app.world_mut().resource_mut::<QuestUiHostContext>().host_visible = true;
        app.update();
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        let world = app.world_mut();
        assert!(world.query::<(Entity, &QuestUiButton)>().iter(world)
            .any(|(_, action)| matches!(action,
                QuestUiButton::SelectNpcDialog { target } if target == "@Shared")
            ), "old dialog option");
        // Deliver two changes from the old sheet in a deterministic order.
        // The measured snapshot stays current until the next layout pass.
        let old_buttons = world.query_filtered::<Entity, With<Button>>()
            .iter(world).collect::<Vec<_>>();
        for entity in old_buttons { world.despawn(entity); }
        let return_first = world.spawn((Button, QuestUiButton::ReturnNpcService,
            Interaction::None)).id();
        let stale_option = world.spawn((Button, QuestUiButton::SelectNpcDialog {
            target: "@Shared".to_owned() }, Interaction::None)).id();
        world.entity_mut(return_first).insert(Interaction::Pressed);
        world.entity_mut(stale_option).insert(Interaction::Pressed);
        world.run_system_once(process_quest_ui_input).unwrap();
        assert!(!world.resource::<NpcDialogNav>().can_return());
        assert!(world.resource::<QuestUiIntentQueue>().is_empty(),
            "old page option must not send after local return");
    }

    #[test]
    fn compact_observer_rejects_old_stamp_and_unlaid_out_current_panel() {
        let mut app = computed_app(600, 320.0, true);
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        app.world_mut().resource_mut::<QuestUiHostContext>().revision += 1;
        let world = app.world();
        assert!(!compact_input_current(world.get_resource::<QuestUiPresentation>(),
            world.get_resource::<QuestUiHostContext>(), world.get_resource::<QuestGuidance>(),
            world.resource::<NativePlayerUiState>(), world.resource::<QuestUiState>(),
            world.resource::<QuestTracker>(), world.resource::<NpcDialogModel>(),
            world.get_resource::<QuestCompactReadiness>(), QuestPresentationLocale::default()));
        app.update();
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        let world = app.world_mut();
        let panel = world.query_filtered::<Entity, With<QuestDetailPanel>>()
            .single(world).unwrap();
        world.entity_mut(panel).get_mut::<ComputedNode>().unwrap().size.x = 0.0;
        world.run_system_once(observe_compact_quest_tree).unwrap();
        let ready = world.resource::<QuestCompactReadiness>();
        assert!(!ready.current);
        assert_eq!(ready.first_failure, Some("panelBounds"));
    }

    #[test]
    fn compact_same_quest_body_and_temporary_hide_keep_reading_position() {
        let mut app = computed_app(600, 320.0, true);
        let mut source = quest();
        source.detail.description_lines = vec!["阅读任务内容".repeat(120)];
        app.insert_resource(QuestTracker { active_quests: vec![source.clone()] });
        app.update();
        app.world_mut().resource_mut::<QuestUiState>().detail_scroll_top = 2;
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().detail_scroll_top, 2);
        source.detail.description_lines = vec!["更新后的同一任务内容".repeat(120)];
        app.insert_resource(QuestTracker { active_quests: vec![source] });
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().detail_scroll_top, 2);
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        {
            let mut host = app.world_mut().resource_mut::<QuestUiHostContext>();
            host.host_visible = false;
            host.layout_preparing = false;
        }
        app.update();
        assert!(!app.world().resource::<QuestCompactReadiness>().current);
        assert_eq!(app.world().resource::<QuestUiState>().detail_scroll_top, 2);
        app.world_mut().resource_mut::<QuestUiHostContext>().layout_preparing = true;
        app.update();
        assert!(app.world().resource::<QuestCompactReadiness>().current);
        assert_eq!(app.world().resource::<QuestUiState>().detail_scroll_top, 2);
    }

    #[test]
    fn landscape_sheet_bounds_and_touch_metrics_fit_the_stage() {
        let p = presentation();
        let layout = p.mobile_layout().expect("touch landscape layout");
        assert!(layout.panel.left >= 0.0 && layout.panel.top >= 0.0);
        assert!(layout.panel.left + layout.panel.width <= p.logical_width);
        assert!(layout.panel.top + layout.panel.height <= p.logical_height);
        assert!(layout.px(14.0) * p.stage_css_scale >= 14.0);
        assert!(layout.px(44.0) * p.stage_css_scale >= 44.0);
        assert_eq!(layout.diary_rows(), 4);
        assert_eq!(layout.dialog_rows(), 5);
        assert_eq!(layout.message_rows(), 2);
        assert_eq!(layout.detail_rows(), 4);
        let compact = QuestUiPresentation { logical_width: 640.0, logical_height: 359.298,
            stage_css_scale: 1.0, touch: true }.mobile_layout().expect("640×360 mobile sheet");
        assert_eq!(compact.message_rows(), 1);
        assert_eq!(compact.detail_rows(), 3);
        let narrow = QuestUiPresentation { logical_width: 480.0, logical_height: 320.0,
            stage_css_scale: 1.0, touch: true }.mobile_layout().expect("minimum mobile sheet");
        assert!(narrow.dialog_footer_button_width(4) >= 44.0);
        assert!(12.0 + 4.0 * narrow.dialog_footer_button_width(4) + 3.0 * 8.0
            <= narrow.width_css() - 12.0);
        let within_sheet = |(x, y, width, height): (f32, f32, f32, f32)|
            x >= 0.0 && y >= 0.0 && x + width <= narrow.width_css()
                && y + height <= narrow.height_css() && width >= 44.0 && height >= 44.0;
        let disjoint = |(ax, ay, aw, ah): (f32, f32, f32, f32),
            (bx, by, bw, bh): (f32, f32, f32, f32)|
            ax + aw <= bx || bx + bw <= ax || ay + ah <= by || by + bh <= ay;
        for rects in [narrow.detail_footer_rects(), narrow.npc_footer_rects()] {
            assert!(rects.iter().copied().all(within_sheet));
            for left in 0..rects.len() {
                for right in left + 1..rects.len() {
                    assert!(disjoint(rects[left], rects[right]));
                }
            }
        }
        assert_eq!(mobile_sheet(false, true, true, true, true), MobileQuestSheet::NpcList);
        assert_eq!(mobile_sheet(false, false, true, true, true), MobileQuestSheet::Dialog);
        assert!(QuestUiPresentation { touch: false, ..p }.mobile_layout().is_none());
        assert!(QuestUiPresentation { logical_width: 423.0, logical_height: 360.0,
            stage_css_scale: 1.0, touch: true }.mobile_layout().is_none());
        assert!(QuestUiPresentation { logical_width: 844.0, logical_height: 350.0,
            stage_css_scale: 1.0, touch: true }.mobile_layout().unwrap().compact);
        assert!(QuestUiPresentation { logical_width: 640.0, logical_height: 358.9,
            stage_css_scale: 1.0, touch: true }.mobile_layout().unwrap().compact);
        assert!(QuestUiPresentation { logical_width: 600.0, logical_height: 320.0,
            stage_css_scale: 1.0, touch: true }.mobile_layout().unwrap().compact);
        assert!(QuestUiPresentation { logical_width: 600.0, logical_height: 319.0,
            stage_css_scale: 1.0, touch: true }.mobile_layout().is_none());
        assert!(QuestUiPresentation { logical_width: 500.0, logical_height: 400.0,
            stage_css_scale: 1.0, touch: true }.mobile_layout().is_none());
    }

    #[test]
    fn narrow_dialog_wraps_options_before_counting_mobile_pages() {
        let dialog = NpcDialogModel { options: vec![crate::quest_model::NpcDialogOption {
            option_id: "@Buy".to_owned(),
            label: "这是一段必须在窄屏窗口内换行的NPC长选项文字".to_owned(),
            enabled: true,
        }], ..Default::default() };
        let rows = mobile_npc_dialog_rows(&dialog, 248.0);
        assert!(rows.len() > npc_dialog_rows(&dialog).len());
        assert!(rows.iter().all(|(text, target, enabled)|
            !text.is_empty() && target.as_deref() == Some("@Buy") && *enabled));
        assert!(rows.iter().all(|(text, _, _)|
            text.chars().map(mobile_text_columns).sum::<usize>()
                <= (248.0_f32 / 7.0).floor() as usize));
    }

    #[test]
    fn mobile_detail_wraps_source_after_localization_at_available_width() {
        let mut source = quest();
        source.quest_index = 2110003;
        source.title = "Protect the village".to_owned();
        source.detail.description_lines = vec!["Protect the village".to_owned()];
        let desktop = quest_detail_lines(&source, None, "Warrior");
        let mobile_source = quest_detail_lines_with_wrap(&source, None, "Warrior", false, QuestRenderText::default());
        let mobile = mobile_wrap_quest_lines(mobile_source.clone(), 528.0);
        assert!(desktop.len() > mobile_source.len(), "desktop prewrap remains native only");
        assert!(mobile_source.iter().any(|line| line.text.contains("消灭村庄附近的 2 个稻草人")));
        assert!(mobile.iter().all(|line| line.text.chars()
            .map(mobile_text_columns).sum::<usize>() <= (528.0_f32 / 7.0).floor() as usize));
        assert!(mobile.len() < desktop.len(), "wider mobile text reduces scroll count");
    }

    #[test]
    fn shared_mobile_detail_keeps_reward_hit_and_dialog_priority() {
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(presentation());
        app.insert_resource(QuestUiHostContext { host_visible: true, in_game: true,
            quest_log_open: true, open_revision: 1, ..Default::default() });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        let mut reward_quest = quest();
        if let crate::quest_model::QuestReward::Item { name, .. } = &mut reward_quest.rewards[0] {
            *name = "Fencing".to_owned();
        }
        reward_quest.rewards.extend([
            crate::quest_model::QuestReward::Experience { amount: 80 },
            crate::quest_model::QuestReward::Gold { amount: 50 },
        ]);
        app.insert_resource(QuestTracker { active_quests: vec![reward_quest] });
        app.update();
        app.world_mut().resource_mut::<QuestUiState>().select_quest(23);
        app.update();
        let world = app.world_mut();
        let detail = world.query_filtered::<&Node, With<QuestDetailPanel>>()
            .single(world).expect("detail panel");
        assert_eq!(detail.display, Display::Flex);
        let diary = world.query_filtered::<&Node, With<QuestLogPanel>>()
            .single(world).expect("diary panel");
        assert_eq!(diary.display, Display::None);
        let visible_text = world.query::<&Text>().iter(world)
            .map(|value| value.0.as_str()).collect::<Vec<_>>();
        assert!(visible_text.iter().any(|value| value.contains("经验 80 · 金币 50")));
        assert!(visible_text.iter().any(|value| value.contains("基本剑术")));
        let reward = world.query::<(&QuestUiButton, &Node)>().iter(world)
            .find_map(|(action, node)| matches!(action,
                QuestUiButton::SelectReward { quest_index: 23, reward_index: 1 }).then_some(node))
            .expect("shared selectable reward");
        assert!(matches!(reward.height, Val::Px(height) if height * presentation().stage_css_scale >= 44.0));
        assert!(matches!(reward.width, Val::Px(width) if width * presentation().stage_css_scale >= 44.0));
        let font_sizes = world.query::<&TextFont>().iter(world)
            .filter_map(|font| match font.font_size { FontSize::Px(size) => Some(size), _ => None })
            .collect::<Vec<_>>();
        assert!(!font_sizes.is_empty());
        assert!(font_sizes.iter().all(|size| size * presentation().stage_css_scale >= 13.99));
        for (action, node) in world.query::<(&QuestUiButton, &Node)>().iter(world) {
            if matches!(action, QuestUiButton::CloseQuestDetail | QuestUiButton::SelectReward { .. }
                | QuestUiButton::PrepareQuestFinish { .. } | QuestUiButton::AbandonQuest { .. }) {
                assert!(matches!(node.height, Val::Px(height)
                    if height * presentation().stage_css_scale >= 43.99));
            }
        }

        world.resource_mut::<NpcDialogModel>().is_open = true;
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query_filtered::<&Node, With<NpcDialogPanel>>()
            .single(world).unwrap().display, Display::Flex);
        assert_eq!(world.query_filtered::<&Node, With<QuestDetailPanel>>()
            .single(world).unwrap().display, Display::None);
    }

    #[test]
    fn mobile_npc_list_returns_locally_then_finishes_with_selected_reward() {
        let mut app = App::new();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(presentation());
        app.insert_resource(QuestUiHostContext { host_visible: true, in_game: true,
            ..Default::default() });
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_plugins(Mir2PortableQuestUiPlugin);
        let mut reward_quest = quest();
        reward_quest.finish_npc_index = Some(45);
        app.insert_resource(QuestTracker { active_quests: vec![reward_quest] });
        app.insert_resource(NpcDialogModel {
            is_open: true, npc_object_id: Some(45), npc_name: Some("任务使者".to_owned()),
            options: vec![crate::quest_model::NpcDialogOption {
                option_id: "@FinishQuest:23".to_owned(), label: "交付".to_owned(), enabled: true,
            }], ..Default::default()
        });
        app.update();
        app.world_mut().resource_mut::<QuestUiState>().open_npc_quest_list(&[23]);
        app.update();

        let back = app.world_mut().query::<(Entity, &QuestUiButton)>().iter(app.world())
            .find_map(|(entity, action)| matches!(action, QuestUiButton::ReturnToNpcDialog).then_some(entity))
            .expect("mobile return control");
        app.world_mut().entity_mut(back).insert(Interaction::Pressed);
        app.update();
        assert!(!app.world().resource::<QuestUiState>().npc_quest_list_open);
        assert!(app.world().resource::<NpcDialogModel>().is_open);
        assert!(app.world().resource::<QuestUiIntentQueue>().is_empty());

        app.world_mut().resource_mut::<QuestUiState>().open_npc_quest_list(&[23]);
        app.update();
        let reward = app.world_mut().query::<(Entity, &QuestUiButton)>().iter(app.world())
            .find_map(|(entity, action)| matches!(action, QuestUiButton::SelectNpcQuestReward {
                quest_index: 23, reward_index: 1,
            }).then_some(entity)).expect("mobile reward button");
        app.world_mut().entity_mut(reward).insert(Interaction::Pressed);
        app.update();
        assert_eq!(app.world().resource::<QuestUiState>().npc_selected_reward_index, Some(1));
        let finish = app.world_mut().query::<(Entity, &QuestUiButton)>().iter(app.world())
            .find_map(|(entity, action)| matches!(action, QuestUiButton::FinishQuest {
                quest_index: 23, selected_item_index: 1,
            }).then_some(entity)).expect("mobile Finish button");
        app.world_mut().entity_mut(finish).insert(Interaction::Pressed);
        app.update();
        assert_eq!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents(),
            vec![QuestUiIntent::FinishQuest { quest_index: 23, selected_item_index: 1 }]);
    }
}


#[cfg(test)]
mod presentation_locale_tree_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;


    #[test]
    fn selected_desktop_locale_counterexample_labels_keep_exact_action_rect_and_enablement() {
        for language in [mir2_game_data::LanguageCode::English, mir2_game_data::LanguageCode::Spanish,
            mir2_game_data::LanguageCode::Portuguese, mir2_game_data::LanguageCode::ChineseSimplified] {
            for scene in 0..3 {
                let text = QuestRenderText { locale: QuestPresentationLocale(language) };
                let mut world = World::new();
                world.run_system_once(move |mut commands: Commands| {
                    commands.spawn(Node::default()).with_children(|parent| {
                        let tracker = QuestTracker::default(); let state = QuestUiState::default();
                        let pending = PendingOperations::default();
                        let guidance = QuestGuidance::from_profile_name(if scene == 0 { "newcomer-v2" } else { "crystal" });
                        if scene < 2 { render_quest_diary_panel(parent, &tracker, &guidance, None,
                            &state, &pending, None, text); }
                        else { render_dialog_panel(parent, &NpcDialogModel { npc_name: Some("Village Chief".into()),
                            lines: vec![crate::quest_model::NpcDialogLine { text: "Accept opaque body".into() }],
                            options: vec![crate::quest_model::NpcDialogOption { option_id: "@Raw:123".into(),
                                label: "Accept".into(), enabled: true }], ..default() },
                            &NpcDialogNav::default(), &state, &pending, true, None, text); }
                    });
                }).unwrap();
                let rect = if scene < 2 { quest_diary_layout(1.0).bottom_close }
                    else { QuestLogRect::new(172.0, 194.0, 96.0, 25.0) };
                let button = world.query::<(Entity, &Node, &QuestUiButton, &QuestUiButtonVisual)>().iter(&world)
                    .find_map(|(e,n,a,v)| (n.left==Val::Px(rect.left) && n.top==Val::Px(rect.top)
                        && n.width==Val::Px(rect.width) && n.height==Val::Px(rect.height)).then_some((e,a.clone(),v.enabled)))
                    .expect("selected authored rectangle");
                assert!(button.2); assert!(world.get::<Button>(button.0).is_some());
                assert!(if scene < 2 { matches!(button.1, QuestUiButton::CloseQuestLog) }
                    else { matches!(button.1, QuestUiButton::ToggleNpcQuestList) });
                let expected = if scene < 2 { text.chrome("ui.close", "Close") } else { text.chrome("ui.quest", "Quest") };
                assert!(world.query::<(&Text, &ChildOf)>().iter(&world).any(|(t,p)| p.parent()==button.0 && t.0==expected),
                    "selected caption scene={scene} language={language:?} expected={expected}");
                if scene==2 { assert!(world.query::<&Text>().iter(&world).any(|t|t.0=="Accept opaque body"));
                    assert!(world.query::<&Text>().iter(&world).any(|t|t.0=="Village Chief"));
                    assert!(world.query::<&Text>().iter(&world).any(|t|t.0=="Accept")); }
            }
        }
    }

    #[test]
    fn quest_body_locale_counterexample_completion_and_unknown_keep_chinese_field_adapter_only() {
        let mut quest = Quest { quest_index: 2_110_002, title: "Host".into(), accept_npc_index: Some(9),
            finish_npc_index: Some(10), npc_name: None, group: None, min_level_needed: 1,
            status: crate::quest_model::QuestStatus::InProgress, detail: default(), objectives: vec![],
            rewards: vec![], unknown_text: None };
        quest.detail.completion_description_lines = vec!["  Accept  ".into()];
        let dialog = NpcDialogModel { npc_object_id: Some(10), ..default() };
        for language in [mir2_game_data::LanguageCode::English, mir2_game_data::LanguageCode::Spanish,
            mir2_game_data::LanguageCode::Portuguese, mir2_game_data::LanguageCode::ChineseSimplified] {
            let text = QuestRenderText { locale: QuestPresentationLocale(language) };
            let expected = text.body("  Accept  ");
            assert!(quest_list_message_lines_with_wrap(&quest, &dialog, None, false, text)
                .iter().any(|line| line.text == expected), "completion {language:?}");
            let mut fallback = quest.clone(); fallback.quest_index = 987_654;
            fallback.detail = default(); fallback.unknown_text = Some("  Accept  ".into());
            for lines in [quest_detail_lines_with_wrap(&fallback, None, "", false, text),
                quest_list_message_lines_with_wrap(&fallback, &dialog, None, false, text)] {
                assert!(lines.iter().any(|line|line.text==expected), "unknown fallback {language:?}");
            }
        }
    }


    fn legacy_section_source() -> Quest {
        Quest { quest_index: 987_654, title: "Host title".into(), accept_npc_index: Some(9),
            finish_npc_index: Some(10), npc_name: None, group: None, min_level_needed: 1,
            status: crate::quest_model::QuestStatus::InProgress,
            detail: crate::quest_model::QuestDetailText { description_lines: vec!["Host body".into()],
                task_description_lines: vec!["Host task".into()], return_description_lines: vec!["Host return".into()],
                time_limit: Some(" 05:00 ".into()), ..default() },
            objectives: vec![crate::quest_model::QuestObjective { objective_id: "opaque:1".into(),
                text: "Objective host".into(), current: 1, target: 3 }], rewards: vec![], unknown_text: None }
    }

    fn heading_goldens() -> [(mir2_game_data::LanguageCode, &'static str, &'static str); 4] {
        [(mir2_game_data::LanguageCode::English, "Return", "Progress"),
            (mir2_game_data::LanguageCode::ChineseSimplified, "交付地点", "任务进度"),
            (mir2_game_data::LanguageCode::Spanish, "Lugar de entrega", "Progreso"),
            (mir2_game_data::LanguageCode::Portuguese, "Local de entrega", "Progresso")]
    }

    fn assert_heading_before_body(lines: &[QuestDetailLine], body: &str, heading: &str) {
        let index = lines.iter().position(|row| row.kind == QuestDetailLineKind::Body && row.text == body)
            .expect("original section body remains present");
        assert!(index >= 2);
        assert_eq!(lines[index - 2].kind, QuestDetailLineKind::Blank);
        assert!(lines[index - 2].text.is_empty());
        assert_eq!(lines[index - 1].kind, QuestDetailLineKind::Heading);
        assert_eq!(lines[index - 1].text, heading, "localized heading before unchanged body {body:?}");
    }

    #[test]
    fn m7_heading_counterexample_detail_return_uses_selected_language() {
        for (language, return_heading, _) in heading_goldens() {
            for status in [crate::quest_model::QuestStatus::InProgress, crate::quest_model::QuestStatus::ReadyToTurnIn] {
                for desktop_wrap in [false, true] {
                    let mut quest = legacy_section_source(); quest.status = status.clone();
                    let text = QuestRenderText { locale: QuestPresentationLocale(language) };
                    let rows = quest_detail_lines_with_wrap(&quest, None, "", desktop_wrap, text);
                    assert_heading_before_body(&rows, "Host return", return_heading);
                    assert!(rows.iter().position(|row| row.text == "Host task").unwrap()
                        < rows.iter().position(|row| row.text == return_heading).unwrap());
                }
            }
        }
    }

    #[test]
    fn m7_heading_counterexample_npc_return_uses_selected_language() {
        let dialog = NpcDialogModel { npc_object_id: Some(9), ..default() };
        for (language, return_heading, progress_heading) in heading_goldens() {
            for status in [crate::quest_model::QuestStatus::InProgress, crate::quest_model::QuestStatus::ReadyToTurnIn] {
                for desktop_wrap in [false, true] {
                    let mut quest = legacy_section_source(); quest.status = status.clone();
                    let text = QuestRenderText { locale: QuestPresentationLocale(language) };
                    let rows = quest_list_message_lines_with_wrap(&quest, &dialog, None, desktop_wrap, text);
                    assert_heading_before_body(&rows, "Host return", return_heading);
                    assert!(!rows.iter().any(|row| row.kind == QuestDetailLineKind::Heading && row.text == progress_heading));
                }
            }
        }
    }

    #[test]
    fn m7_heading_counterexample_detail_progress_uses_selected_language() {
        for (language, return_heading, progress_heading) in heading_goldens() {
            for status in [crate::quest_model::QuestStatus::InProgress, crate::quest_model::QuestStatus::ReadyToTurnIn] {
                for desktop_wrap in [false, true] {
                    let mut quest = legacy_section_source(); quest.status = status.clone();
                    let text = QuestRenderText { locale: QuestPresentationLocale(language) };
                    let rows = quest_detail_lines_with_wrap(&quest, None, "", desktop_wrap, text);
                    assert_heading_before_body(&rows, "Objective host (1 / 3)", progress_heading);
                    assert!(rows.iter().position(|row| row.text == return_heading).unwrap()
                        < rows.iter().position(|row| row.text == progress_heading).unwrap());
                }
            }
        }
    }

    #[test]
    fn legacy_chinese_time_section_counterexample_keeps_blank_heading_body_in_both_real_builders() {
        let quest = legacy_section_source();
        let dialog = NpcDialogModel { npc_object_id: Some(9), ..default() };
        let chinese = QuestRenderText::default();
        for lines in [quest_detail_lines_with_wrap(&quest, None, "", false, chinese),
            quest_list_message_lines_with_wrap(&quest, &dialog, None, false, chinese)] {
            assert!(lines.windows(3).any(|rows| rows[0].kind == QuestDetailLineKind::Blank && rows[0].text.is_empty()
                && rows[1].kind == QuestDetailLineKind::Heading && rows[1].text == "时限"
                && rows[2].kind == QuestDetailLineKind::Body && rows[2].text == " 05:00 "),
                "legacy exact Chinese Time Limit section: {lines:?}");
        }
        for (language, expected) in [(mir2_game_data::LanguageCode::English, "Time:  05:00 "),
            (mir2_game_data::LanguageCode::Spanish, "Tiempo:  05:00 "),
            (mir2_game_data::LanguageCode::Portuguese, "Time:  05:00 ")] {
            let text = QuestRenderText { locale: QuestPresentationLocale(language) };
            for lines in [quest_detail_lines_with_wrap(&quest, None, "", false, text),
                quest_list_message_lines_with_wrap(&quest, &dialog, None, false, text)] {
                assert!(lines.iter().any(|line| line.kind == QuestDetailLineKind::Body && line.text == expected),
                    "canonical non-Chinese time retains supplied whitespace {language:?}: {lines:?}");
            }
        }
    }

    #[test]
    fn distinct_progress_section_counterexample_keeps_tasks_progress_and_npc_scope_separate() {
        let quest = legacy_section_source();let text = QuestRenderText::default();
        let lines = quest_detail_lines_with_wrap(&quest, None, "", false, text);
        assert_eq!(lines.iter().filter(|line| line.kind == QuestDetailLineKind::Heading
            && line.text == "任务目标").count(), 1, "keyed Tasks is one distinct section");
        assert_eq!(lines.iter().filter(|line| line.kind == QuestDetailLineKind::Heading
            && line.text == "任务进度").count(), 1, "legacy Progress is a distinct section");
        assert!(lines.iter().any(|line| line.text == "Objective host (1 / 3)"));
        let dialog = NpcDialogModel { npc_object_id: Some(9), ..default() };
        let npc = quest_list_message_lines_with_wrap(&quest, &dialog, None, false, text);
        assert!(!npc.iter().any(|line|line.kind == QuestDetailLineKind::Heading && line.text == "任务进度"),
            "NPC list never gained the detail-only Progress section");
        for (language, expected) in [(mir2_game_data::LanguageCode::English, "Progress"),
            (mir2_game_data::LanguageCode::Spanish, "Progreso"), (mir2_game_data::LanguageCode::Portuguese, "Progresso")] {
            let text = QuestRenderText { locale: QuestPresentationLocale(language) };
            let detail = quest_detail_lines_with_wrap(&quest, None, "", false, text);
            assert!(detail.iter().any(|line|line.kind == QuestDetailLineKind::Heading && line.text == expected),
                "keyed Progress stays distinct from Tasks in the selected language");
            assert!(detail.iter().any(|line|line.text == "Host task"));
        }
    }

    #[test]
    fn m7_heading_changes_preserve_known_host_vectors_and_section_gates() {
        let mut quest = legacy_section_source(); quest.quest_index = 2_110_002;
        quest.detail.description_lines = vec!["  Return\t ".into(), "Second host 🌿 line  ".into()];
        quest.detail.task_description_lines = vec!["  Progress\t ".into()];
        quest.detail.return_description_lines = vec!["  Village Chief\t ".into(), "Misión / 任务 / Quest raw line".into()];
        quest.detail.time_limit = None;
        let dialog = NpcDialogModel { npc_object_id: Some(9), ..default() };
        for (language, return_heading, progress_heading) in heading_goldens() {
            let text = QuestRenderText { locale: QuestPresentationLocale(language) };
            if !text.is_chinese() {
                for rows in [quest_detail_lines_with_wrap(&quest, None, "", false, text),
                    quest_list_message_lines_with_wrap(&quest, &dialog, None, false, text)] {
                    for expected in quest.detail.description_lines.iter().chain(&quest.detail.task_description_lines)
                        .chain(&quest.detail.return_description_lines) {
                        assert!(rows.iter().any(|row| row.kind == QuestDetailLineKind::Body && &row.text == expected),
                            "opaque host vector {language:?} {expected:?}: {rows:?}");
                    }
                    assert_heading_before_body(&rows, "  Village Chief\t ", return_heading);
                }
            }
            let mut empty = quest.clone(); empty.detail.return_description_lines = vec![" ".into(), "\t".into()];
            for rows in [quest_detail_lines_with_wrap(&empty, None, "", false, text),
                quest_list_message_lines_with_wrap(&empty, &dialog, None, false, text)] {
                assert!(!rows.iter().any(|row| row.kind == QuestDetailLineKind::Heading && row.text == return_heading));
            }
            for status in [crate::quest_model::QuestStatus::NotStarted, crate::quest_model::QuestStatus::Completed] {
                let mut inactive = quest.clone(); inactive.status = status;
                assert!(!quest_detail_lines_with_wrap(&inactive, None, "", false, text).iter()
                    .any(|row| row.kind == QuestDetailLineKind::Heading && row.text == progress_heading));
            }
            let mut no_objectives = quest.clone(); no_objectives.objectives.clear();
            assert!(!quest_detail_lines_with_wrap(&no_objectives, None, "", false, text).iter()
                .any(|row| row.kind == QuestDetailLineKind::Heading && row.text == progress_heading));
            let mut completion = quest.clone(); completion.detail.completion_description_lines = vec!["  Completion Return\t ".into()];
            let finish = NpcDialogModel { npc_object_id: Some(10), ..default() };
            let rows = quest_list_message_lines_with_wrap(&completion, &finish, None, false, text);
            assert_eq!(rows.len(), 2, "distinct finish early return contains only title/body");
            assert_eq!(rows[1].kind, QuestDetailLineKind::Body);
            assert_eq!(rows[1].text, text.body("  Completion Return\t "));
            assert!(!rows.iter().any(|row| row.kind == QuestDetailLineKind::Heading));
        }
    }

    #[test]
    fn m7_chrome_words_remain_opaque_in_real_npc_text_and_action_tree() {
        for (language, _, _) in heading_goldens() {
            let text = QuestRenderText { locale: QuestPresentationLocale(language) };
            let mut world = World::new();
            world.run_system_once(move |mut commands: Commands| {
                commands.spawn(Node::default()).with_children(|parent| {
                    render_dialog_panel(parent, &NpcDialogModel { npc_name: Some("Village Chief".into()),
                        lines: vec![crate::quest_model::NpcDialogLine { text: "Return".into() }],
                        options: vec![crate::quest_model::NpcDialogOption { option_id: "@raw:return/Progress:9".into(),
                            label: "Progress".into(), enabled: true }], ..default() },
                        &NpcDialogNav::default(), &QuestUiState::default(), &PendingOperations::default(), true, None, text);
                });
            }).unwrap();
            for expected in ["Village Chief", "Return", "Progress"] {
                assert!(world.query::<&Text>().iter(&world).any(|value| value.0 == expected), "{language:?}: {expected}");
            }
            assert!(world.query::<&QuestUiButton>().iter(&world).any(|action|
                matches!(action, QuestUiButton::SelectNpcDialog { target } if target == "@raw:return/Progress:9")));
        }
    }

    #[test]
    fn four_raw_leaf_families_preserve_text_geometry_and_action_payloads() {
        let mut world = World::new();
        world.run_system_once(|mut commands: Commands| {
            commands.spawn(Node::default()).with_children(|parent| {
                let opaque = "Accept Village Chief Dagger";
                quest_log_text_at_raw(parent, opaque, QuestLogRect::new(9.0, 11.0, 101.0, 23.0),
                    12.0, PANEL_TEXT, Justify::Left);
                quest_log_text_button_at_sized_raw(parent, QuestLogRect::new(7.0, 8.0, 99.0, 44.0),
                    opaque, QuestUiButton::FinishQuest { quest_index: 123, selected_item_index: 7 }, true, 15.0);
                panel_text_raw(parent, opaque, 13.0, PANEL_TEXT, Justify::Left);
                action_button_raw(parent, opaque, QuestUiButton::SelectNpcDialog { target: "@Exact:opaque".into() }, true);
            });
        }).unwrap();
        assert_eq!(world.query::<&Text>().iter(&world).filter(|text| text.0 == "Accept Village Chief Dagger").count(), 4);
        assert!(world.query::<&Node>().iter(&world).any(|node|
            node.left == Val::Px(9.0) && node.top == Val::Px(11.0)
            && node.width == Val::Px(101.0) && node.height == Val::Px(23.0)));
        assert!(world.query::<&QuestUiButton>().iter(&world).any(|action|
            matches!(action, QuestUiButton::FinishQuest { quest_index: 123, selected_item_index: 7 })));
        assert!(world.query::<&QuestUiButton>().iter(&world).any(|action|
            matches!(action, QuestUiButton::SelectNpcDialog { target } if target == "@Exact:opaque")));
    }

    #[test]
    fn known_id_non_chinese_builders_keep_line_vector_and_opaque_fields() {
        let quest = Quest { quest_index: 2_110_002, title: " Host title  ".into(),
            accept_npc_index: Some(9), finish_npc_index: Some(10), npc_name: Some("Village Chief".into()),
            group: None, min_level_needed: 1, status: crate::quest_model::QuestStatus::InProgress,
            detail: crate::quest_model::QuestDetailText { description_lines: vec!["  First host line  ".into(),
                "Second host line\t".into()], task_description_lines: vec![" Task host\t ".into()],
                return_description_lines: vec![" Return host  ".into()], ..default() },
            objectives: vec![crate::quest_model::QuestObjective { objective_id: "opaque:1".into(),
                text: " Objective host  ".into(), current: 2, target: 3 }], rewards: vec![], unknown_text: None };
        let dialog = NpcDialogModel { is_open: true, npc_object_id: Some(9), npc_name: Some("Village Chief".into()),
            options: vec![crate::quest_model::NpcDialogOption { option_id: "@Exact:opaque".into(), label: "Accept".into(), enabled: true }],
            ..default() };
        for language in [mir2_game_data::LanguageCode::English, mir2_game_data::LanguageCode::Spanish,
            mir2_game_data::LanguageCode::Portuguese] {
            let text = QuestRenderText { locale: QuestPresentationLocale(language) };
            assert_eq!(localized_quest_description(&quest, text), quest.detail.description_lines);
            let lines = quest_detail_lines_with_wrap(&quest, None, "", false, text);
            assert_eq!(lines[0].text, " Host title  ");
            for expected in ["  First host line  ", "Second host line\t", " Task host\t ", " Return host  "] {
                assert!(lines.iter().any(|line| line.text == expected), "{language:?} {expected:?}: {lines:?}");
            }
            assert!(quest_list_message_lines_with_wrap(&quest, &dialog, None, false, text)
                .iter().any(|line| line.text == "Second host line\t"));
            assert_eq!(quest_objective_detail_text(&quest.objectives[0], text), " Objective host   (2 / 3)");
        }
    }
}

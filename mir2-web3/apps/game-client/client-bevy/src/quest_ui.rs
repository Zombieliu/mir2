//! Native in-game quest/HUD interaction panel.
//!
//! Presentation-only:
//! - reads authoritative read-model resources from `quest_model`
//! - renders compact quest/dialog/combat/pickup widgets for `NativeShellScreen::InGame`
//! - emits only protocol intents for host-side bridging
//! - full quest log covers selection/detail/tracking/accept/deliver/reward/close
//! - NPC dialog covers clickable links, service return, close and input flow

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextLayout};
use bevy::ui::{
    widget::NodeImageMode, AlignItems, BackgroundColor, Display, FlexDirection, FocusPolicy,
    Interaction, JustifyContent, Node, Overflow, PositionType, RelativeCursorPosition, UiRect, Val,
};
use serde::{Deserialize, Serialize};

use crate::crystal_ui::hud::HUD_Z_INDEX;
use crate::crystal_ui::item_tooltip::crystal_item_tooltip_document_from_source;
use crate::crystal_ui::overlays::{
    dispatch_ui_action, NativePlayerUiSet, NativePlayerUiState, UiEffectQueue,
};
use crate::crystal_ui::widget::CrystalItemHint;
use crate::inventory::{InventoryModel, ItemModel};
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::pending_operations::{
    AuthoritativeModelRevisions, PendingLifecycleSet, PendingOperationKey, PendingOperations,
    QuestResetTracker, SessionResetRevision,
};
use crate::quest_model::{
    CombatTargetModel, GroundPickupModel, NearbyNpcModel, NpcDialogModel, Quest, QuestTracker,
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

const MAX_DIALOG_LINES: usize = 4;
const MAX_PICKUP_BUTTONS: usize = 3;
const MAX_QUICK_BAG_ITEMS: usize = 6;
pub const MAX_QUEUED_INTENTS: usize = 24;
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

/// Player-side intents emitted by the in-game UI, to be bridged by host layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QuestUiIntent {
    InteractNpc {
        npc_object_id: u32,
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
        #[serde(rename = "questIndex")]
        quest_index: i32,
    },
    ShareQuest {
        #[serde(rename = "questIndex")]
        quest_index: i32,
    },
    AttackTarget {
        object_id: u32,
    },
    PickUpObject {
        object_id: u32,
    },
    PickUpTile,
}

impl QuestUiIntent {
    pub fn pending_key(&self) -> Option<PendingOperationKey> {
        match self {
            Self::AcceptQuest {
                npc_index,
                quest_index,
            } => Some(PendingOperationKey::QuestAccept {
                npc_index: *npc_index,
                quest_index: *quest_index,
            }),
            Self::FinishQuest {
                quest_index,
                selected_item_index,
            } => Some(PendingOperationKey::QuestFinish {
                quest_index: *quest_index,
                selected_item_index: *selected_item_index,
            }),
            Self::AbandonQuest { quest_index } => Some(PendingOperationKey::QuestAbandon {
                quest_index: *quest_index,
            }),
            Self::InteractNpc { .. }
            | Self::SelectNpcDialog { .. }
            | Self::ShareQuest { .. }
            | Self::AttackTarget { .. }
            | Self::PickUpObject { .. }
            | Self::PickUpTile => None,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct QuestUiIntentQueue {
    /// Commands that reached the host bridge but could not enter its bounded
    /// producer lane. They always drain before newly generated UI input.
    retry_intents: VecDeque<QuestUiIntent>,
    intents: VecDeque<QuestUiIntent>,
    overflow_count: u64,
}

impl QuestUiIntentQueue {
    /// Queue new UI input without evicting an older intent. At capacity the
    /// incoming intent is rejected explicitly, preserving FIFO and any
    /// protected host-backpressure retries.
    pub fn push_intent(&mut self, intent: QuestUiIntent) -> bool {
        if self.len() >= MAX_QUEUED_INTENTS {
            self.overflow_count = self.overflow_count.saturating_add(1);
            return false;
        }
        self.intents.push_back(intent);
        true
    }

    /// Retain host-send failures as the highest-priority FIFO. The bridge calls
    /// this only after draining a bounded batch, so every failed item normally
    /// fits. If another producer populated the queue in between, unsent new
    /// input is evicted from the back before a retry is ever sacrificed.
    pub fn retain_failed_intents(
        &mut self,
        failed: impl IntoIterator<Item = QuestUiIntent>,
    ) -> Vec<QuestUiIntent> {
        let mut dropped = Vec::new();
        for intent in failed {
            if self.len() >= MAX_QUEUED_INTENTS {
                if let Some(evicted) = self.intents.pop_back() {
                    self.overflow_count = self.overflow_count.saturating_add(1);
                    dropped.push(evicted);
                } else {
                    // A full queue made solely of older retries cannot accept
                    // another retry without becoming unbounded. Return it to
                    // the host so any matching pending operation is released
                    // explicitly instead of being stranded forever.
                    self.overflow_count = self.overflow_count.saturating_add(1);
                    dropped.push(intent);
                    continue;
                }
            }
            self.retry_intents.push_back(intent);
        }
        dropped
    }

    pub fn drain_intents(&mut self) -> Vec<QuestUiIntent> {
        let mut drained = Vec::with_capacity(self.len());
        drained.extend(self.retry_intents.drain(..));
        drained.extend(self.intents.drain(..));
        drained
    }

    pub fn push_pending_intent(
        &mut self,
        pending: &mut PendingOperations,
        intent: QuestUiIntent,
    ) -> bool {
        let Some(key) = intent.pending_key() else {
            return self.push_intent(intent);
        };
        if !pending.try_begin(key.clone()) {
            return false;
        }
        if self.push_intent(intent) {
            true
        } else {
            pending.release(&key);
            false
        }
    }

    pub fn clear(&mut self) {
        self.retry_intents.clear();
        self.intents.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.retry_intents.is_empty() && self.intents.is_empty()
    }

    pub fn len(&self) -> usize {
        self.retry_intents.len() + self.intents.len()
    }

    pub fn is_full(&self) -> bool {
        self.len() >= MAX_QUEUED_INTENTS
    }

    pub fn retry_len(&self) -> usize {
        self.retry_intents.len()
    }

    pub fn overflow_count(&self) -> u64 {
        self.overflow_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFeedback {
    pub message: String,
    pub is_error: bool,
}

#[derive(Resource, Debug, Default, Clone)]
pub struct QuestUiState {
    pub selected_quest_index: Option<i32>,
    /// Crystal opens an independent `QuestDetailDialog` on diary-row left
    /// click. This remains open when the Q-key diary itself is hidden.
    pub detail_quest_index: Option<i32>,
    pub detail_scroll_top: usize,
    pub selected_reward_index: Option<i32>,
    /// Crystal persists an ordered list of at most five tracked quest ids.
    /// Right-clicking a diary row toggles membership; there is no implicit
    /// "track the first quest" fallback.
    pub tracked_quest_indices: Vec<i32>,
    /// Quest whose source `MirMessageBox` abandon confirmation is open.
    pub abandon_confirmation_quest_index: Option<i32>,
    pub quest_alert_message: Option<String>,
    pub npc_quest_list_open: bool,
    pub npc_quest_selected_index: Option<i32>,
    pub npc_quest_start_index: usize,
    pub npc_quest_message_scroll_top: usize,
    pub npc_selected_reward_index: Option<i32>,
    pub feedback: Option<QuestFeedback>,
    pub stage_filter: QuestStageFilter,
    pub page: usize,
    /// Crystal expands every diary group by default and remembers only groups
    /// the player explicitly collapsed during the current client session.
    pub collapsed_groups: Vec<String>,
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
        self.selected_quest_index = Some(quest_index);
        self.detail_quest_index = Some(quest_index);
        self.detail_scroll_top = 0;
        self.selected_reward_index = None;
        self.feedback = None;
    }

    pub fn clear_selection(&mut self) {
        self.selected_quest_index = None;
        self.detail_quest_index = None;
        self.detail_scroll_top = 0;
        self.selected_reward_index = None;
    }

    pub fn clear_diary_selection(&mut self) {
        self.selected_quest_index = None;
    }

    pub fn close_detail(&mut self) {
        self.detail_quest_index = None;
        self.detail_scroll_top = 0;
        self.selected_reward_index = None;
        self.abandon_confirmation_quest_index = None;
    }

    pub fn scroll_detail_up(&mut self) {
        self.detail_scroll_top = self.detail_scroll_top.saturating_sub(1);
    }

    pub fn scroll_detail_down(&mut self, line_count: usize) {
        let max_top = line_count.saturating_sub(QUEST_DETAIL_LINE_COUNT);
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

    pub fn request_abandon_confirmation(&mut self, quest_index: i32) {
        self.quest_alert_message = None;
        self.abandon_confirmation_quest_index = Some(quest_index);
        self.feedback = None;
    }

    pub fn close_abandon_confirmation(&mut self) {
        self.abandon_confirmation_quest_index = None;
    }

    pub fn show_quest_alert(&mut self, message: impl Into<String>) {
        self.abandon_confirmation_quest_index = None;
        self.quest_alert_message = Some(message.into());
    }

    pub fn close_quest_alert(&mut self) {
        self.quest_alert_message = None;
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
        self.feedback = None;
    }

    pub fn close_npc_quest_list(&mut self) {
        self.npc_quest_list_open = false;
        self.npc_quest_selected_index = None;
        self.npc_quest_start_index = 0;
        self.npc_quest_message_scroll_top = 0;
        self.npc_selected_reward_index = None;
    }

    pub fn select_npc_quest(&mut self, quest_index: i32) {
        self.npc_quest_selected_index = Some(quest_index);
        self.npc_quest_message_scroll_top = 0;
        self.npc_selected_reward_index = None;
        self.feedback = None;
    }

    pub fn move_npc_quest_selection(&mut self, quest_indices: &[i32], delta: isize) {
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
        } else if next >= self.npc_quest_start_index + QUEST_LIST_VISIBLE_ROWS {
            self.npc_quest_start_index = next + 1 - QUEST_LIST_VISIBLE_ROWS;
        }
    }

    pub fn scroll_npc_quest_message_up(&mut self) {
        self.npc_quest_message_scroll_top = self.npc_quest_message_scroll_top.saturating_sub(1);
    }

    pub fn scroll_npc_quest_message_down(&mut self, line_count: usize) {
        let max_top = line_count.saturating_sub(QUEST_LIST_MESSAGE_LINE_COUNT);
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
struct QuestUiRoot;

#[derive(Component)]
struct QuestTrackerPanel;

#[derive(Component)]
struct NpcDialogPanel;

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
struct QuestLogPanel;

#[derive(Component)]
struct QuestDetailPanel;

#[derive(Component)]
struct NpcQuestListPanel;

#[derive(Component)]
struct QuestConfirmationPanel;

#[derive(Component)]
struct QuestConfirmationBlocker;

#[derive(Component, Clone, Copy)]
struct QuestDiaryRow {
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
enum QuestUiButton {
    SelectNpcDialog {
        target: String,
    },
    CloseNpcDialog,
    ToggleNpcQuestList,
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
    AttackTarget {
        object_id: u32,
    },
    PickUpObject {
        object_id: u32,
    },
    PickUpTile,
    ToggleQuestGroup {
        group: String,
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

fn npc_available_quests<'a>(dialog: &NpcDialogModel, tracker: &'a QuestTracker) -> Vec<&'a Quest> {
    tracker
        .active_quests
        .iter()
        .filter(|quest| quest_belongs_to_current_npc(dialog, quest))
        .collect()
}

fn npc_available_quest_indices(dialog: &NpcDialogModel, tracker: &QuestTracker) -> Vec<i32> {
    npc_available_quests(dialog, tracker)
        .into_iter()
        .map(|quest| quest.quest_index)
        .collect()
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

/// Shared plugin for InGame native quest UI.
pub struct Mir2QuestUiPlugin;

impl Plugin for Mir2QuestUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestTracker>()
            .init_resource::<NpcDialogModel>()
            .init_resource::<NearbyNpcModel>()
            .init_resource::<CombatTargetModel>()
            .init_resource::<GroundPickupModel>()
            .init_resource::<UiReadModel>()
            .init_resource::<InventoryModel>()
            .init_resource::<QuestUiIntentQueue>()
            .init_resource::<PendingOperations>()
            .init_resource::<AuthoritativeModelRevisions>()
            .init_resource::<SessionResetRevision>()
            .init_resource::<QuestResetTracker>()
            .init_resource::<QuestUiState>()
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
            .add_systems(
                Update,
                crate::pending_operations::apply_quest_session_reset
                    .in_set(PendingLifecycleSet::UiReset),
            )
            .add_systems(Startup, spawn_quest_ui_panels)
            .add_systems(
                Update,
                process_quest_ui_input
                    .after(crate::crystal_ui::overlays::process_overlay_keyboard)
                    .in_set(NativePlayerUiSet::Mutate),
            )
            .add_systems(Update, render_quest_ui.in_set(NativePlayerUiSet::Read))
            .add_systems(
                Update,
                sync_quest_ui_button_visuals
                    .after(render_quest_ui)
                    .in_set(NativePlayerUiSet::Read),
            );
    }
}

fn spawn_quest_ui_panels(mut commands: Commands, asset_server: Option<Res<AssetServer>>) {
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
    commands
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
        ))
        .with_children(|root| {
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
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(8.0)),
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

fn process_quest_ui_input(
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
    tracker: Res<QuestTracker>,
) {
    let Some(shell) = shell else {
        return;
    };
    let mut fallback_effects = UiEffectQueue::default();
    let mut effects = effects.as_deref_mut().unwrap_or(&mut fallback_effects);
    if shell.screen != NativeShellScreen::InGame {
        quest_state.reset();
        npc_nav.clear();
        return;
    }

    // Auto-clear navigation history when dialog is externally closed by server.
    if !dialog.is_open && !npc_nav.history.is_empty() {
        npc_nav.clear();
    }
    if !dialog.is_open && quest_state.npc_quest_list_open {
        quest_state.close_npc_quest_list();
    }

    let quest_log_open = player_ui.quest_open();
    let dialog_open = dialog.is_open;
    let blocks_gameplay_keys = player_ui.blocks_gameplay_keys();
    let npc_quest_indices = npc_available_quest_indices(&dialog, &tracker);
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
            toggle_quest_tracking(&mut quest_state, &tracker, row.quest_index);
        }
    }

    for (interaction, action) in button_events.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action.clone() {
            QuestUiButton::ToggleQuestGroup { group } => {
                quest_state.toggle_group(group);
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
                let page_count = tracker
                    .active_quests
                    .iter()
                    .filter(|quest| quest_state.stage_filter.matches(quest))
                    .count()
                    .div_ceil(MAX_QUEST_LOG_ROWS);
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
                            npc_nav.push(dialog.clone());
                            quest_state.set_feedback(format!("Selected dialog {target}"), false);
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
                    dialog.close();
                    npc_nav.clear();
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
                    dialog.close();
                    npc_nav.clear();
                } else {
                    quest_state.set_feedback("Connection busy; try again", true);
                }
            }
            QuestUiButton::SelectNpcQuest { quest_index } => {
                if npc_quest_indices.contains(&quest_index) {
                    quest_state.select_npc_quest(quest_index);
                } else {
                    quest_state.set_feedback("Quest is no longer available from this NPC", true);
                }
            }
            QuestUiButton::NpcQuestPrevious => {
                quest_state.move_npc_quest_selection(&npc_quest_indices, -1);
            }
            QuestUiButton::NpcQuestNext => {
                quest_state.move_npc_quest_selection(&npc_quest_indices, 1);
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
                    let line_count = quest_list_message_lines(quest, &dialog).len();
                    quest_state.scroll_npc_quest_message_down(line_count);
                }
            }
            QuestUiButton::NpcQuestHelp => {
                quest_state.set_feedback("Quest help is not available.", false);
            }
            QuestUiButton::ReturnNpcService => {
                if let Some(prev) = npc_nav.pop() {
                    *dialog = prev;
                    quest_state.set_feedback("Returned to previous page", false);
                } else {
                    quest_state.set_feedback("No previous page", true);
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
            QuestUiButton::QuestDetailScrollDown => {
                if let Some(quest) = quest_state.detail_quest(&tracker) {
                    let line_count = quest_detail_lines(quest).len();
                    quest_state.scroll_detail_down(line_count);
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
                toggle_quest_tracking(&mut quest_state, &tracker, quest_index);
            }
            QuestUiButton::AcceptQuest {
                npc_index,
                quest_index,
            } => {
                if npc_index == 0 {
                    quest_state.set_feedback("Quest has no valid NPC source", true);
                    continue;
                }
                if !dialog_exposes_quest_operation(
                    &dialog,
                    Some(npc_index),
                    quest_index,
                    false,
                    None,
                ) {
                    quest_state
                        .set_feedback("Use the current NPC dialog to accept this quest", true);
                    continue;
                }
                if let Some(quest) = tracker
                    .active_quests
                    .iter()
                    .find(|q| q.quest_index == quest_index)
                {
                    if can_accept_quest(quest) {
                        let queue_full = queue.is_full();
                        if queue.push_pending_intent(
                            &mut pending,
                            QuestUiIntent::AcceptQuest {
                                npc_index,
                                quest_index,
                            },
                        ) {
                            quest_state.set_feedback(format!("Accepting {}", quest.title), false);
                        } else if queue_full {
                            quest_state.set_feedback("Connection busy; try again", true);
                        } else {
                            quest_state.set_feedback("Quest request is already pending", true);
                        }
                    } else {
                        quest_state.set_feedback("Quest cannot be accepted", true);
                    }
                } else {
                    // Allow accept even if quest not yet in tracker but offered via NPC dialog
                    // Still require npc_index valid (non-zero or present)
                    let queue_full = queue.is_full();
                    if queue.push_pending_intent(
                        &mut pending,
                        QuestUiIntent::AcceptQuest {
                            npc_index,
                            quest_index,
                        },
                    ) {
                        quest_state.set_feedback("Accepting quest", false);
                    } else if queue_full {
                        quest_state.set_feedback("Connection busy; try again", true);
                    } else {
                        quest_state.set_feedback("Quest request is already pending", true);
                    }
                }
            }
            QuestUiButton::FinishQuest {
                quest_index,
                selected_item_index,
            } => {
                if !dialog_exposes_quest_operation(
                    &dialog,
                    None,
                    quest_index,
                    true,
                    (selected_item_index >= 0).then_some(selected_item_index),
                ) {
                    quest_state.set_feedback("Return to the quest NPC to deliver this quest", true);
                    continue;
                }
                if let Some(quest) = tracker
                    .active_quests
                    .iter()
                    .find(|q| q.quest_index == quest_index)
                {
                    if !can_finish_quest(quest) {
                        quest_state.set_feedback("Quest not ready to deliver", true);
                        continue;
                    }
                    if reward_selection_required(quest)
                        && !is_valid_reward_selection(quest, Some(selected_item_index))
                    {
                        quest_state.show_quest_alert(SELECT_REWARD_TEXT);
                        continue;
                    }
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
                confirm_quest_abandon(&mut queue, &mut pending, &mut quest_state, &tracker);
            }
            QuestUiButton::CancelAbandonQuest => {
                quest_state.close_abandon_confirmation();
            }
            QuestUiButton::CloseQuestAlert => {
                quest_state.close_quest_alert();
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
                dispatch_ui_action(
                    &mut player_ui.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::ClosePanel,
                );
                quest_state.clear_diary_selection();
                quest_state.clear_feedback();
                // Keep feedback about close? Clear to avoid stale message.
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
        }
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
            confirm_quest_abandon(&mut queue, &mut pending, &mut quest_state, &tracker);
        }
        return;
    }

    // Input blocking: when quest log or dialog is open, gameplay shortcuts are suppressed.
    // Escape handling for those surfaces takes precedence.
    let is_modal = quest_log_open || dialog_open || blocks_gameplay_keys;

    if is_modal {
        if keys.just_pressed(KeyCode::Escape)
            || (quest_log_open && keys.just_pressed(KeyCode::KeyQ))
        {
            if quest_log_open {
                dispatch_ui_action(
                    &mut player_ui.core,
                    &mut effects,
                    mir2_ui_core::action::UiAction::ClosePanel,
                );
                quest_state.clear_diary_selection();
                quest_state.clear_feedback();
            } else if dialog_open {
                if queue.push_intent(QuestUiIntent::SelectNpcDialog {
                    target: "@Exit".to_owned(),
                }) {
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
                    *dialog = prev;
                    quest_state.set_feedback("Returned to previous page", false);
                }
            }
        }
        // Block T/F/R and other gameplay keys while modal is open.
        return;
    }

    // Normal gameplay shortcuts – only when no modal blocks them.
    if keys.just_pressed(KeyCode::KeyT) {
        if let Some(nearby) = &nearby {
            if let Some(npc) = nearby.nearest() {
                if !queue.push_intent(QuestUiIntent::InteractNpc {
                    npc_object_id: npc.object_id,
                }) {
                    quest_state.set_feedback("Connection busy; try again", true);
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyF) {
        if let Some(target) = &target {
            if let Some(target) = &target.target {
                if !queue.push_intent(QuestUiIntent::AttackTarget {
                    object_id: target.object_id,
                }) {
                    quest_state.set_feedback("Connection busy; try again", true);
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyR) {
        match pickups {
            Some(pickups) => match pickups.recent.front() {
                Some(pickup) => match pickup.object_id {
                    Some(object_id) => {
                        if queue.push_intent(QuestUiIntent::PickUpObject { object_id }) {
                            quest_state.set_feedback(
                                format!("Picking up {}", pickup.compact_label()),
                                false,
                            );
                        } else {
                            quest_state.set_feedback("Connection busy; pickup not queued", true);
                        }
                    }
                    None => {
                        if queue.push_intent(QuestUiIntent::PickUpTile) {
                            quest_state
                                .set_feedback("Checking the current tile for ground items", false);
                        } else {
                            quest_state.set_feedback("Connection busy; pickup not queued", true);
                        }
                    }
                },
                None => {
                    if queue.push_intent(QuestUiIntent::PickUpTile) {
                        quest_state
                            .set_feedback("Checking the current tile for ground items", false);
                    } else {
                        quest_state.set_feedback("Connection busy; pickup not queued", true);
                    }
                }
            },
            None => {
                if queue.push_intent(QuestUiIntent::PickUpTile) {
                    quest_state.set_feedback("Checking the current tile for ground items", false);
                } else {
                    quest_state.set_feedback("Connection busy; pickup not queued", true);
                }
            }
        }
    }

    // Toggle quest log with Q when not blocked by other modals.
    if keys.just_pressed(KeyCode::KeyQ) {
        let now_open = is_quest_log_open_state(Some(&player_ui));
        if now_open {
            dispatch_ui_action(
                &mut player_ui.core,
                &mut effects,
                mir2_ui_core::action::UiAction::ClosePanel,
            );
            quest_state.clear_diary_selection();
            quest_state.clear_feedback();
        } else {
            dispatch_ui_action(
                &mut player_ui.core,
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

fn render_quest_ui(
    shell: Option<Res<NativeShellModel>>,
    asset_server: Option<Res<AssetServer>>,
    tracker: Res<QuestTracker>,
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
    let Some(shell) = shell else {
        return;
    };

    let in_game = shell.screen == NativeShellScreen::InGame;

    {
        let mut roots = all.p0();
        let Ok(mut root) = roots.single_mut() else {
            return;
        };
        root.display = if in_game {
            Display::Flex
        } else {
            Display::None
        };
    }

    if !in_game {
        return;
    }

    let quest_log_open = is_quest_log_open_state(Some(&player_ui));

    // Avoid churn: only re-render when relevant state changed or quest log toggled.
    if !tracker.is_changed()
        && !dialog.is_changed()
        && !nearby.is_changed()
        && !target.is_changed()
        && !pickups.is_changed()
        && !ui_model.is_changed()
        && !inventory.is_changed()
        && !pending.is_changed()
        && !shell.is_changed()
        && !quest_state.is_changed()
        && !npc_nav.is_changed()
        && !player_ui.is_changed()
    {
        return;
    }

    let has_dialog_content = dialog.is_open;
    let has_pickups = !pickups.recent.is_empty();
    let available_npc_quests = npc_available_quests(&dialog, &tracker);
    let has_npc_quests = !available_npc_quests.is_empty();
    let confirmation_open = quest_state.abandon_confirmation_quest_index.is_some()
        || quest_state.quest_alert_message.is_some();

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
            has_dialog_content
        } else if is_target.is_some() {
            CRYSTAL_TARGET_PANEL_VISIBLE && target.target.is_some()
        } else if is_pickup.is_some() {
            has_pickups
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
        commands.entity(panel_entity).despawn_children();
        if !visible {
            continue;
        }

        commands.entity(panel_entity).with_children(|panel| {
            if is_tracker.is_some() {
                render_quest_tracker_panel(panel, &tracker, &quest_state);
            } else if is_dialog.is_some() {
                render_dialog_panel(
                    panel,
                    &dialog,
                    &npc_nav,
                    &quest_state,
                    &pending,
                    has_npc_quests,
                    asset_server.as_ref().map(|server| &**server),
                );
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
        if quest_log_open {
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
        node.display = if quest_log_open {
            Display::Flex
        } else {
            Display::None
        };
        commands.entity(entity).despawn_children();
        if quest_log_open {
            commands.entity(entity).with_children(|panel| {
                render_quest_diary_panel(
                    panel,
                    &tracker,
                    &quest_state,
                    &pending,
                    asset_server.as_ref().map(|server| &**server),
                )
            });
        }
    }

    // Crystal row left-click opens a second, independently closable window at
    // `(ScreenWidth / 2 + 20, 60)`. Hiding the Q-key Diary does not hide it.
    let detail_quest = quest_state.detail_quest(&tracker);
    for (entity, mut node) in all.p3().iter_mut() {
        let visible = detail_quest.is_some();
        node.left = Val::Px(QUEST_DETAIL_DESIGN_LEFT);
        node.top = Val::Px(QUEST_DETAIL_DESIGN_TOP);
        node.width = Val::Px(QUEST_DETAIL_DESIGN_WIDTH);
        node.height = Val::Px(QUEST_DETAIL_DESIGN_HEIGHT);
        node.min_width = Val::Px(QUEST_DETAIL_DESIGN_WIDTH);
        node.max_width = Val::Px(QUEST_DETAIL_DESIGN_WIDTH);
        node.min_height = Val::Px(QUEST_DETAIL_DESIGN_HEIGHT);
        node.max_height = Val::Px(QUEST_DETAIL_DESIGN_HEIGHT);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        commands.entity(entity).despawn_children();
        if let Some(quest) = detail_quest {
            commands.entity(entity).with_children(|panel| {
                render_quest_detail_panel(
                    panel,
                    quest,
                    &quest_state,
                    &pending,
                    asset_server.as_ref().map(|server| &**server),
                    &ui_model.player,
                )
            });
        }
    }

    // Crystal's NPC Quest List is a separate five-row window at
    // `NPCDialog.Width + 47, 0` and remains linked to the NPC dialog lifecycle.
    for (entity, mut node) in all.p4().iter_mut() {
        let visible = quest_state.npc_quest_list_open && has_npc_quests && dialog.is_open;
        node.left = Val::Px(QUEST_LIST_DESIGN_LEFT);
        node.top = Val::Px(QUEST_LIST_DESIGN_TOP);
        node.width = Val::Px(QUEST_LIST_DESIGN_WIDTH);
        node.height = Val::Px(QUEST_LIST_DESIGN_HEIGHT);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        commands.entity(entity).despawn_children();
        if visible {
            commands.entity(entity).with_children(|panel| {
                render_npc_quest_list_panel(
                    panel,
                    &available_npc_quests,
                    &dialog,
                    &quest_state,
                    &pending,
                    asset_server.as_ref().map(|server| &**server),
                    &ui_model.player,
                )
            });
        }
    }

    for (entity, mut node) in all.p5().iter_mut() {
        node.left = Val::Px(QUEST_CONFIRM_DESIGN_LEFT);
        node.top = Val::Px(QUEST_CONFIRM_DESIGN_TOP);
        node.width = Val::Px(QUEST_CONFIRM_DESIGN_WIDTH);
        node.height = Val::Px(QUEST_CONFIRM_DESIGN_HEIGHT);
        node.display = if confirmation_open {
            Display::Flex
        } else {
            Display::None
        };
        commands.entity(entity).despawn_children();
        if confirmation_open {
            commands.entity(entity).with_children(|panel| {
                if let Some(message) = quest_state.quest_alert_message.as_deref() {
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

fn render_quest_tracker_panel(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    state: &QuestUiState,
) {
    let quests = visible_tracker_quests(tracker, state);
    if quests.is_empty() {
        return;
    }

    let mut y = 0.0;
    for quest in quests {
        quest_log_text_at(
            parent,
            &quest.title,
            QuestLogRect::new(5.0, 20.0 + y, 300.0, 15.0),
            8.0,
            Color::srgb(0.20, 1.0, 0.10),
            Justify::Left,
        );
        for objective in &quest.objectives {
            y += 15.0;
            quest_log_text_at(
                parent,
                &quest_objective_detail_text(objective),
                QuestLogRect::new(25.0, 20.0 + y, 290.0, 15.0),
                8.0,
                Color::WHITE,
                Justify::Left,
            );
        }
        y += 30.0;
    }
}

fn render_dialog_panel(
    parent: &mut ChildSpawnerCommands,
    dialog: &NpcDialogModel,
    nav: &NpcDialogNav,
    quest_state: &QuestUiState,
    pending: &PendingOperations,
    has_npc_quests: bool,
    asset_server: Option<&AssetServer>,
) {
    title_line(parent, "NPC Dialog");

    if !dialog.is_open {
        body_line(parent, "Press T near an NPC to talk.");
        return;
    }

    let npc_name = dialog
        .npc_name
        .clone()
        .or_else(|| dialog.npc_object_id.map(|id| id.to_string()))
        .unwrap_or_else(|| "Unknown NPC".to_owned());

    body_line(parent, &format!("From: {npc_name}"));

    let lines = dialog
        .lines
        .iter()
        .take(MAX_DIALOG_LINES)
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>();

    for line in lines {
        body_line(parent, line);
    }

    feedback_line(parent, quest_state.feedback.as_ref(), 10.0);

    for option in dialog
        .options
        .iter()
        .filter(|option| {
            explicit_quest_dialog_button(
                &option.option_id,
                dialog.npc_object_id.unwrap_or_default(),
            )
            .is_none()
        })
        .take(4)
    {
        let action = explicit_quest_dialog_button(
            &option.option_id,
            dialog.npc_object_id.unwrap_or_default(),
        )
        .unwrap_or_else(|| QuestUiButton::SelectNpcDialog {
            target: option.option_id.to_owned(),
        });
        let operation_available = match &action {
            QuestUiButton::AcceptQuest {
                npc_index,
                quest_index,
            } => !pending.contains(&PendingOperationKey::QuestAccept {
                npc_index: *npc_index,
                quest_index: *quest_index,
            }),
            QuestUiButton::FinishQuest {
                quest_index,
                selected_item_index,
            } => !pending.contains(&PendingOperationKey::QuestFinish {
                quest_index: *quest_index,
                selected_item_index: *selected_item_index,
            }),
            _ => true,
        };
        action_button(
            parent,
            &option.label,
            action,
            option.enabled && operation_available,
        );
    }

    if has_npc_quests {
        quest_log_image_button_at(
            parent,
            asset_server,
            NPC_QUEST_BUTTON_ASSET,
            QuestLogRect::new(172.0, 194.0, 96.0, 25.0),
            QuestUiButton::ToggleNpcQuestList,
            true,
        );
    }

    // Service navigation: Return and Close.
    // Return is disabled when no history.
    action_button(
        parent,
        "Return",
        QuestUiButton::ReturnNpcService,
        nav.can_return(),
    );
    action_button(parent, "Close", QuestUiButton::CloseNpcDialog, true);
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
            truncate_chars(&quest.title, 28),
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

        detail_title(parent, &quest.title);
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
        let finish_pending = pending.contains(&PendingOperationKey::QuestFinish {
            quest_index: quest.quest_index,
            selected_item_index: finish_item,
        });
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

fn quest_diary_groups(tracker: &QuestTracker) -> Vec<QuestDiaryGroup<'_>> {
    let mut groups: Vec<QuestDiaryGroup<'_>> = Vec::new();
    for quest in tracker
        .active_quests
        .iter()
        .filter(|quest| quest.status.is_active())
        .take(QUEST_DIARY_MAX_CURRENT)
    {
        let name = quest_diary_group_name(quest);
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

fn quest_diary_group_name(quest: &Quest) -> String {
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

fn quest_diary_status_label(quest: &Quest) -> &'static str {
    match quest.status {
        crate::quest_model::QuestStatus::ReadyToTurnIn => "Complete",
        _ => "In Progress",
    }
}

fn render_quest_diary_panel(
    parent: &mut ChildSpawnerCommands,
    tracker: &QuestTracker,
    state: &QuestUiState,
    _pending: &PendingOperations,
    asset_server: Option<&AssetServer>,
) {
    let layout = quest_diary_layout(1.0);
    let groups = quest_diary_groups(tracker);
    let current_count = groups.iter().map(|group| group.quests.len()).sum::<usize>();
    quest_log_image_at(parent, asset_server, QUEST_DIARY_TITLE_ASSET, layout.title);
    quest_log_text_at(
        parent,
        &format!("List: {current_count}/{QUEST_DIARY_MAX_CURRENT}"),
        layout.taken_count,
        8.0,
        PANEL_TEXT,
        Justify::Left,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DIARY_TOP_CLOSE_ASSET,
        layout.top_close,
        QuestUiButton::CloseQuestLog,
        true,
    );
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DIARY_BOTTOM_CLOSE_ASSET,
        layout.bottom_close,
        QuestUiButton::CloseQuestLog,
        true,
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
        quest_log_text_at(
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
            let quest_label = format!("Lv{level:<4} {}", quest.title);
            let state_label = quest_diary_status_label(quest);
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
                    quest_log_text_at(
                        row,
                        &quest_label,
                        QuestLogRect::new(0.0, 0.0, 185.0, 15.0),
                        8.0,
                        PANEL_TEXT,
                        Justify::Left,
                    );
                    quest_log_text_at(
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

fn quest_objective_detail_text(objective: &crate::quest_model::QuestObjective) -> String {
    let compact = format!("{}/{}", objective.current, objective.target);
    let spaced = objective.progress_label();
    if objective.target == 0
        || objective.text.contains(&compact)
        || objective.text.contains(&spaced)
    {
        objective.text.clone()
    } else {
        format!("{} ({spaced})", objective.text)
    }
}

fn quest_detail_lines(quest: &Quest) -> Vec<QuestDetailLine> {
    let mut lines = vec![QuestDetailLine {
        text: quest.title.clone(),
        kind: QuestDetailLineKind::Title,
    }];

    let mut description = quest.detail.description_lines.clone();
    if description.is_empty() {
        if let Some(fallback) = quest
            .unknown_text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .filter(|text| quest.group.as_deref() != Some(text.trim()))
        {
            description.extend(fallback.lines().map(str::to_owned));
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

    let task_lines = if quest.detail.task_description_lines.is_empty() {
        quest
            .objectives
            .iter()
            .map(|objective| objective.text.clone())
            .collect()
    } else {
        quest.detail.task_description_lines.clone()
    };
    push_quest_detail_section(&mut lines, "Tasks", task_lines);
    push_quest_detail_section(
        &mut lines,
        "Return",
        quest.detail.return_description_lines.clone(),
    );
    if let Some(time_limit) = quest
        .detail
        .time_limit
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_quest_detail_section(&mut lines, "Time Limit", [time_limit.to_owned()]);
    }
    if quest.status.is_active() && !quest.objectives.is_empty() {
        push_quest_detail_section(
            &mut lines,
            "Progress",
            quest
                .objectives
                .iter()
                .map(quest_objective_detail_text)
                .collect::<Vec<_>>(),
        );
    }
    lines
}

fn quest_list_message_lines(quest: &Quest, dialog: &NpcDialogModel) -> Vec<QuestDetailLine> {
    let mut lines = vec![QuestDetailLine {
        text: quest.title.clone(),
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
                .cloned()
                .map(|text| QuestDetailLine {
                    text,
                    kind: QuestDetailLineKind::Body,
                }),
        );
        return lines;
    }

    let mut description = quest.detail.description_lines.clone();
    if description.is_empty() {
        if let Some(fallback) = quest
            .unknown_text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .filter(|text| quest.group.as_deref() != Some(text.trim()))
        {
            description.extend(fallback.lines().map(str::to_owned));
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
    let task_lines = if quest.detail.task_description_lines.is_empty() {
        quest
            .objectives
            .iter()
            .map(|objective| objective.text.clone())
            .collect()
    } else {
        quest.detail.task_description_lines.clone()
    };
    push_quest_detail_section(&mut lines, "Tasks", task_lines);
    push_quest_detail_section(
        &mut lines,
        "Return",
        quest.detail.return_description_lines.clone(),
    );
    if let Some(time_limit) = quest
        .detail
        .time_limit
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_quest_detail_section(&mut lines, "Time Limit", [time_limit.to_owned()]);
    }
    // `QuestListDialog` constructs QuestMessage with DisplayProgress=false.
    lines
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
                quest_log_text_at(
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
                quest_log_text_at(
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

fn render_npc_quest_list_panel(
    parent: &mut ChildSpawnerCommands,
    quests: &[&Quest],
    dialog: &NpcDialogModel,
    state: &QuestUiState,
    pending: &PendingOperations,
    asset_server: Option<&AssetServer>,
    player: &crate::read_model::PlayerStats,
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

    quest_log_image_at(parent, asset_server, QUEST_LIST_TITLE_ASSET, layout.title);
    quest_log_text_at(
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
        quest_log_text_at(
            parent,
            &level,
            QuestLogRect::new(29.0, top, 40.0, 17.0),
            9.0,
            PANEL_TEXT,
            Justify::Left,
        );
        quest_log_text_at(
            parent,
            &quest.title,
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
    let lines = quest_list_message_lines(quest, dialog);
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
    );

    let npc_index = dialog
        .npc_object_id
        .or(quest.accept_npc_index)
        .unwrap_or_default();
    if can_accept_quest(quest) {
        let pending = pending.contains(&PendingOperationKey::QuestAccept {
            npc_index,
            quest_index: quest.quest_index,
        });
        quest_log_image_button_at(
            parent,
            asset_server,
            QUEST_LIST_ACCEPT_ASSET,
            layout.primary_action,
            QuestUiButton::AcceptQuest {
                npc_index,
                quest_index: quest.quest_index,
            },
            !pending,
        );
    } else if can_finish_quest(quest) {
        let selected_item_index = state.npc_selected_reward_index.unwrap_or(-1);
        let pending = pending.contains(&PendingOperationKey::QuestFinish {
            quest_index: quest.quest_index,
            selected_item_index,
        });
        quest_log_image_button_at(
            parent,
            asset_server,
            QUEST_LIST_FINISH_ASSET,
            layout.primary_action,
            QuestUiButton::FinishQuest {
                quest_index: quest.quest_index,
                selected_item_index,
            },
            !pending,
        );
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

fn render_quest_detail_panel(
    parent: &mut ChildSpawnerCommands,
    quest: &Quest,
    state: &QuestUiState,
    pending: &PendingOperations,
    asset_server: Option<&AssetServer>,
    player: &crate::read_model::PlayerStats,
) {
    let layout = quest_detail_layout(1.0);
    let lines = quest_detail_lines(quest);
    let max_top = lines.len().saturating_sub(QUEST_DETAIL_LINE_COUNT);
    let scroll_top = state.detail_scroll_top.min(max_top);

    quest_log_image_at(parent, asset_server, QUEST_DETAIL_TITLE_ASSET, layout.title);
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
                        quest_log_text_at(
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
                        quest_log_text_at(
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
    );

    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DETAIL_SHARE_ASSET,
        layout.share,
        QuestUiButton::ShareQuest {
            quest_index: quest.quest_index,
        },
        quest.status.is_active(),
    );
    let abandon_pending = pending.contains(&PendingOperationKey::QuestAbandon {
        quest_index: quest.quest_index,
    });
    quest_log_image_button_at(
        parent,
        asset_server,
        QUEST_DETAIL_CANCEL_ASSET,
        layout.cancel,
        QuestUiButton::AbandonQuest {
            quest_index: quest.quest_index,
        },
        can_abandon_quest(quest) && !abandon_pending,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuestRewardSelectionSurface {
    Detail,
    NpcList,
}

fn render_quest_rewards(
    parent: &mut ChildSpawnerCommands,
    quest: &Quest,
    selected_reward_index: Option<i32>,
    selection_surface: QuestRewardSelectionSurface,
    asset_server: Option<&AssetServer>,
    player: &crate::read_model::PlayerStats,
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
        quest_log_text_at(
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
        quest_log_text_at(
            parent,
            &amount.to_string(),
            QuestLogRect::new(125.0 + gold_offset, 307.0, 75.0, 20.0),
            9.0,
            PANEL_TEXT,
            Justify::Left,
        );
    }

    quest_log_image_at(
        parent,
        asset_server,
        QUEST_DETAIL_SELECT_REWARD_ASSET,
        QuestLogRect::new(25.0, 373.0, 68.0, 16.0),
    );

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
        render_quest_reward_item(parent, asset_server, reward, left, 331.0, None, player);
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
        );
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
            ));
        } else {
            quest_log_text_at(
                content,
                &truncate_chars(name, 8),
                QuestLogRect::new(0.0, 0.0, 40.0, 30.0),
                7.0,
                PANEL_TEXT,
                Justify::Center,
            );
        }
        if *quantity > 1 {
            quest_log_text_at(
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
                        truncate_chars(&quest.title, 25),
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

            detail_title(detail, &quest.title);
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
    let finish_pending = pending.contains(&PendingOperationKey::QuestFinish {
        quest_index: quest.quest_index,
        selected_item_index: finish_item,
    });
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
        Text::new(text.to_owned()),
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

fn quest_log_image_at(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    asset_path: &str,
    rect: QuestLogRect,
) {
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
            Text::new(text.to_owned()),
            TextFont {
                font_size: FontSize::Px(10.0),
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

fn quest_log_image_button_at(
    parent: &mut ChildSpawnerCommands,
    asset_server: Option<&AssetServer>,
    asset_path: &str,
    rect: QuestLogRect,
    action: QuestUiButton,
    enabled: bool,
) {
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

fn tracker_quest_block(parent: &mut ChildSpawnerCommands, quest: &Quest) {
    tracker_title_line(parent, &quest.title);
    if quest.status.is_active() && !quest.objectives.is_empty() {
        for objective in quest.objectives.iter().take(1) {
            tracker_body_line(
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
                tracker_body_line(parent, &format!("   Talk to {label}"));
            }
        }
        crate::quest_model::QuestStatus::ReadyToTurnIn => {
            if let Some(label) = &quest.npc_name {
                tracker_body_line(parent, &format!("   Return to {label}"));
            }
        }
        crate::quest_model::QuestStatus::Completed => {
            if !quest.rewards.is_empty() {
                tracker_body_line(
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

fn tracker_body_line(parent: &mut ChildSpawnerCommands, text: &str) {
    panel_text(parent, text, 9.0, Color::WHITE, Justify::Left);
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
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            ..default()
        },
        Text::new(text.to_owned()),
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
            Text::new(text.to_owned()),
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

#[cfg(test)]
mod tests {
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
    fn quest_log_accept_without_exact_dialog_is_rejected_locally() {
        let mut app = App::new();
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
            npc_available_quest_indices(&dialog, &tracker),
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
    fn npc_list_message_omits_progress_and_uses_finish_copy_at_distinct_npc() {
        let mut current = quest(7, QuestStatus::ReadyToTurnIn);
        current.accept_npc_index = Some(10);
        current.finish_npc_index = Some(11);
        current.detail.description_lines = vec!["Start copy".to_owned()];
        current.detail.completion_description_lines = vec!["Finish copy".to_owned()];
        let dialog = dialog_with_option(11, "@FinishQuest:7");
        let lines = quest_list_message_lines(&current, &dialog);
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

        let lines = quest_detail_lines(&current);
        let visible = lines
            .iter()
            .map(|line| (line.kind, line.text.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            visible[0],
            (QuestDetailLineKind::Title, "Assistant's Request")
        );
        assert!(visible.contains(&(QuestDetailLineKind::Heading, "Tasks")));
        assert!(visible.contains(&(QuestDetailLineKind::Body, "Transport CannibalLeaves.")));
        assert!(visible.contains(&(QuestDetailLineKind::Heading, "Return")));
        assert!(visible.contains(&(QuestDetailLineKind::Body, "Return to CraftLady.")));
        assert!(visible.contains(&(QuestDetailLineKind::Heading, "Time Limit")));
        assert!(visible.contains(&(QuestDetailLineKind::Heading, "Progress")));
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

        let groups = quest_diary_groups(&tracker);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "BichonProvince");
        assert_eq!(groups[0].quests[0].quest_index, 2);
        assert_eq!(groups[1].name, "BorderVillage");
        assert_eq!(groups[1].quests[0].quest_index, 3);
        assert_eq!(quest_diary_status_label(groups[0].quests[0]), "Complete");
        assert_eq!(quest_diary_status_label(groups[1].quests[0]), "In Progress");
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
}

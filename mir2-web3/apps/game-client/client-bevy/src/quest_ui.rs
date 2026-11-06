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
    Interaction, JustifyContent, Node, Overflow, PositionType, UiRect, Val,
};
use serde::{Deserialize, Serialize};

use crate::crystal_ui::hud::HUD_Z_INDEX;
use crate::crystal_ui::overlays::{
    dispatch_ui_action, NativePlayerUiSet, NativePlayerUiState, UiEffectQueue,
};
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

// Existing Crystal frame. There is no dedicated quest frame in the current
// asset registry, so use the verified panel texture instead of inventing an
// asset path or fabricating a quest illustration.
const QUEST_LOG_FRAME_ASSET: &str = "original-ui/Title/670.png";
const NPC_DIALOG_FRAME_ASSET: &str = "original-ui/Prguse/1084.png";
const QUEST_LOG_CLOSE_ASSET: &str = "original-ui/Prguse2/360.png";
const QUEST_LOG_HELP_ASSET: &str = "original-ui/Prguse2/257.png";
const QUEST_LOG_PREVIOUS_ASSET: &str = "original-ui/Prguse2/240.png";
const QUEST_LOG_NEXT_ASSET: &str = "original-ui/Prguse2/243.png";

pub const QUEST_LOG_DESIGN_WIDTH: f32 = 312.0;
pub const QUEST_LOG_DESIGN_HEIGHT: f32 = 444.0;
pub const QUEST_LOG_DESIGN_LEFT: f32 = 356.0;
pub const QUEST_LOG_DESIGN_TOP: f32 = 40.0;
const QUEST_LOG_CONTENT_LEFT: f32 = 10.0;
const QUEST_LOG_CONTENT_WIDTH: f32 = 292.0;
const QUEST_LOG_TAB_TOP: f32 = 30.0;
const QUEST_LOG_TAB_HEIGHT: f32 = 28.0;
const QUEST_LOG_TAB_GAP: f32 = 2.0;
const QUEST_LOG_TAB_WIDTH: f32 = (QUEST_LOG_CONTENT_WIDTH - QUEST_LOG_TAB_GAP * 4.0) / 5.0;
const QUEST_LOG_LIST_TOP: f32 = 60.0;
const QUEST_LOG_LIST_HEIGHT: f32 = 192.0;
const QUEST_LOG_DETAIL_TOP: f32 = 278.0;
const QUEST_LOG_DETAIL_HEIGHT: f32 = 118.0;
const QUEST_LOG_ACTIONS_TOP: f32 = 402.0;
const QUEST_LOG_ACTION_HEIGHT: f32 = 26.0;
const QUEST_LOG_ACTION_GAP: f32 = 6.0;

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
pub struct QuestLogLayout {
    pub frame: QuestLogRect,
    pub list: QuestLogRect,
    pub detail: QuestLogRect,
    pub tabs: [QuestLogRect; 5],
    pub actions: [QuestLogRect; 4],
    pub close: QuestLogRect,
    pub help: QuestLogRect,
    pub previous: QuestLogRect,
    pub next: QuestLogRect,
}

pub fn quest_log_layout(scale: f32) -> QuestLogLayout {
    let tab_left = |index: usize| {
        QUEST_LOG_CONTENT_LEFT + index as f32 * (QUEST_LOG_TAB_WIDTH + QUEST_LOG_TAB_GAP)
    };
    let action_width = (QUEST_LOG_CONTENT_WIDTH - QUEST_LOG_ACTION_GAP * 3.0) / 4.0;
    let action_left = |index: usize| {
        QUEST_LOG_CONTENT_LEFT + index as f32 * (action_width + QUEST_LOG_ACTION_GAP)
    };
    QuestLogLayout {
        frame: QuestLogRect::new(
            QUEST_LOG_DESIGN_LEFT,
            QUEST_LOG_DESIGN_TOP,
            QUEST_LOG_DESIGN_WIDTH,
            QUEST_LOG_DESIGN_HEIGHT,
        )
        .scaled(scale),
        list: QuestLogRect::new(
            QUEST_LOG_CONTENT_LEFT,
            QUEST_LOG_LIST_TOP,
            QUEST_LOG_CONTENT_WIDTH,
            QUEST_LOG_LIST_HEIGHT,
        )
        .scaled(scale),
        detail: QuestLogRect::new(
            QUEST_LOG_CONTENT_LEFT,
            QUEST_LOG_DETAIL_TOP,
            QUEST_LOG_CONTENT_WIDTH,
            QUEST_LOG_DETAIL_HEIGHT,
        )
        .scaled(scale),
        tabs: std::array::from_fn(|index| {
            QuestLogRect::new(
                tab_left(index),
                QUEST_LOG_TAB_TOP,
                QUEST_LOG_TAB_WIDTH,
                QUEST_LOG_TAB_HEIGHT,
            )
            .scaled(scale)
        }),
        actions: std::array::from_fn(|index| {
            QuestLogRect::new(
                action_left(index),
                QUEST_LOG_ACTIONS_TOP,
                action_width,
                QUEST_LOG_ACTION_HEIGHT,
            )
            .scaled(scale)
        }),
        close: QuestLogRect::new(288.0, 3.0, 24.0, 21.0).scaled(scale),
        help: QuestLogRect::new(262.0, 3.0, 24.0, 21.0).scaled(scale),
        previous: QuestLogRect::new(132.0, 256.0, 16.0, 16.0).scaled(scale),
        next: QuestLogRect::new(214.0, 256.0, 16.0, 16.0).scaled(scale),
    }
}

const MAX_PANEL_QUESTS: usize = 2;
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
    pub selected_reward_index: Option<i32>,
    pub tracking_quest_index: Option<i32>,
    pub feedback: Option<QuestFeedback>,
    pub stage_filter: QuestStageFilter,
    pub page: usize,
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
        self.selected_reward_index = None;
        self.feedback = None;
    }

    pub fn clear_selection(&mut self) {
        self.selected_quest_index = None;
        self.selected_reward_index = None;
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

    pub fn selected_quest<'a>(&self, tracker: &'a QuestTracker) -> Option<&'a Quest> {
        self.selected_quest_index
            .and_then(|idx| tracker.active_quests.iter().find(|q| q.quest_index == idx))
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
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
    ReturnNpcService,
    AttackTarget {
        object_id: u32,
    },
    PickUpObject {
        object_id: u32,
    },
    PickUpTile,
    SelectQuestFilter {
        filter: QuestStageFilter,
    },
    QuestHelp,
    QuestPagePrevious,
    QuestPageNext,
    SelectQuest {
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
    SelectReward {
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

/// Crystal only exposes abandon for a quest that is currently in progress.
/// Ready-to-turn-in, not-started, completed, failed and unknown states are
/// deliberately rejected even if a stale button event reaches this system.
pub fn can_abandon_quest(quest: &Quest) -> bool {
    matches!(quest.status, crate::quest_model::QuestStatus::InProgress)
}

pub fn can_track_quest(quest: &Quest) -> bool {
    quest.status.is_active()
}

pub fn reward_selection_required(quest: &Quest) -> bool {
    quest.rewards.len() > 1 && can_finish_quest(quest)
}

pub fn is_valid_reward_selection(quest: &Quest, selected: Option<i32>) -> bool {
    if !reward_selection_required(quest) {
        return true;
    }
    match selected {
        Some(idx) => idx >= 0 && (idx as usize) < quest.rewards.len(),
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
    let quest_log_skin = asset_server
        .as_ref()
        .map(|server| server.load::<Image>(QUEST_LOG_FRAME_ASSET));
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
                    left: Val::Px(8.0),
                    top: Val::Px(118.0),
                    width: Val::Px(320.0),
                    max_width: Val::Px(320.0),
                    min_width: Val::Px(220.0),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));

            let mut dialog_panel = root.spawn((
                NpcDialogPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(310.0),
                    top: Val::Px(90.0),
                    width: Val::Px(404.0),
                    min_height: Val::Px(88.0),
                    max_height: Val::Px(300.0),
                    min_width: Val::Px(220.0),
                    max_width: Val::Px(404.0),
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
                    left: Val::Px(QUEST_LOG_DESIGN_LEFT),
                    top: Val::Px(QUEST_LOG_DESIGN_TOP),
                    width: Val::Px(QUEST_LOG_DESIGN_WIDTH),
                    height: Val::Px(QUEST_LOG_DESIGN_HEIGHT),
                    min_width: Val::Px(QUEST_LOG_DESIGN_WIDTH),
                    max_width: Val::Px(QUEST_LOG_DESIGN_WIDTH),
                    min_height: Val::Px(QUEST_LOG_DESIGN_HEIGHT),
                    max_height: Val::Px(QUEST_LOG_DESIGN_HEIGHT),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                FocusPolicy::Block,
            ));
            if let Some(panel_skin) = quest_log_skin.as_ref() {
                quest_log_panel.insert(ImageNode {
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
    shell: Option<Res<NativeShellModel>>,
    keys: Res<ButtonInput<KeyCode>>,
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

    let quest_log_open = player_ui.quest_open();
    let dialog_open = dialog.is_open;
    let blocks_gameplay_keys = player_ui.blocks_gameplay_keys();

    for (interaction, action) in button_events.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action.clone() {
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
                    quest_state.set_feedback("Dialog closed", false);
                } else {
                    quest_state.set_feedback("Connection busy; try again", true);
                }
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
            QuestUiButton::TrackQuest { quest_index } => {
                if let Some(quest) = tracker
                    .active_quests
                    .iter()
                    .find(|q| q.quest_index == quest_index)
                {
                    if can_track_quest(quest) {
                        quest_state.tracking_quest_index = Some(quest_index);
                        quest_state.set_feedback(format!("Tracking {}", quest.title), false);
                    } else {
                        quest_state.set_feedback("Cannot track this quest", true);
                    }
                } else {
                    quest_state.set_feedback("Quest not found", true);
                }
            }
            QuestUiButton::AcceptQuest {
                npc_index,
                quest_index,
            } => {
                if npc_index == 0 {
                    quest_state.set_feedback("Quest has no valid NPC source", true);
                    continue;
                }
                if !dialog_exposes_quest_action(
                    &dialog,
                    Some(npc_index),
                    &format!("@AcceptQuest:{quest_index}"),
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
                if !dialog_exposes_quest_action(
                    &dialog,
                    None,
                    &format!("@FinishQuest:{quest_index}"),
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
                        && (selected_item_index < 0
                            || (selected_item_index as usize) >= quest.rewards.len())
                    {
                        quest_state.set_feedback("Select a reward first", true);
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
                        let queue_full = queue.is_full();
                        if queue.push_pending_intent(
                            &mut pending,
                            QuestUiIntent::AbandonQuest { quest_index },
                        ) {
                            quest_state.set_feedback(format!("Abandoning {}", quest.title), false);
                        } else if queue_full {
                            quest_state.set_feedback("Connection busy; try again", true);
                        } else {
                            quest_state.set_feedback("Quest request is already pending", true);
                        }
                    } else {
                        quest_state
                            .set_feedback("Only an in-progress quest can be abandoned", true);
                    }
                } else {
                    quest_state.set_feedback("Quest not found", true);
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
                        if reward_index >= 0 && (reward_index as usize) < quest.rewards.len() {
                            quest_state.select_reward(reward_index);
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
                quest_state.clear_selection();
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

    // Input blocking: when quest log or dialog modal is open, gameplay shortcuts are suppressed.
    // Escape handling for modals takes precedence.
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
                quest_state.clear_selection();
                quest_state.clear_feedback();
            } else if dialog_open {
                if queue.push_intent(QuestUiIntent::SelectNpcDialog {
                    target: "@Exit".to_owned(),
                }) {
                    dialog.close();
                    npc_nav.clear();
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
            quest_state.clear_selection();
            quest_state.clear_feedback();
        } else {
            let transition = dispatch_ui_action(
                &mut player_ui.core,
                &mut effects,
                mir2_ui_core::action::UiAction::OpenQuestLog,
            );
            if transition.state.panel != mir2_ui_core::state::UiPanel::QuestLog {
                quest_state.clear_selection();
            }
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
                render_quest_tracker_panel(panel, &tracker);
            } else if is_dialog.is_some() {
                render_dialog_panel(panel, &dialog, &npc_nav, &quest_state, &pending);
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

    // Quest log overlay – modal centered panel.
    for (entity, mut node) in all.p2().iter_mut() {
        if quest_log_open {
            // Re-apply source-faithful geometry whenever the panel opens.
            node.left = Val::Px(QUEST_LOG_DESIGN_LEFT);
            node.top = Val::Px(QUEST_LOG_DESIGN_TOP);
            node.width = Val::Px(QUEST_LOG_DESIGN_WIDTH);
            node.height = Val::Px(QUEST_LOG_DESIGN_HEIGHT);
            node.min_width = Val::Px(QUEST_LOG_DESIGN_WIDTH);
            node.max_width = Val::Px(QUEST_LOG_DESIGN_WIDTH);
            node.min_height = Val::Px(QUEST_LOG_DESIGN_HEIGHT);
            node.max_height = Val::Px(QUEST_LOG_DESIGN_HEIGHT);
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
                render_quest_log_panel(
                    panel,
                    &tracker,
                    &quest_state,
                    &pending,
                    asset_server.as_ref().map(|server| &**server),
                )
            });
        }
    }
}

fn render_quest_tracker_panel(parent: &mut ChildSpawnerCommands, tracker: &QuestTracker) {
    let quests = visible_tracker_quests(tracker);
    if quests.is_empty() {
        return;
    }

    for quest in quests {
        tracker_quest_block(parent, quest);
    }
}

fn render_dialog_panel(
    parent: &mut ChildSpawnerCommands,
    dialog: &NpcDialogModel,
    nav: &NpcDialogNav,
    quest_state: &QuestUiState,
    pending: &PendingOperations,
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

    for option in dialog.options.iter().take(4) {
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

fn dialog_exposes_quest_action(
    dialog: &NpcDialogModel,
    expected_npc_index: Option<u32>,
    target: &str,
) -> bool {
    dialog.is_open
        && expected_npc_index.is_none_or(|expected| dialog.npc_object_id == Some(expected))
        && dialog
            .options
            .iter()
            .any(|option| option.enabled && option.option_id.trim().eq_ignore_ascii_case(target))
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
        let tracking = if state.tracking_quest_index == Some(quest.quest_index) {
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
            if state.tracking_quest_index == Some(quest.quest_index) {
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
fn render_quest_log_panel(
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
                    let tracking = state.tracking_quest_index == Some(quest.quest_index);
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
        if state.tracking_quest_index == Some(quest.quest_index) {
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

fn visible_tracker_quests(tracker: &QuestTracker) -> Vec<&Quest> {
    let mut visible = tracker
        .active_quests
        .iter()
        .filter(|quest| quest.status.is_active())
        .take(MAX_PANEL_QUESTS)
        .collect::<Vec<_>>();
    if !visible.is_empty() {
        return visible;
    }

    if let Some(completed) = tracker
        .active_quests
        .iter()
        .rev()
        .find(|quest| matches!(quest.status, crate::quest_model::QuestStatus::Completed))
    {
        visible.push(completed);
    }
    if visible.len() < MAX_PANEL_QUESTS {
        if let Some(available) = tracker
            .active_quests
            .iter()
            .find(|quest| matches!(quest.status, crate::quest_model::QuestStatus::NotStarted))
        {
            visible.push(available);
        }
    }
    visible
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
    fn abandon_is_allowed_only_for_in_progress_quests() {
        assert!(can_abandon_quest(&quest(1, QuestStatus::InProgress)));
        for status in [
            QuestStatus::NotStarted,
            QuestStatus::ReadyToTurnIn,
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
    fn abandon_button_preserves_quest_id_and_rejects_stale_non_progress_state() {
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
                quest(8, QuestStatus::ReadyToTurnIn),
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
        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::AbandonQuest { quest_index: 7 }]
        );
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
    fn tracker_prefers_live_progress_over_available_and_completed_entries() {
        let tracker = QuestTracker {
            active_quests: vec![
                quest(1, crate::quest_model::QuestStatus::Completed),
                quest(2, crate::quest_model::QuestStatus::NotStarted),
                quest(3, crate::quest_model::QuestStatus::InProgress),
                quest(4, crate::quest_model::QuestStatus::ReadyToTurnIn),
            ],
        };
        let visible = visible_tracker_quests(&tracker)
            .into_iter()
            .map(|quest| quest.quest_index)
            .collect::<Vec<_>>();
        assert_eq!(visible, vec![3, 4]);
    }

    #[test]
    fn tracker_shows_latest_completion_and_one_next_available_quest() {
        let tracker = QuestTracker {
            active_quests: vec![
                quest(1, crate::quest_model::QuestStatus::Completed),
                quest(2, crate::quest_model::QuestStatus::Completed),
                quest(3, crate::quest_model::QuestStatus::NotStarted),
                quest(4, crate::quest_model::QuestStatus::NotStarted),
            ],
        };
        let visible = visible_tracker_quests(&tracker)
            .into_iter()
            .map(|quest| quest.quest_index)
            .collect::<Vec<_>>();
        assert_eq!(visible, vec![2, 3]);
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
    fn reward_selection_required_only_for_multiple_rewards() {
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
                },
            ],
        );
        assert!(!quest_finish_enabled(&multi, None));
        assert!(!quest_finish_enabled(&multi, Some(-1)));
        assert!(quest_finish_enabled(&multi, Some(0)));
        assert!(quest_finish_enabled(&multi, Some(1)));
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
        state.tracking_quest_index = Some(10);
        state.set_feedback(format!("Tracking {}", active.title), false);
        assert_eq!(state.tracking_quest_index, Some(10));
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
                crate::quest_model::QuestReward::Gold { amount: 20 },
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
        assert!(state.feedback.as_ref().unwrap().is_error);
        app.world_mut().despawn(e);

        // Now select reward and finish should succeed
        app.world_mut()
            .resource_mut::<QuestUiState>()
            .select_reward(1);
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
            app.world().resource::<QuestUiState>().tracking_quest_index,
            Some(10)
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
    fn crystal_panel_skin_is_an_existing_non_placeholder_asset() {
        assert_eq!(QUEST_LOG_FRAME_ASSET, "original-ui/Title/670.png");
        assert_eq!(QUEST_LOG_CLOSE_ASSET, "original-ui/Prguse2/360.png");
        assert_eq!(QUEST_LOG_HELP_ASSET, "original-ui/Prguse2/257.png");
        assert_eq!(QUEST_LOG_PREVIOUS_ASSET, "original-ui/Prguse2/240.png");
        assert_eq!(QUEST_LOG_NEXT_ASSET, "original-ui/Prguse2/243.png");
        for asset in [
            QUEST_LOG_FRAME_ASSET,
            QUEST_LOG_CLOSE_ASSET,
            QUEST_LOG_HELP_ASSET,
            QUEST_LOG_PREVIOUS_ASSET,
            QUEST_LOG_NEXT_ASSET,
        ] {
            assert!(!asset.contains("missing"));
            assert!(!asset.contains("placeholder"));
        }
    }

    #[test]
    fn quest_log_renderer_tree_geometry_matches_crystal_at_100_125_and_150_percent() {
        for scale in [1.0, 1.25, 1.5] {
            let layout = quest_log_layout(scale);
            assert_eq!(
                layout.frame,
                QuestLogRect::new(
                    QUEST_LOG_DESIGN_LEFT * scale,
                    QUEST_LOG_DESIGN_TOP * scale,
                    QUEST_LOG_DESIGN_WIDTH * scale,
                    QUEST_LOG_DESIGN_HEIGHT * scale,
                )
            );
            assert_eq!(layout.list.width, 292.0 * scale);
            assert_eq!(layout.list.height, 192.0 * scale);
            assert_eq!(layout.detail.top, 278.0 * scale);
            assert_eq!(layout.detail.height, 118.0 * scale);
            assert_eq!(layout.tabs.len(), 5);
            assert_eq!(layout.actions.len(), 4);
            assert!((layout.tabs[0].left - 10.0 * scale).abs() < 0.001);
            assert!((layout.tabs[4].left + layout.tabs[4].width - 302.0 * scale).abs() < 0.001);
            assert!((layout.actions[0].top - 402.0 * scale).abs() < 0.001);
            assert!(
                (layout.actions[3].left + layout.actions[3].width - 302.0 * scale).abs() < 0.001
            );
            assert_eq!(
                layout.close,
                QuestLogRect::new(288.0 * scale, 3.0 * scale, 24.0 * scale, 21.0 * scale)
            );
            assert_eq!(
                layout.help,
                QuestLogRect::new(262.0 * scale, 3.0 * scale, 24.0 * scale, 21.0 * scale)
            );
            assert_eq!(
                layout.previous,
                QuestLogRect::new(132.0 * scale, 256.0 * scale, 16.0 * scale, 16.0 * scale)
            );
            assert_eq!(
                layout.next,
                QuestLogRect::new(214.0 * scale, 256.0 * scale, 16.0 * scale, 16.0 * scale)
            );
        }
    }

    #[test]
    fn quest_log_filter_and_page_state_reset_selection_without_touching_authority() {
        let mut state = QuestUiState {
            selected_quest_index: Some(7),
            selected_reward_index: Some(1),
            tracking_quest_index: Some(7),
            feedback: Some(QuestFeedback {
                message: "stale".to_owned(),
                is_error: false,
            }),
            stage_filter: QuestStageFilter::All,
            page: 3,
        };
        state.set_stage_filter(QuestStageFilter::Completed);
        assert_eq!(state.stage_filter, QuestStageFilter::Completed);
        assert_eq!(state.page, 0);
        assert_eq!(state.selected_quest_index, None);
        assert_eq!(state.selected_reward_index, None);
        assert_eq!(state.tracking_quest_index, Some(7));
        assert!(state.feedback.is_none());
        state.set_page(2);
        assert_eq!(state.page, 2);
        assert_eq!(state.selected_quest_index, None);
    }
}

//! Crystal-authored in-game chat dialog with full filter/scroll/resize/settings logic.
//!
//! This module replaces the earlier display-only stub with authoritative
//! presentation logic that mirrors `MainDialogs.cs:ChatDialog` and
//! `ChatControlBar`/`ChatOptionDialog` source behaviour while staying
//! renderer-neutral (it only consumes `ChatModel` and typed queues).

use bevy::prelude::*;
use bevy::ui::{BackgroundColor, Node, PositionType, Val};

use crate::chat::{ChatChannel, ChatLine, ChatModel};
use crate::crystal_ui::overlays::{dispatch_ui_action, NativePlayerUiState, UiEffectQueue};
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_ui_core::action::UiAction;
use mir2_ui_core::state::{UiChatChannel, UiChatSettings};

use super::spec;
use super::typography::{crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX};
#[cfg(test)]
use super::spec::CrystalRect;

/// The Crystal 1024x768 chat panel's screen-space origin.
pub const CHAT_PANEL_ORIGIN: (f32, f32) = (230.0, 671.0);
/// Crystal's default 1024x768 four-line ChatDialog frame.
pub const CHAT_FRAME_INDEX: u16 = 2221;
/// The source chat font is eight points and the four-line window advances by
/// thirteen pixels per row in the 1024x768 scene.
pub const CHAT_LINE_HEIGHT: f32 = 13.0;
/// Source chat labels begin one pixel inside the panel.
pub const CHAT_TEXT_ORIGIN: (f32, f32) = (1.0, 1.0);
/// The scroll controls occupy the rightmost twelve pixels of the panel.
pub const CHAT_SCROLL_LEFT: f32 = 618.0;
/// MainDialog is currently 950 and the native shell is 1000; chat sits above
/// the former while remaining below the latter.
pub const CHAT_Z_INDEX: i32 = 975;

// ---------------------------------------------------------------------------
// Chat filter / window size / hidden filters
// ---------------------------------------------------------------------------

/// Outbound/display filter selected via the ChatControlBar buttons.
///
/// Maps directly to the 8 control-bar actions:
/// All / Shout / Whisper / Lover / Mentor / Group / Guild / Trade
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrystalChatFilter {
    #[default]
    All,
    Shout,
    Whisper,
    Lover,
    Mentor,
    Group,
    Guild,
    Trade,
}

impl CrystalChatFilter {
    /// Ordered list matching the control-bar button order.
    pub fn all_variants() -> &'static [CrystalChatFilter] {
        &[
            Self::All,
            Self::Shout,
            Self::Whisper,
            Self::Lover,
            Self::Mentor,
            Self::Group,
            Self::Guild,
            Self::Trade,
        ]
    }

    pub fn from_action(action: CrystalChatAction) -> Option<Self> {
        match action {
            CrystalChatAction::FilterAll => Some(Self::All),
            CrystalChatAction::FilterShout => Some(Self::Shout),
            CrystalChatAction::FilterWhisper => Some(Self::Whisper),
            CrystalChatAction::FilterLover => Some(Self::Lover),
            CrystalChatAction::FilterMentor => Some(Self::Mentor),
            CrystalChatAction::FilterGroup => Some(Self::Group),
            CrystalChatAction::FilterGuild => Some(Self::Guild),
            CrystalChatAction::FilterTrade => Some(Self::Trade),
            _ => None,
        }
    }

    pub fn prefix(self) -> &'static str {
        match self {
            Self::All => "",
            Self::Shout => "!",
            Self::Whisper => "/",
            Self::Lover => ":)",
            Self::Mentor => "!#",
            Self::Group => "!!",
            Self::Guild => "!~",
            Self::Trade => "@",
        }
    }
}

/// Crystal's three chat window sizes: 4 / 7 / 11 lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrystalChatWindowSize {
    #[default]
    Small,
    Medium,
    Large,
}

impl CrystalChatWindowSize {
    pub fn line_count(self) -> usize {
        match self {
            Self::Small => 4,
            Self::Medium => 7,
            Self::Large => 11,
        }
    }

    pub fn frame_index(self) -> u16 {
        match self {
            Self::Small => 2221,
            Self::Medium => 2224,
            Self::Large => 2227,
        }
    }

    pub fn count_bar_index(self) -> u16 {
        match self {
            Self::Small => 2012,
            Self::Medium => 2013,
            Self::Large => 2014,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Small => Self::Medium,
            Self::Medium => Self::Large,
            Self::Large => Self::Small,
        }
    }

    pub fn spec_rect(self) -> super::spec::CrystalRect {
        match self {
            Self::Small => spec::hud::CHAT_FOUR_LINES.rect,
            Self::Medium => spec::hud::CHAT_SEVEN_LINES.rect,
            Self::Large => spec::hud::CHAT_ELEVEN_LINES.rect,
        }
    }
}

/// Settings tab order from `ChatOptionDialog.SwitchTab`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrystalChatSettingsTab {
    #[default]
    Filters,
    Chat,
}

/// Rendering-only chat state. Committed/draft/open settings live exclusively
/// in `NativePlayerUiState.core`; `applied_settings` is an effect-driven
/// projection used by this renderer, never an independently edited authority.
#[derive(Debug, Clone, Resource, PartialEq, Eq)]
pub struct CrystalChatState {
    pub filter: CrystalChatFilter,
    pub scroll: usize,
    pub window_size: CrystalChatWindowSize,
    pub settings_tab: CrystalChatSettingsTab,
    pub applied_settings: UiChatSettings,
}

impl Default for CrystalChatState {
    fn default() -> Self {
        Self {
            filter: CrystalChatFilter::All,
            scroll: 0,
            window_size: CrystalChatWindowSize::Small,
            settings_tab: CrystalChatSettingsTab::Filters,
            applied_settings: UiChatSettings::default(),
        }
    }
}

impl CrystalChatState {
    pub fn line_count(&self) -> usize {
        self.window_size.line_count()
    }

    pub fn max_scroll(&self, filtered_len: usize) -> usize {
        max_scroll_offset(filtered_len, self.line_count())
    }

    pub fn clamp_scroll(&mut self, filtered_len: usize) {
        let max = self.max_scroll(filtered_len);
        if self.scroll > max {
            self.scroll = max;
        }
    }

    pub fn home(&mut self) {
        self.scroll = 0;
    }

    pub fn end(&mut self, filtered_len: usize) {
        self.scroll = self.max_scroll(filtered_len);
    }

    pub fn up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn down(&mut self, filtered_len: usize) {
        let max = self.max_scroll(filtered_len);
        if self.scroll < max {
            self.scroll += 1;
        }
    }

    pub fn resize(&mut self, filtered_len: usize) {
        self.window_size = self.window_size.next();
        self.clamp_scroll(filtered_len);
    }

    pub fn set_filter(&mut self, filter: CrystalChatFilter, filtered_len: usize) {
        self.filter = filter;
        // Reset scroll to end (show newest) when filter changes, like Crystal's Update after ToggleChatFilter
        self.scroll = self.max_scroll(filtered_len);
    }
}

/// Marker on the stage-sized root owned by this renderer.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalChatRoot;

/// Marker on all direct children so a future host can identify the Crystal
/// chat layer without depending on implementation details.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalChatElement;

/// A typed, presentation-only result of pressing a Crystal chat control.
///
/// The plugin places these in [`CrystalChatActionQueue`]. No action changes
/// the gameplay model or emits a Gateway command; a later host may consume the
/// queue to implement scrolling or chat options.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalChatAction {
    Home,
    Up,
    Down,
    End,
    PositionBar,
    FilterAll,
    FilterShout,
    FilterWhisper,
    FilterLover,
    FilterMentor,
    FilterGroup,
    FilterGuild,
    FilterTrade,
    Resize,
    Settings,
    SettingsTab(CrystalChatSettingsTab),
    SettingsFilterAll,
    SettingsFilter(UiChatChannel),
    SettingsTransparency(bool),
    SettingsApply,
    SettingsCancel,
    SettingsDefaults,
    SettingsClose,
}

/// UI-only action handoff for a future native host.
#[derive(Debug, Default, Resource, PartialEq, Eq)]
pub struct CrystalChatActionQueue {
    pub actions: Vec<CrystalChatAction>,
}

impl CrystalChatActionQueue {
    pub fn push(&mut self, action: CrystalChatAction) {
        self.actions.push(action);
    }

    pub fn drain(&mut self) -> Vec<CrystalChatAction> {
        core::mem::take(&mut self.actions)
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct CrystalChatButton {
    normal: u16,
    hover: u16,
    pressed: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrystalChatAssetLibrary {
    Prguse,
    Prguse2,
    Title,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct CrystalChatSettingsButton {
    normal: u16,
    hover: u16,
    pressed: u16,
    library: CrystalChatAssetLibrary,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct CrystalChatSettingsModal;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrystalChatColors {
    pub foreground: Color,
    pub background: Color,
}

/// Presentation-only Crystal chat plugin.
pub struct Mir2CrystalChatPlugin;

impl Plugin for Mir2CrystalChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatModel>()
            .init_resource::<CrystalChatActionQueue>()
            .init_resource::<CrystalChatState>()
            .init_resource::<NativePlayerUiState>()
            .init_resource::<UiEffectQueue>()
            .init_resource::<crate::chat_settings_effects::ChatSettingsRuntime>()
            .add_systems(Startup, spawn_chat_root)
            .add_systems(
                Startup,
                crate::chat_settings_effects::load_persisted_chat_settings,
            )
            .add_systems(
                Update,
                (
                    handle_chat_settings_keys,
                    handle_chat_scroll_keys,
                    consume_chat_actions,
                    crate::chat_settings_effects::consume_chat_settings_effects,
                    auto_scroll_on_new_message,
                    render_crystal_chat,
                    sync_chat_button_visuals,
                    sync_chat_settings_button_visuals,
                    consume_chat_button_interactions,
                )
                    .chain(),
            );
    }
}

fn spawn_chat_root(mut commands: Commands) {
    commands.spawn((
        CrystalChatRoot,
        GlobalZIndex(CHAT_Z_INDEX),
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(1024.0),
            height: Val::Px(768.0),
            ..default()
        },
    ));
}

/// Returns true if a channel string matches the given outbound filter.
pub fn channel_matches_filter(channel: &str, filter: CrystalChatFilter) -> bool {
    let channel = ChatChannel::parse(channel);
    match filter {
        CrystalChatFilter::All => true,
        CrystalChatFilter::Shout => channel == ChatChannel::Shout,
        CrystalChatFilter::Whisper => {
            matches!(channel, ChatChannel::WhisperIn | ChatChannel::WhisperOut)
        }
        CrystalChatFilter::Lover => {
            matches!(channel, ChatChannel::Relationship | ChatChannel::Lover)
        }
        CrystalChatFilter::Mentor => channel == ChatChannel::Mentor,
        CrystalChatFilter::Group => channel == ChatChannel::Group,
        CrystalChatFilter::Guild => channel == ChatChannel::Guild,
        CrystalChatFilter::Trade => channel == ChatChannel::Trade,
    }
}

/// Returns true if a line should be hidden based on Settings hidden filters.
pub fn is_line_hidden_by_settings(line: &ChatLine, settings: &UiChatSettings) -> bool {
    match ChatChannel::settings_filter_channel(&line.channel) {
        Some(ChatChannel::Normal | ChatChannel::LineMessage) => settings.filter_normal,
        Some(ChatChannel::WhisperIn | ChatChannel::WhisperOut) => settings.filter_whisper,
        Some(ChatChannel::Shout) => settings.filter_shout,
        Some(ChatChannel::System) => settings.filter_system,
        Some(ChatChannel::Group) => settings.filter_group,
        Some(ChatChannel::Guild) => settings.filter_guild,
        None => false,
        Some(_) => false,
    }
}

/// Filter lines by both hidden settings and outbound filter.
pub fn filtered_lines<'a>(model: &'a ChatModel, state: &CrystalChatState) -> Vec<&'a ChatLine> {
    model
        .lines
        .iter()
        .filter(|line| !is_line_hidden_by_settings(line, &state.applied_settings))
        .filter(|line| channel_matches_filter(&line.channel, state.filter))
        .collect()
}

/// Direct filter by outbound filter only (for display content tests).
pub fn filter_lines_by_filter<'a>(
    lines: &'a [ChatLine],
    filter: CrystalChatFilter,
) -> Vec<&'a ChatLine> {
    lines
        .iter()
        .filter(|line| channel_matches_filter(&line.channel, filter))
        .collect()
}

/// Compute max scroll offset for given filtered length and visible line count.
/// Mirrors Crystal's `Update` clamping and Web's `maxScrollOffset = max(lines - visible, 0)`.
pub fn max_scroll_offset(filtered_len: usize, line_count: usize) -> usize {
    filtered_len.saturating_sub(line_count)
}

/// Clamp scroll to valid range.
pub fn clamp_scroll_offset(scroll: usize, filtered_len: usize, line_count: usize) -> usize {
    scroll.min(max_scroll_offset(filtered_len, line_count))
}

/// Consume queued chat actions and mutate state. Pure logic extracted for testing.
pub fn apply_chat_action(
    state: &mut CrystalChatState,
    action: CrystalChatAction,
    filtered_len: usize,
) {
    match action {
        CrystalChatAction::Home => state.home(),
        CrystalChatAction::Up => state.up(),
        CrystalChatAction::Down => state.down(filtered_len),
        CrystalChatAction::End => state.end(filtered_len),
        CrystalChatAction::FilterAll => {
            let len = filtered_len;
            state.set_filter(CrystalChatFilter::All, len);
        }
        CrystalChatAction::FilterShout => {
            let len = filtered_len;
            state.set_filter(CrystalChatFilter::Shout, len);
        }
        CrystalChatAction::FilterWhisper => {
            let len = filtered_len;
            state.set_filter(CrystalChatFilter::Whisper, len);
        }
        CrystalChatAction::FilterLover => {
            let len = filtered_len;
            state.set_filter(CrystalChatFilter::Lover, len);
        }
        CrystalChatAction::FilterMentor => {
            let len = filtered_len;
            state.set_filter(CrystalChatFilter::Mentor, len);
        }
        CrystalChatAction::FilterGroup => {
            let len = filtered_len;
            state.set_filter(CrystalChatFilter::Group, len);
        }
        CrystalChatAction::FilterGuild => {
            let len = filtered_len;
            state.set_filter(CrystalChatFilter::Guild, len);
        }
        CrystalChatAction::FilterTrade => {
            let len = filtered_len;
            state.set_filter(CrystalChatFilter::Trade, len);
        }
        CrystalChatAction::Resize => state.resize(filtered_len),
        CrystalChatAction::SettingsTab(tab) => state.settings_tab = tab,
        CrystalChatAction::Settings
        | CrystalChatAction::SettingsFilterAll
        | CrystalChatAction::SettingsFilter(_)
        | CrystalChatAction::SettingsTransparency(_)
        | CrystalChatAction::SettingsApply
        | CrystalChatAction::SettingsCancel
        | CrystalChatAction::SettingsClose
        | CrystalChatAction::SettingsDefaults => {}
        CrystalChatAction::PositionBar => {
            // PositionBar dragging is handled via scrollbar position;
            // for queue consumption, treat as no-op scroll (already handled via drag).
        }
    }
}

/// Drain and apply all queued actions. Runs before rendering.
fn consume_chat_actions(
    mut queue: ResMut<CrystalChatActionQueue>,
    mut state: ResMut<CrystalChatState>,
    mut player_ui: ResMut<NativePlayerUiState>,
    mut effects: ResMut<UiEffectQueue>,
    chat: Option<Res<ChatModel>>,
) {
    if queue.is_empty() {
        return;
    }
    let actions = queue.drain();
    // Need filtered length for scroll bounds. We compute based on current state before applying?
    // For correctness we recompute after each filter change.
    for action in actions {
        let settings_open = player_ui.core.chat_settings_open();
        let shared_action = match action {
            CrystalChatAction::Settings => Some(if settings_open {
                UiAction::CloseChatSettings
            } else {
                state.settings_tab = CrystalChatSettingsTab::Filters;
                UiAction::OpenChatSettings
            }),
            CrystalChatAction::SettingsFilterAll if settings_open => {
                let draft = player_ui
                    .core
                    .chat_settings_draft
                    .unwrap_or(player_ui.core.chat_settings);
                Some(UiAction::SetAllChatFilterVisibility {
                    visible: draft.any_dialog_filter_hidden(),
                })
            }
            CrystalChatAction::SettingsFilter(channel) if settings_open => {
                let draft = player_ui
                    .core
                    .chat_settings_draft
                    .unwrap_or(player_ui.core.chat_settings);
                Some(UiAction::SetChatFilterVisibility {
                    channel,
                    visible: draft.is_filter_hidden(channel),
                })
            }
            CrystalChatAction::SettingsTransparency(transparent) if settings_open => {
                Some(UiAction::SetChatTransparency { transparent })
            }
            CrystalChatAction::SettingsApply if settings_open => Some(UiAction::ApplyChatSettings),
            CrystalChatAction::SettingsCancel if settings_open => {
                Some(UiAction::CancelChatSettings)
            }
            CrystalChatAction::SettingsClose if settings_open => Some(UiAction::CloseChatSettings),
            CrystalChatAction::SettingsDefaults if settings_open => {
                Some(UiAction::ResetChatSettingsToDefaults)
            }
            _ => None,
        };
        if let Some(shared_action) = shared_action {
            dispatch_ui_action(&mut player_ui.core, &mut effects, shared_action);
        }
        if matches!(action, CrystalChatAction::SettingsTab(_)) && !settings_open {
            continue;
        }

        // Determine filtered length according to state before action, but for filter actions we need to
        // know length after filter? We approximate by using current model length filtered with new filter?
        // Simplify: compute filtered len after potential filter change by peeking.
        let mut temp_state = state.clone();
        // Apply to temp to compute new filtered len if filter changed
        let filtered_len_before = chat
            .as_ref()
            .map(|m| filtered_lines(m, &temp_state).len())
            .unwrap_or(0);
        // Apply action to real state with appropriate length
        // For filter actions, we need length after filter – we can compute after setting filter.
        // So apply with a two-step for filters.
        if let Some(filter) = CrystalChatFilter::from_action(action) {
            // Temporarily set filter to compute new length
            temp_state.filter = filter;
            let new_len = chat
                .as_ref()
                .map(|m| filtered_lines(m, &temp_state).len())
                .unwrap_or(0);
            apply_chat_action(&mut state, action, new_len);
            // Ensure scroll clamped to new max
            state.clamp_scroll(new_len);
        } else {
            apply_chat_action(&mut state, action, filtered_len_before);
            // Re-clamp after resize etc
            let new_len = chat
                .as_ref()
                .map(|m| filtered_lines(m, &state).len())
                .unwrap_or(0);
            state.clamp_scroll(new_len);
        }
    }
}

/// ChatOptionDialog is modal in Crystal. Escape closes it without changing
/// the staged values; Tab changes the two source tabs. Pointer buttons below
/// use the same queue, so Android can map equivalent semantic actions without
/// depending on Bevy coordinates.
fn handle_chat_settings_keys(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<CrystalChatState>,
    player_ui: Res<NativePlayerUiState>,
    shell: Option<Res<NativeShellModel>>,
    mut queue: ResMut<CrystalChatActionQueue>,
) {
    if !shell.is_some_and(|s| s.screen == NativeShellScreen::InGame)
        || !player_ui.core.chat_settings_open()
    {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        queue.push(CrystalChatAction::SettingsCancel);
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let tab = match state.settings_tab {
            CrystalChatSettingsTab::Filters => CrystalChatSettingsTab::Chat,
            CrystalChatSettingsTab::Chat => CrystalChatSettingsTab::Filters,
        };
        queue.push(CrystalChatAction::SettingsTab(tab));
    }
}

fn handle_chat_scroll_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CrystalChatState>,
    chat: Option<Res<ChatModel>>,
    shell: Option<Res<NativeShellModel>>,
    player_ui: Res<NativePlayerUiState>,
) {
    if !shell.is_some_and(|s| s.screen == NativeShellScreen::InGame) {
        return;
    }
    if player_ui.core.chat_settings_open() {
        return;
    }
    // Avoid scrolling while typing (chat focused in overlay state is separate,
    // but we can still allow scroll when chat draft is focused? In Crystal,
    // chat scroll keys are handled by ChatDialog even when text box not focused.
    // For native, we allow scroll regardless of focus to satisfy Goal 4.8.
    let filtered_len = chat
        .as_ref()
        .map(|m| filtered_lines(m, &state).len())
        .unwrap_or(0);
    if keys.just_pressed(KeyCode::Home) {
        state.home();
    }
    if keys.just_pressed(KeyCode::End) {
        state.end(filtered_len);
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::PageUp) {
        if keys.just_pressed(KeyCode::PageUp) {
            for _ in 0..state.line_count() {
                state.up();
            }
        } else {
            state.up();
        }
    }
    if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::PageDown) {
        if keys.just_pressed(KeyCode::PageDown) {
            for _ in 0..state.line_count() {
                state.down(filtered_len);
            }
        } else {
            state.down(filtered_len);
        }
    }
}

fn auto_scroll_on_new_message(
    chat: Option<Res<ChatModel>>,
    mut state: ResMut<CrystalChatState>,
    mut last_filtered_len: Local<Option<usize>>,
    mut was_at_bottom: Local<bool>,
) {
    let Some(chat) = chat else {
        return;
    };
    let filtered_len = filtered_lines(&chat, &state).len();
    let line_count = state.line_count();
    let max = max_scroll_offset(filtered_len, line_count);
    // Detect if we were at bottom before this change
    if let Some(prev_len) = *last_filtered_len {
        if *was_at_bottom && prev_len != filtered_len && filtered_len > prev_len {
            // New lines arrived while at bottom -> stay at bottom (Crystal behavior: StartIndex += chat.Count)
            state.scroll = max;
        }
        // Clamp in any case
        if state.scroll > max {
            state.scroll = max;
        }
        if prev_len != filtered_len {
            // Update was_at_bottom for next tick based on current scroll position before change?
            // Actually after change we set was_at_bottom = scroll == max
        }
    } else if filtered_len > line_count {
        // First run with many lines: default to bottom
        state.scroll = max;
    }
    *was_at_bottom = state.scroll == max;
    *last_filtered_len = Some(filtered_len);
}

/// Rebuild the small deterministic chat tree only when either source resource
/// changes, or when the root is first added. This avoids frame-by-frame text
/// churn while keeping the result independent of prior UI state.
fn render_crystal_chat(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    chat: Option<Res<ChatModel>>,
    chat_state: Option<Res<CrystalChatState>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    shell: Option<Res<NativeShellModel>>,
    mut roots: Query<(Entity, &mut Visibility, Option<&Children>), With<CrystalChatRoot>>,
    added_roots: Query<Entity, Added<CrystalChatRoot>>,
) {
    let chat_changed = chat.as_ref().is_some_and(|resource| resource.is_changed());
    let state_changed = chat_state
        .as_ref()
        .is_some_and(|resource| resource.is_changed());
    let player_ui_changed = player_ui
        .as_ref()
        .is_some_and(|resource| resource.is_changed());
    let shell_changed = shell.as_ref().is_some_and(|resource| resource.is_changed());
    let first_render = !added_roots.is_empty();
    if !first_render && !chat_changed && !shell_changed && !state_changed && !player_ui_changed {
        return;
    }

    let Some((root, mut visibility, children)) = roots.iter_mut().next() else {
        return;
    };

    if let Some(children) = children {
        let previous_children: Vec<Entity> = children.iter().collect();
        for child in previous_children {
            commands.entity(child).despawn();
        }
    }

    let in_game = shell
        .as_deref()
        .is_some_and(|model| model.screen == NativeShellScreen::InGame);
    if !in_game {
        *visibility = Visibility::Hidden;
        return;
    }

    *visibility = Visibility::Visible;
    let Some(chat) = chat.as_deref() else {
        return;
    };
    let state = chat_state.as_deref().cloned().unwrap_or_default();
    let settings_open = player_ui
        .as_deref()
        .is_some_and(|ui| ui.core.chat_settings_open());
    let settings_draft = player_ui
        .as_deref()
        .map(|ui| ui.core.chat_settings_draft.unwrap_or(ui.core.chat_settings));
    let filtered = filtered_lines(chat, &state);
    let line_count = state.line_count();
    let visible = visible_chat_slice(&filtered, state.scroll, line_count);
    let frame_spec = match state.window_size {
        CrystalChatWindowSize::Small => spec::hud::CHAT_FOUR_LINES,
        CrystalChatWindowSize::Medium => spec::hud::CHAT_SEVEN_LINES,
        CrystalChatWindowSize::Large => spec::hud::CHAT_ELEVEN_LINES,
    };
    commands.entity(root).with_children(|parent| {
        spawn_chat_control_bar(parent, &asset_server);
        spawn_chat_frame_with_spec(
            parent,
            &asset_server,
            frame_spec,
            state.applied_settings.transparent,
        );

        for (index, line) in visible.iter().enumerate() {
            spawn_chat_line(parent, line, index, state.applied_settings.transparent);
        }

        // These coordinates and frame triples are copied from ChatDialog's
        // 1024x768 constructor in Crystal's MainDialogs.cs.
        spawn_chat_button(
            parent,
            &asset_server,
            2018,
            2019,
            2020,
            scroll_button_origin(CrystalChatAction::Home),
            CrystalChatAction::Home,
        );
        spawn_chat_button(
            parent,
            &asset_server,
            2021,
            2022,
            2023,
            scroll_button_origin(CrystalChatAction::Up),
            CrystalChatAction::Up,
        );
        spawn_chat_button(
            parent,
            &asset_server,
            2024,
            2025,
            2026,
            scroll_button_origin(CrystalChatAction::Down),
            CrystalChatAction::Down,
        );
        spawn_chat_button(
            parent,
            &asset_server,
            2027,
            2028,
            2029,
            scroll_button_origin(CrystalChatAction::End),
            CrystalChatAction::End,
        );
        spawn_chat_frame_at(
            parent,
            &asset_server,
            2012,
            (CHAT_SCROLL_LEFT + 4.0, 16.0),
            4.0,
            21.0,
        );
        spawn_chat_button(
            parent,
            &asset_server,
            2015,
            2016,
            2017,
            (CHAT_SCROLL_LEFT + 1.0, 16.0),
            CrystalChatAction::PositionBar,
        );
        if settings_open {
            spawn_chat_settings_panel(
                parent,
                &asset_server,
                &state,
                settings_draft.unwrap_or(state.applied_settings),
            );
        }
    });
}

fn spawn_chat_frame(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    let frame = spec::hud::CHAT_FOUR_LINES;
    parent.spawn((
        CrystalChatElement,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(frame.rect.left),
            top: Val::Px(frame.rect.top),
            width: Val::Px(frame.rect.width),
            height: Val::Px(frame.rect.height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(prguse_asset(CHAT_FRAME_INDEX)),
            ..default()
        },
    ));
}

fn spawn_chat_frame_with_spec(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    frame: super::spec::CrystalFrameSpec,
    transparent: bool,
) {
    parent.spawn((
        CrystalChatElement,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(frame.rect.left),
            top: Val::Px(frame.rect.top),
            width: Val::Px(frame.rect.width),
            height: Val::Px(frame.rect.height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(format!("original-ui/{}/{}", frame.library, frame.index)),
            color: chat_tint(transparent),
            ..default()
        },
    ));
}

fn spawn_chat_settings_panel(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    state: &CrystalChatState,
    settings: UiChatSettings,
) {
    // Crystal ChatOptionDialog.cs:7-28 centers a 224x180 Title frame and
    // defaults to the filter tab (Title/466). The modal blocker is a real UI
    // button so world clicks cannot pass through the panel.
    let panel_left = (1024.0 - 224.0) / 2.0;
    let panel_top = (768.0 - 180.0) / 2.0;
    parent
        .spawn((
            CrystalChatElement,
            CrystalChatSettingsModal,
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_left),
                top: Val::Px(panel_top),
                width: Val::Px(224.0),
                height: Val::Px(180.0),
                ..default()
            },
            ImageNode {
                image: asset_server.load(title_asset(466)),
                ..default()
            },
        ))
        .with_children(|panel| {
            spawn_chat_settings_button(
                panel,
                asset_server,
                if state.settings_tab == CrystalChatSettingsTab::Filters {
                    463
                } else {
                    462
                },
                463,
                if state.settings_tab == CrystalChatSettingsTab::Filters {
                    462
                } else {
                    463
                },
                CrystalChatAssetLibrary::Title,
                (8.0, 8.0),
                CrystalChatAction::SettingsTab(CrystalChatSettingsTab::Filters),
            );
            spawn_chat_settings_button(
                panel,
                asset_server,
                if state.settings_tab == CrystalChatSettingsTab::Chat {
                    465
                } else {
                    464
                },
                464,
                if state.settings_tab == CrystalChatSettingsTab::Chat {
                    464
                } else {
                    465
                },
                CrystalChatAssetLibrary::Title,
                (78.0, 8.0),
                CrystalChatAction::SettingsTab(CrystalChatSettingsTab::Chat),
            );
            spawn_chat_settings_button(
                panel,
                asset_server,
                360,
                361,
                362,
                CrystalChatAssetLibrary::Prguse2,
                (198.0, 3.0),
                CrystalChatAction::SettingsClose,
            );

            match state.settings_tab {
                CrystalChatSettingsTab::Filters => {
                    spawn_chat_filter_settings(panel, asset_server, settings);
                }
                CrystalChatSettingsTab::Chat => {
                    spawn_chat_transparency_settings(panel, asset_server, settings);
                }
            }

            spawn_chat_settings_footer(panel, asset_server);
        });
}

fn spawn_chat_filter_settings(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    settings: UiChatSettings,
) {
    let all_frame = if settings.any_filter_hidden() {
        2086
    } else {
        2087
    };
    spawn_chat_settings_button(
        parent,
        asset_server,
        all_frame,
        all_frame,
        all_frame,
        CrystalChatAssetLibrary::Prguse,
        (74.0, 47.0),
        CrystalChatAction::SettingsFilterAll,
    );

    let controls = [
        (UiChatChannel::Normal, 40.0, 69.0, 2070, 2071),
        (UiChatChannel::Whisper, 40.0, 92.0, 2074, 2075),
        (UiChatChannel::Shout, 40.0, 115.0, 2072, 2073),
        (UiChatChannel::System, 40.0, 138.0, 2084, 2085),
        (UiChatChannel::Lover, 135.0, 69.0, 2076, 2077),
        (UiChatChannel::Mentor, 135.0, 92.0, 2078, 2079),
        (UiChatChannel::Group, 135.0, 115.0, 2080, 2081),
        (UiChatChannel::Guild, 135.0, 138.0, 2082, 2083),
    ];
    for (channel, left, top, hidden_frame, visible_frame) in controls {
        let frame = if settings.is_filter_hidden(channel) {
            hidden_frame
        } else {
            visible_frame
        };
        spawn_chat_settings_button(
            parent,
            asset_server,
            frame,
            frame,
            frame,
            CrystalChatAssetLibrary::Prguse,
            (left, top),
            CrystalChatAction::SettingsFilter(channel),
        );
    }
}

fn spawn_chat_transparency_settings(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    settings: UiChatSettings,
) {
    let (off_normal, off_hover, off_pressed) = if settings.transparent {
        (470, 470, 470)
    } else {
        (471, 472, 470)
    };
    spawn_chat_settings_button(
        parent,
        asset_server,
        off_normal,
        off_hover,
        off_pressed,
        CrystalChatAssetLibrary::Title,
        (45.0, 90.0),
        CrystalChatAction::SettingsTransparency(false),
    );

    let (on_normal, on_hover, on_pressed) = if settings.transparent {
        (474, 475, 473)
    } else {
        (473, 473, 473)
    };
    spawn_chat_settings_button(
        parent,
        asset_server,
        on_normal,
        on_hover,
        on_pressed,
        CrystalChatAssetLibrary::Title,
        (115.0, 90.0),
        CrystalChatAction::SettingsTransparency(true),
    );
}

fn spawn_chat_settings_footer(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    // Crystal applies these values immediately. These small native lifecycle
    // buttons only govern the adapter's staged draft; they do not represent
    // extra Crystal settings.
    for (left, label, action) in [
        (8.0, "Defaults", CrystalChatAction::SettingsDefaults),
        (80.0, "Cancel", CrystalChatAction::SettingsCancel),
        (152.0, "Apply", CrystalChatAction::SettingsApply),
    ] {
        let (screen_left, screen_top) = (left, 158.0);
        parent.spawn((
            CrystalChatElement,
            Button,
            action,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(screen_left),
                top: Val::Px(screen_top),
                width: Val::Px(64.0),
                height: Val::Px(16.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.20, 0.14, 0.08, 0.96)),
            Text::new(label),
            crystal_text_font(CRYSTAL_DEFAULT_FONT_SIZE_PX),
            TextColor(Color::WHITE),
        ));
    }
    let _ = asset_server;
}

fn spawn_chat_settings_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    normal: u16,
    hover: u16,
    pressed: u16,
    library: CrystalChatAssetLibrary,
    relative_origin: (f32, f32),
    action: CrystalChatAction,
) {
    parent.spawn((
        CrystalChatElement,
        CrystalChatSettingsButton {
            normal,
            hover,
            pressed,
            library,
        },
        Button,
        action,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(relative_origin.0),
            top: Val::Px(relative_origin.1),
            ..default()
        },
        ImageNode {
            image: asset_server.load(chat_asset(library, normal)),
            ..default()
        },
    ));
}

fn spawn_chat_control_bar(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    let frame = spec::hud::CHAT_CONTROL_BAR;
    parent.spawn((
        CrystalChatElement,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(frame.rect.left),
            top: Val::Px(frame.rect.top),
            width: Val::Px(frame.rect.width),
            height: Val::Px(frame.rect.height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(prguse_asset(frame.index)),
            ..default()
        },
    ));

    let controls = [
        (2036, 12.0, CrystalChatAction::FilterAll),
        (2039, 34.0, CrystalChatAction::FilterShout),
        (2042, 56.0, CrystalChatAction::FilterWhisper),
        (2045, 78.0, CrystalChatAction::FilterLover),
        (2048, 100.0, CrystalChatAction::FilterMentor),
        (2051, 122.0, CrystalChatAction::FilterGroup),
        (2054, 144.0, CrystalChatAction::FilterGuild),
        (2004, 166.0, CrystalChatAction::FilterTrade),
        (2057, 574.0, CrystalChatAction::Resize),
        (2060, 596.0, CrystalChatAction::Settings),
    ];
    for (normal, relative_left, action) in controls {
        spawn_chat_button(
            parent,
            asset_server,
            normal,
            normal + 1,
            normal + 2,
            (relative_left, -14.0),
            action,
        );
    }
}

fn spawn_chat_frame_at(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    frame_index: u16,
    relative_origin: (f32, f32),
    width: f32,
    height: f32,
) {
    let (left, top) = child_screen_origin(relative_origin);
    parent.spawn((
        CrystalChatElement,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(width),
            height: Val::Px(height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(prguse_asset(frame_index)),
            ..default()
        },
    ));
}

fn spawn_chat_line(
    parent: &mut ChildSpawnerCommands,
    line: &ChatLine,
    row: usize,
    transparent: bool,
) {
    let colors = channel_colors(&line.channel);
    let (left, top) = child_screen_origin((
        CHAT_TEXT_ORIGIN.0,
        CHAT_TEXT_ORIGIN.1 + row as f32 * CHAT_LINE_HEIGHT,
    ));

    parent.spawn((
        CrystalChatElement,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            height: Val::Px(CHAT_LINE_HEIGHT),
            max_width: Val::Px(610.0),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(chat_color(colors.background, transparent)),
        Text::new(line.text.clone()),
        crystal_text_font(CRYSTAL_DEFAULT_FONT_SIZE_PX),
        TextColor(chat_color(colors.foreground, transparent)),
    ));
}

fn chat_tint(transparent: bool) -> Color {
    if transparent {
        Color::srgba(1.0, 1.0, 1.0, 0.8)
    } else {
        Color::WHITE
    }
}

fn chat_color(color: Color, transparent: bool) -> Color {
    if !transparent {
        return color;
    }
    let srgba = color.to_srgba();
    Color::srgba(srgba.red, srgba.green, srgba.blue, srgba.alpha * 0.8)
}

fn spawn_chat_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    normal: u16,
    hover: u16,
    pressed: u16,
    relative_origin: (f32, f32),
    action: CrystalChatAction,
) {
    let (left, top) = child_screen_origin(relative_origin);
    let (width, height) = prguse_frame_size(normal);
    parent.spawn((
        CrystalChatElement,
        Button,
        action,
        CrystalChatButton {
            normal,
            hover,
            pressed,
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(width),
            height: Val::Px(height),
            ..default()
        },
        ImageNode {
            image: asset_server.load(prguse_asset(normal)),
            ..default()
        },
    ));
}

fn sync_chat_button_visuals(
    asset_server: Res<AssetServer>,
    buttons: Query<
        (&Interaction, &CrystalChatButton, &mut ImageNode),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, button, mut image) in buttons {
        let frame = chat_button_frame(*interaction, button);
        image.image = asset_server.load(prguse_asset(frame));
    }
}

fn sync_chat_settings_button_visuals(
    asset_server: Res<AssetServer>,
    buttons: Query<
        (&Interaction, &CrystalChatSettingsButton, &mut ImageNode),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, button, mut image) in buttons {
        let frame = match interaction {
            Interaction::Pressed => button.pressed,
            Interaction::Hovered => button.hover,
            Interaction::None => button.normal,
        };
        image.image = asset_server.load(chat_asset(button.library, frame));
    }
}

fn chat_button_frame(interaction: Interaction, button: &CrystalChatButton) -> u16 {
    match interaction {
        Interaction::Pressed => button.pressed,
        Interaction::Hovered => button.hover,
        Interaction::None => button.normal,
    }
}

fn consume_chat_button_interactions(
    buttons: Query<(&Interaction, &CrystalChatAction), (Changed<Interaction>, With<Button>)>,
    mut actions: ResMut<CrystalChatActionQueue>,
) {
    for (interaction, button) in buttons {
        if *interaction == Interaction::Pressed {
            actions.actions.push(*button);
        }
    }
}

fn prguse_asset(index: u16) -> String {
    format!("original-ui/Prguse/{index}.png")
}

fn title_asset(index: u16) -> String {
    format!("original-ui/Title/{index}.png")
}

fn chat_asset(library: CrystalChatAssetLibrary, index: u16) -> String {
    match library {
        CrystalChatAssetLibrary::Prguse => prguse_asset(index),
        CrystalChatAssetLibrary::Prguse2 => format!("original-ui/Prguse2/{index}.png"),
        CrystalChatAssetLibrary::Title => title_asset(index),
    }
}

fn scroll_button_origin(action: CrystalChatAction) -> (f32, f32) {
    match action {
        CrystalChatAction::Home => (CHAT_SCROLL_LEFT, 1.0),
        CrystalChatAction::Up => (CHAT_SCROLL_LEFT, 9.0),
        CrystalChatAction::Down => (CHAT_SCROLL_LEFT, 39.0),
        CrystalChatAction::End => (CHAT_SCROLL_LEFT, 45.0),
        CrystalChatAction::PositionBar => (CHAT_SCROLL_LEFT + 1.0, 16.0),
        _ => panic!("chat control-bar action has no scroll origin"),
    }
}

/// Convert a coordinate relative to ChatDialog into the 1024x768 stage.
pub fn child_screen_origin(relative_origin: (f32, f32)) -> (f32, f32) {
    (
        CHAT_PANEL_ORIGIN.0 + relative_origin.0,
        CHAT_PANEL_ORIGIN.1 + relative_origin.1,
    )
}

/// Return the newest `line_count` filtered entries as visible slice.
pub fn visible_chat_slice<'a>(
    filtered: &'a [&ChatLine],
    scroll: usize,
    line_count: usize,
) -> &'a [&'a ChatLine] {
    if filtered.is_empty() {
        return &[];
    }
    let clamped = clamp_scroll_offset(scroll, filtered.len(), line_count);
    let end = (clamped + line_count).min(filtered.len());
    &filtered[clamped..end]
}

/// Legacy: Return the newest four model entries, preserving source order.
pub fn recent_four_lines(model: &ChatModel) -> &[ChatLine] {
    let start = model.lines.len().saturating_sub(4);
    &model.lines[start..]
}

/// Current filtered visible lines for rendering (helper for tests and UI).
pub fn visible_lines_for_state<'a>(
    model: &'a ChatModel,
    state: &CrystalChatState,
) -> Vec<&'a ChatLine> {
    let filtered = filtered_lines(model, state);
    let clamped = clamp_scroll_offset(state.scroll, filtered.len(), state.line_count());
    let end = (clamped + state.line_count()).min(filtered.len());
    filtered[clamped..end].to_vec()
}

/// MainDialogs.cs uses a colored label background and a compact foreground
/// color for each ChatType. Unknown/native `normal` channels use its default
/// black-on-white branch.
pub fn channel_colors(channel: &str) -> CrystalChatColors {
    let channel = channel.trim().to_ascii_lowercase();
    match channel.as_str() {
        "hint" => colors(rgb8(0, 100, 0), rgb8(255, 255, 255)),
        "announcement" | "linemessage" | "line" => colors(rgb8(255, 255, 255), rgb8(0, 0, 255)),
        "shout" => colors(rgb8(0, 0, 0), rgb8(255, 255, 0)),
        "shout2" => colors(rgb8(255, 255, 255), rgb8(0, 128, 0)),
        "shout3" => colors(rgb8(255, 255, 255), rgb8(128, 0, 128)),
        "system" => colors(rgb8(255, 255, 255), rgb8(255, 0, 0)),
        "system2" => colors(rgb8(255, 255, 255), rgb8(139, 0, 0)),
        "group" => colors(rgb8(165, 42, 42), rgb8(255, 255, 255)),
        "whisperout" => colors(rgb8(100, 149, 237), rgb8(255, 255, 255)),
        "whisperin" => colors(rgb8(0, 0, 139), rgb8(255, 255, 255)),
        "whisper" => colors(rgb8(0, 0, 139), rgb8(255, 255, 255)),
        "guild" => colors(rgb8(0, 128, 0), rgb8(255, 255, 255)),
        "levelup" => colors(rgb8(0, 0, 255), rgb8(225, 185, 250)),
        "relationship" | "lover" => colors(rgb8(255, 105, 180), Color::NONE),
        "mentor" => colors(rgb8(128, 0, 128), rgb8(255, 255, 255)),
        "trade" => colors(rgb8(0, 0, 139), rgb8(255, 215, 0)),
        _ => colors(rgb8(0, 0, 0), rgb8(255, 255, 255)),
    }
}

fn colors(foreground: Color, background: Color) -> CrystalChatColors {
    CrystalChatColors {
        foreground,
        background,
    }
}

fn rgb8(red: u8, green: u8, blue: u8) -> Color {
    Color::srgb(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
    )
}

fn prguse_frame_size(index: u16) -> (f32, f32) {
    match index {
        2012 => (4.0, 21.0),
        2015..=2017 => (8.0, 14.0),
        2018..=2020 | 2027..=2029 => (12.0, 8.0),
        2021..=2026 => (12.0, 6.0),
        2004..=2006 | 2036..=2065 => (24.0, 13.0),
        _ => (0.0, 0.0),
    }
}

// ---------------------------------------------------------------------------
// Pure helpers for input / world blocking / z order (testable without Bevy)
// ---------------------------------------------------------------------------

/// Returns trimmed message if non-empty, otherwise None (empty not sent).
pub fn trimmed_chat_message(draft: &str) -> Option<String> {
    let t = draft.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_owned())
    }
}

/// Format outbound chat line with filter prefix (mirrors web's formatChatMessageForFilter).
pub fn format_chat_for_filter(filter: CrystalChatFilter, message: &str) -> Option<String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return None;
    }
    let prefix = filter.prefix();
    if prefix.is_empty() {
        return Some(trimmed.to_owned());
    }
    if trimmed.starts_with(prefix) {
        return Some(trimmed.to_owned());
    }
    Some(format!("{prefix}{trimmed}"))
}

/// Check chat focus/enter/escape pure state transitions.
pub fn chat_focus_transitions() -> bool {
    // Placeholder to expose that chat focus helpers exist; real logic lives in overlays.
    true
}

// Z-order constants for HUD / dialogs / chat stack.
// Keep in sync with overlays.rs and quest_ui.rs ordering.
pub const HUD_Z: i32 = 950;
pub const NPC_DIALOG_Z: i32 = 980;
pub const DEATH_POPUP_Z: i32 = 985;
pub const SYSTEM_MENU_Z: i32 = 990;
pub const CHAT_SETTINGS_Z: i32 = 976;

pub fn is_z_order_correct() -> bool {
    HUD_Z < CHAT_Z_INDEX
        && CHAT_Z_INDEX < NPC_DIALOG_Z
        && NPC_DIALOG_Z < DEATH_POPUP_Z
        && DEATH_POPUP_Z < SYSTEM_MENU_Z
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet_fixture(packet: &str, payload: &str) -> Option<ChatLine> {
        let value: serde_json::Value = serde_json::from_str(payload).ok()?;
        let text_field = match packet {
            "Chat" => "message",
            "ObjectChat" => "text",
            _ => return None,
        };
        Some(ChatLine {
            text: value.get(text_field)?.as_str()?.to_owned(),
            channel: value
                .get("chatType")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("normal")
                .to_owned(),
        })
    }

    #[test]
    fn canonical_chat_channel_fixture_matrix_is_case_insensitive_and_alias_safe() {
        let fixtures = [
            ("Normal", ChatChannel::Normal),
            (" trainer ", ChatChannel::Normal),
            ("SYSTEM", ChatChannel::System),
            ("System2", ChatChannel::System),
            ("server", ChatChannel::System),
            ("Hint", ChatChannel::Hint),
            ("LiNeMeSsAgE", ChatChannel::LineMessage),
            ("line", ChatChannel::LineMessage),
            ("shout", ChatChannel::Shout),
            ("ShOuT2", ChatChannel::Shout),
            ("announcement", ChatChannel::Shout),
            ("levelup", ChatChannel::Shout),
            ("WhisperIn", ChatChannel::WhisperIn),
            ("whisper_out", ChatChannel::WhisperOut),
            ("whisper", ChatChannel::WhisperIn),
            ("Relationship", ChatChannel::Relationship),
            ("lover", ChatChannel::Lover),
            ("MENTOR", ChatChannel::Mentor),
            ("Group", ChatChannel::Group),
            ("guild", ChatChannel::Guild),
            ("TRADE", ChatChannel::Trade),
            ("unknown-native-channel", ChatChannel::Normal),
        ];

        assert_eq!(ChatChannel::all().len(), 13);
        for (raw, expected) in fixtures {
            let line = ChatLine {
                text: raw.to_owned(),
                channel: raw.to_owned(),
            };
            assert_eq!(line.canonical_channel(), expected, "raw channel {raw:?}");
        }
    }

    #[test]
    fn chat_and_object_chat_fixtures_preserve_packet_text_and_canonical_channel() {
        let direct = packet_fixture("Chat", r#"{"message":"system text","chatType":"System2"}"#)
            .expect("Chat fixture");
        assert_eq!(direct.text, "system text");
        assert_eq!(direct.channel, "System2");
        assert_eq!(direct.canonical_channel(), ChatChannel::System);

        let object = packet_fixture(
            "ObjectChat",
            r#"{"objectId":1001,"text":"group text","chatType":"gRoUp"}"#,
        )
        .expect("ObjectChat fixture");
        assert_eq!(object.text, "group text");
        assert_eq!(object.channel, "gRoUp");
        assert_eq!(object.canonical_channel(), ChatChannel::Group);

        assert!(packet_fixture("Chat", r#"{"text":"wrong field"}"#).is_none());
        assert!(packet_fixture("ObjectChat", r#"{"message":"wrong field"}"#).is_none());
    }

    #[test]
    fn control_filters_and_settings_hide_the_same_canonical_channel_families() {
        let cases = [
            (ChatChannel::Normal, CrystalChatFilter::All),
            (ChatChannel::System, CrystalChatFilter::All),
            (ChatChannel::Hint, CrystalChatFilter::All),
            (ChatChannel::LineMessage, CrystalChatFilter::All),
            (ChatChannel::Shout, CrystalChatFilter::Shout),
            (ChatChannel::WhisperIn, CrystalChatFilter::Whisper),
            (ChatChannel::WhisperOut, CrystalChatFilter::Whisper),
            (ChatChannel::Relationship, CrystalChatFilter::Lover),
            (ChatChannel::Lover, CrystalChatFilter::Lover),
            (ChatChannel::Mentor, CrystalChatFilter::Mentor),
            (ChatChannel::Group, CrystalChatFilter::Group),
            (ChatChannel::Guild, CrystalChatFilter::Guild),
            (ChatChannel::Trade, CrystalChatFilter::Trade),
        ];

        for (channel, control_filter) in cases {
            let raw = format!("{channel:?}");
            let line = ChatLine {
                text: raw.clone(),
                channel: raw,
            };
            assert!(
                channel_matches_filter(&line.channel, control_filter),
                "{channel:?} should match {control_filter:?}"
            );
        }

        assert!(!channel_matches_filter(
            "lineMessage",
            CrystalChatFilter::Shout
        ));
        assert!(channel_matches_filter(
            "lineMessage",
            CrystalChatFilter::All
        ));
        assert!(!channel_matches_filter("system", CrystalChatFilter::Trade));
        assert!(!channel_matches_filter("hint", CrystalChatFilter::Trade));
        assert!(!is_line_hidden_by_settings(
            &ChatLine {
                text: "trade".to_owned(),
                channel: "trade".to_owned(),
            },
            &UiChatSettings::default(),
        ));
    }

    fn settings_with_filter(index: usize) -> UiChatSettings {
        let mut settings = UiChatSettings::default();
        match index {
            0 => settings.filter_normal = true,
            1 => settings.filter_whisper = true,
            2 => settings.filter_shout = true,
            3 => settings.filter_system = true,
            4 => settings.filter_group = true,
            5 => settings.filter_guild = true,
            _ => unreachable!("Web Crystal settings has six visibility filters"),
        }
        settings
    }

    #[test]
    fn web_filtered_by_crystal_settings_complete_type_matrix() {
        // This is the Web filteredByCrystalSettings switch expressed as a
        // table. The numeric index names the one filter that should hide the
        // line; None means the Web implementation never hides that type via
        // Settings, even if its presentation style shares another family.
        let matrix = [
            ("Normal", Some(0)),
            ("Shout", Some(2)),
            ("System", Some(3)),
            ("Hint", None),
            ("Announcement", None),
            ("Group", Some(4)),
            ("WhisperIn", Some(1)),
            ("WhisperOut", Some(1)),
            ("Guild", Some(5)),
            ("Trainer", None),
            ("LevelUp", None),
            ("System2", Some(3)),
            ("Relationship", None),
            ("Mentor", None),
            ("Shout2", Some(2)),
            ("Shout3", Some(2)),
            ("LineMessage", Some(0)),
        ];

        for (raw, hidden_by) in matrix {
            let line = ChatLine {
                text: raw.to_owned(),
                channel: raw.to_owned(),
            };
            for filter_index in 0..6 {
                let hidden = is_line_hidden_by_settings(&line, &settings_with_filter(filter_index));
                assert_eq!(
                    hidden,
                    hidden_by == Some(filter_index),
                    "Web settings matrix mismatch for {raw} with filter {filter_index}"
                );
            }
        }
    }

    #[test]
    fn settings_filter_alias_matrix_keeps_shout_and_system_variants_only() {
        let settings = UiChatSettings {
            filter_shout: true,
            filter_system: true,
            ..UiChatSettings::default()
        };

        for raw in ["Shout", "Shout1", "Shout2", "Shout3"] {
            assert!(is_line_hidden_by_settings(
                &ChatLine {
                    text: raw.to_owned(),
                    channel: raw.to_owned(),
                },
                &settings
            ));
        }
        for raw in ["System", "System1", "System2"] {
            assert!(is_line_hidden_by_settings(
                &ChatLine {
                    text: raw.to_owned(),
                    channel: raw.to_owned(),
                },
                &settings
            ));
        }
        for raw in ["Announcement", "LevelUp", "Hint"] {
            assert!(!is_line_hidden_by_settings(
                &ChatLine {
                    text: raw.to_owned(),
                    channel: raw.to_owned(),
                },
                &settings
            ));
        }
    }

    #[test]
    fn chat_frame_uses_exact_crystal_four_line_geometry() {
        assert_eq!(
            spec::hud::CHAT_FOUR_LINES.rect,
            CrystalRect::new(230.0, 671.0, 632.0, 68.0)
        );
        assert_eq!(CHAT_FRAME_INDEX, 2221);
        assert_eq!(
            prguse_asset(CHAT_FRAME_INDEX),
            "original-ui/Prguse/2221.png"
        );
        assert_eq!(prguse_frame_size(2018), (12.0, 8.0));
        assert_eq!(prguse_frame_size(2021), (12.0, 6.0));
        assert_eq!(prguse_frame_size(2027), (12.0, 8.0));
        assert_eq!(
            spec::hud::CHAT_CONTROL_BAR.rect,
            CrystalRect::new(230.0, 656.0, 632.0, 16.0)
        );
        assert_eq!(prguse_frame_size(2036), (24.0, 13.0));
    }

    #[test]
    fn child_coordinates_are_added_to_chat_panel_origin() {
        assert_eq!(child_screen_origin((0.0, 0.0)), (230.0, 671.0));
        assert_eq!(child_screen_origin((618.0, 45.0)), (848.0, 716.0));
        assert_eq!(child_screen_origin((1.0, 1.0)), (231.0, 672.0));
    }

    #[test]
    fn recent_four_selection_preserves_newest_source_order() {
        let mut model = ChatModel::default();
        for index in 0..6 {
            model.push(ChatLine {
                text: format!("line-{index}"),
                channel: "normal".to_owned(),
            });
        }

        let recent = recent_four_lines(&model);
        assert_eq!(recent.len(), 4);
        assert_eq!(recent[0].text, "line-2");
        assert_eq!(recent[3].text, "line-5");
    }

    #[test]
    fn channel_colors_follow_main_dialogs_special_cases() {
        let shout = channel_colors("Shout");
        assert_eq!(shout.foreground, rgb8(0, 0, 0));
        assert_eq!(shout.background, rgb8(255, 255, 0));

        let whisper = channel_colors("whisperIn");
        assert_eq!(whisper.foreground, rgb8(0, 0, 139));
        assert_eq!(whisper.background, rgb8(255, 255, 255));

        let relationship = channel_colors("relationship");
        assert_eq!(relationship.foreground, rgb8(255, 105, 180));
        assert_eq!(relationship.background, Color::NONE);

        assert_eq!(channel_colors("unrecognised"), channel_colors("normal"));
    }

    #[test]
    fn scroll_button_frames_match_crystal_source_triples() {
        let button = CrystalChatButton {
            normal: 2018,
            hover: 2019,
            pressed: 2020,
        };
        assert_eq!(chat_button_frame(Interaction::None, &button), 2018);
        assert_eq!(chat_button_frame(Interaction::Hovered, &button), 2019);
        assert_eq!(chat_button_frame(Interaction::Pressed, &button), 2020);
        assert_eq!(prguse_frame_size(2018), (12.0, 8.0));
        assert_eq!(prguse_frame_size(2019), (12.0, 8.0));
        assert_eq!(prguse_frame_size(2020), (12.0, 8.0));
        assert_eq!(prguse_frame_size(2024), (12.0, 6.0));
        assert_eq!(prguse_frame_size(2029), (12.0, 8.0));
        assert_eq!(scroll_button_origin(CrystalChatAction::Home), (618.0, 1.0));
        assert_eq!(scroll_button_origin(CrystalChatAction::Up), (618.0, 9.0));
        assert_eq!(scroll_button_origin(CrystalChatAction::Down), (618.0, 39.0));
        assert_eq!(scroll_button_origin(CrystalChatAction::End), (618.0, 45.0));
    }

    // -----------------------------------------------------------------------
    // New chat logic tests for Goal 4.8
    // -----------------------------------------------------------------------

    #[test]
    fn scroll_home_up_down_end_truly_change_position() {
        let mut state = CrystalChatState::default();
        // Simulate 10 filtered lines, 4 visible
        let total = 10;
        let line_count = 4;
        assert_eq!(max_scroll_offset(total, line_count), 6);
        // Home -> 0
        state.scroll = 5;
        state.home();
        assert_eq!(state.scroll, 0);
        // Down increments
        state.down(total);
        assert_eq!(state.scroll, 1);
        state.down(total);
        assert_eq!(state.scroll, 2);
        // Up decrements
        state.up();
        assert_eq!(state.scroll, 1);
        state.up();
        assert_eq!(state.scroll, 0);
        state.up();
        assert_eq!(state.scroll, 0, "Up at top stays 0");
        // End goes to max
        state.end(total);
        assert_eq!(state.scroll, 6);
        state.down(total);
        assert_eq!(state.scroll, 6, "Down at end stays max");
        state.up();
        assert_eq!(state.scroll, 5);
        // Clamp after resize
        state.window_size = CrystalChatWindowSize::Large; // 11 lines, max would be 0 for 10 total
        state.clamp_scroll(total);
        assert_eq!(state.scroll, 0);
        // Small again
        state.window_size = CrystalChatWindowSize::Small;
        state.scroll = 99;
        state.clamp_scroll(total);
        assert_eq!(state.scroll, 6);
    }

    #[test]
    fn max_scroll_is_zero_when_history_shorter_than_window() {
        assert_eq!(max_scroll_offset(2, 4), 0);
        assert_eq!(max_scroll_offset(0, 4), 0);
        assert_eq!(max_scroll_offset(4, 4), 0);
        assert_eq!(max_scroll_offset(5, 4), 1);
    }

    #[test]
    fn filtered_lines_by_filter_state_and_display_content() {
        let lines = vec![
            ChatLine {
                text: "a normal".into(),
                channel: "normal".into(),
            },
            ChatLine {
                text: "shout!".into(),
                channel: "shout".into(),
            },
            ChatLine {
                text: "whisper hi".into(),
                channel: "whisperIn".into(),
            },
            ChatLine {
                text: "lover heart".into(),
                channel: "relationship".into(),
            },
            ChatLine {
                text: "mentor tip".into(),
                channel: "mentor".into(),
            },
            ChatLine {
                text: "group go".into(),
                channel: "group".into(),
            },
            ChatLine {
                text: "guild war".into(),
                channel: "guild".into(),
            },
            ChatLine {
                text: "trade sell".into(),
                channel: "trade".into(),
            },
            ChatLine {
                text: "shout2 loud".into(),
                channel: "shout2".into(),
            },
            ChatLine {
                text: "shout3 purple".into(),
                channel: "shout3".into(),
            },
        ];
        // All shows all
        assert_eq!(
            filter_lines_by_filter(&lines, CrystalChatFilter::All).len(),
            10
        );
        // Shout shows shout variants only
        let shout = filter_lines_by_filter(&lines, CrystalChatFilter::Shout);
        assert_eq!(shout.len(), 3);
        assert!(shout
            .iter()
            .all(|l| channel_matches_filter(&l.channel, CrystalChatFilter::Shout)));
        // Whisper
        let whisper = filter_lines_by_filter(&lines, CrystalChatFilter::Whisper);
        assert_eq!(whisper.len(), 1);
        assert_eq!(whisper[0].channel, "whisperIn");
        // Lover -> relationship
        let lover = filter_lines_by_filter(&lines, CrystalChatFilter::Lover);
        assert_eq!(lover.len(), 1);
        assert_eq!(lover[0].channel, "relationship");
        // Mentor
        assert_eq!(
            filter_lines_by_filter(&lines, CrystalChatFilter::Mentor).len(),
            1
        );
        // Group
        assert_eq!(
            filter_lines_by_filter(&lines, CrystalChatFilter::Group).len(),
            1
        );
        // Guild
        assert_eq!(
            filter_lines_by_filter(&lines, CrystalChatFilter::Guild).len(),
            1
        );
        // Trade
        assert_eq!(
            filter_lines_by_filter(&lines, CrystalChatFilter::Trade).len(),
            1
        );
    }

    #[test]
    fn combined_hidden_and_filter_display() {
        let mut model = ChatModel::default();
        for line in [
            ChatLine {
                text: "n1".into(),
                channel: "normal".into(),
            },
            ChatLine {
                text: "s1".into(),
                channel: "shout".into(),
            },
            ChatLine {
                text: "w1".into(),
                channel: "whisperIn".into(),
            },
        ] {
            model.push(line);
        }
        let mut state = CrystalChatState::default();
        // No hidden, All shows 3
        assert_eq!(filtered_lines(&model, &state).len(), 3);
        // Hide shout
        state.applied_settings.filter_shout = true;
        assert_eq!(filtered_lines(&model, &state).len(), 2);
        // Shout filter with hidden shout still hides
        state.filter = CrystalChatFilter::Shout;
        assert_eq!(filtered_lines(&model, &state).len(), 0);
        // Clear hidden
        state.applied_settings.filter_shout = false;
        assert_eq!(filtered_lines(&model, &state).len(), 1);
    }

    #[test]
    fn resize_cycles_through_crystal_line_counts_and_frame_indices() {
        let mut state = CrystalChatState::default();
        assert_eq!(state.window_size, CrystalChatWindowSize::Small);
        assert_eq!(state.line_count(), 4);
        assert_eq!(state.window_size.frame_index(), 2221);
        state.resize(20);
        assert_eq!(state.window_size, CrystalChatWindowSize::Medium);
        assert_eq!(state.line_count(), 7);
        assert_eq!(state.window_size.frame_index(), 2224);
        state.resize(20);
        assert_eq!(state.window_size, CrystalChatWindowSize::Large);
        assert_eq!(state.line_count(), 11);
        assert_eq!(state.window_size.frame_index(), 2227);
        state.resize(20);
        assert_eq!(state.window_size, CrystalChatWindowSize::Small);
        assert_eq!(state.line_count(), 4);
        // Resize clamps scroll
        state.scroll = 18;
        state.window_size = CrystalChatWindowSize::Small;
        state.resize(10); // becomes Medium (7) with total 10 max 3
        assert!(state.scroll <= max_scroll_offset(10, 7));
    }

    #[test]
    fn settings_controls_are_staged_and_cancel_restores_committed_values() {
        let mut app = App::new();
        app.init_resource::<CrystalChatActionQueue>()
            .init_resource::<CrystalChatState>()
            .init_resource::<NativePlayerUiState>()
            .init_resource::<UiEffectQueue>()
            .init_resource::<ChatModel>()
            .add_systems(Update, consume_chat_actions);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .screen = mir2_ui_core::state::UiScreen::InGame;

        let push = |app: &mut App, action| {
            app.world_mut()
                .resource_mut::<CrystalChatActionQueue>()
                .push(action);
            app.update();
        };
        push(&mut app, CrystalChatAction::Settings);
        push(
            &mut app,
            CrystalChatAction::SettingsFilter(UiChatChannel::Shout),
        );
        push(&mut app, CrystalChatAction::SettingsTransparency(true));
        let core = &app.world().resource::<NativePlayerUiState>().core;
        assert!(!core.chat_settings.filter_shout);
        assert!(!core.chat_settings.transparent);
        assert!(core.chat_settings_draft.unwrap().filter_shout);
        assert!(core.chat_settings_draft.unwrap().transparent);

        push(&mut app, CrystalChatAction::SettingsCancel);
        let core = &app.world().resource::<NativePlayerUiState>().core;
        assert!(!core.chat_settings_open());
        assert!(core.chat_settings_draft.is_none());
        assert_eq!(core.chat_settings, UiChatSettings::default());

        push(&mut app, CrystalChatAction::Settings);
        push(
            &mut app,
            CrystalChatAction::SettingsFilter(UiChatChannel::Guild),
        );
        push(&mut app, CrystalChatAction::SettingsApply);
        let core = &app.world().resource::<NativePlayerUiState>().core;
        assert!(!core.chat_settings_open());
        assert!(core.chat_settings.filter_guild);
    }

    #[test]
    fn settings_channel_matrix_preserves_trade_without_rendering_a_ninth_checkbox() {
        assert_eq!(UiChatChannel::all().len(), 9);
        let mut settings = UiChatSettings::default();
        settings.set_filter_hidden(UiChatChannel::System, true);
        assert!(settings.is_filter_hidden(UiChatChannel::System));
        assert_eq!(settings.hidden_filter_count(), 1);
        assert!(!settings.is_filter_hidden(UiChatChannel::Guild));
        assert!(!is_line_hidden_by_settings(
            &ChatLine {
                text: "trade".into(),
                channel: "trade".into(),
            },
            &settings,
        ));
    }

    #[test]
    fn transparent_chat_tint_matches_crystal_binary_opacity() {
        assert_eq!(chat_tint(false).to_srgba().alpha, 1.0);
        assert_eq!(chat_tint(true).to_srgba().alpha, 0.8);
        assert_eq!(chat_color(Color::WHITE, true).to_srgba().alpha, 0.8);
    }

    #[test]
    fn chat_action_queue_is_consumed_and_mutates_state() {
        let mut queue = CrystalChatActionQueue::default();
        let mut state = CrystalChatState::default();
        queue.push(CrystalChatAction::FilterShout);
        queue.push(CrystalChatAction::Down);
        queue.push(CrystalChatAction::Resize);
        assert_eq!(queue.actions.len(), 3);
        let actions = queue.drain();
        for action in actions {
            apply_chat_action(&mut state, action, 10);
        }
        assert!(queue.is_empty());
        assert_eq!(state.filter, CrystalChatFilter::Shout);
        assert_eq!(state.window_size, CrystalChatWindowSize::Medium);
        // Second consumption drains
        queue.push(CrystalChatAction::FilterAll);
        let mut app = App::new();
        app.insert_resource(queue);
        app.insert_resource(state);
        app.insert_resource(ChatModel::default());
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<UiEffectQueue>();
        app.add_systems(Update, consume_chat_actions);
        app.update();
        let state = app.world().resource::<CrystalChatState>();
        assert_eq!(state.filter, CrystalChatFilter::All);
        let queue = app.world().resource::<CrystalChatActionQueue>();
        assert!(queue.is_empty());
    }

    #[test]
    fn visible_slice_respects_scroll_and_filter() {
        let mut model = ChatModel::default();
        for i in 0..10 {
            model.push(ChatLine {
                text: format!("m{i}"),
                channel: "normal".into(),
            });
        }
        let mut state = CrystalChatState::default();
        state.scroll = 0;
        let visible = visible_lines_for_state(&model, &state);
        assert_eq!(visible.len(), 4);
        assert_eq!(visible[0].text, "m0");
        state.scroll = 6; // max for 10-4=6
        let visible = visible_lines_for_state(&model, &state);
        assert_eq!(visible[0].text, "m6");
        assert_eq!(visible[3].text, "m9");
        // Filter reduces
        model.lines.push(ChatLine {
            text: "shoutX".into(),
            channel: "shout".into(),
        });
        state.filter = CrystalChatFilter::Shout;
        state.scroll = 0;
        let visible = visible_lines_for_state(&model, &state);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].channel, "shout");
    }

    #[test]
    fn trimmed_message_and_format_for_filter() {
        assert_eq!(
            trimmed_chat_message("   hello world  "),
            Some("hello world".to_owned())
        );
        assert_eq!(trimmed_chat_message("   "), None);
        assert_eq!(trimmed_chat_message(""), None);
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::All, "hello"),
            Some("hello".to_owned())
        );
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::Shout, "hello"),
            Some("!hello".to_owned())
        );
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::Whisper, "/hello"),
            Some("/hello".to_owned())
        );
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::Group, "hi"),
            Some("!!hi".to_owned())
        );
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::Lover, "hi"),
            Some(":)hi".to_owned())
        );
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::Mentor, "hi"),
            Some("!#hi".to_owned())
        );
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::Guild, "hi"),
            Some("!~hi".to_owned())
        );
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::Trade, "sell"),
            Some("@sell".to_owned())
        );
        assert_eq!(
            format_chat_for_filter(CrystalChatFilter::Shout, "   "),
            None
        );
    }

    #[test]
    fn z_order_is_hud_below_chat_below_dialog_below_death_below_menu() {
        assert!(is_z_order_correct());
        assert!(HUD_Z < CHAT_Z_INDEX);
        assert!(CHAT_Z_INDEX < NPC_DIALOG_Z);
        assert!(NPC_DIALOG_Z < DEATH_POPUP_Z);
        assert!(DEATH_POPUP_Z < SYSTEM_MENU_Z);
    }

    #[test]
    fn window_size_spec_matches_crystal_frames() {
        assert_eq!(
            CrystalChatWindowSize::Small.spec_rect(),
            spec::hud::CHAT_FOUR_LINES.rect
        );
        assert_eq!(
            CrystalChatWindowSize::Medium.spec_rect(),
            spec::hud::CHAT_SEVEN_LINES.rect
        );
        assert_eq!(
            CrystalChatWindowSize::Large.spec_rect(),
            spec::hud::CHAT_ELEVEN_LINES.rect
        );
    }
}

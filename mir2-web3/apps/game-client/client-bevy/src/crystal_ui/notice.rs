//! Crystal login-notice dialog for the Windows-native host.
//!
//! The server remains authoritative for when a notice is delivered. This
//! module owns only renderer-local visibility, scrolling, and the exact
//! Crystal-authored dialog geometry/assets from `NoticeDialog.cs`.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::text::{Justify, LineBreak, TextLayout};
use bevy::ui::{Display, Node, Overflow, PositionType, Val};
use unicode_width::UnicodeWidthStr;

use super::typography::crystal_text_font;

pub const NOTICE_WIDTH: f32 = 316.0;
pub const NOTICE_HEIGHT: f32 = 466.0;
pub const NOTICE_LEFT: f32 = (1024.0 - NOTICE_WIDTH) / 2.0;
pub const NOTICE_TOP: f32 = ((768.0 - NOTICE_HEIGHT) / 3.0).floor();
pub const NOTICE_MAXIMUM_LINES: usize = 19;
pub const NOTICE_Z_INDEX: i32 = 995;

const NOTICE_TITLE_FONT_PX: f32 = 10.0 * 96.0 / 72.0;
const NOTICE_BODY_FONT_PX: f32 = 10.0 * 96.0 / 72.0;
const NOTICE_BODY_LEFT: f32 = 25.0;
const NOTICE_BODY_WIDTH: f32 = 264.0;
const NOTICE_SCROLL_GUTTER_LEFT: f32 = 293.0;
const NOTICE_SCROLL_TOP: f32 = 46.0;
const NOTICE_SCROLL_BOTTOM: f32 = 399.0;
const NOTICE_BODY_WRAP_COLUMNS: usize = 38;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticePacketUpdate {
    pub generation: u64,
    pub sequence: u64,
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Resource)]
pub struct NoticeDialogState {
    visible: bool,
    generation: u64,
    sequence: u64,
    title: String,
    lines: Vec<String>,
    scroll_line: usize,
    has_update: bool,
}

impl NoticeDialogState {
    pub fn is_open(&self) -> bool {
        self.visible
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn scroll_line(&self) -> usize {
        self.scroll_line
    }

    pub fn visible_lines(&self) -> &[String] {
        let end = (self.scroll_line + NOTICE_MAXIMUM_LINES).min(self.lines.len());
        &self.lines[self.scroll_line.min(end)..end]
    }

    pub fn has_scroll_controls(&self) -> bool {
        self.lines.len() > NOTICE_MAXIMUM_LINES
    }

    pub fn would_accept(&self, update: &NoticePacketUpdate) -> bool {
        !self.has_update
            || update.generation > self.generation
            || (update.generation == self.generation && update.sequence > self.sequence)
    }

    /// Apply a newer authoritative packet. Empty notices are consumed but do
    /// not open a blank modal, matching Crystal's `Update` early return.
    pub fn observe(&mut self, update: NoticePacketUpdate) -> bool {
        if !self.would_accept(&update) {
            return false;
        }
        self.generation = update.generation;
        self.sequence = update.sequence;
        self.has_update = true;
        if update.message.trim().is_empty() {
            return false;
        }

        self.title = update.title;
        self.lines = normalize_notice_lines(&update.message);
        self.scroll_line = 0;
        self.visible = true;
        true
    }

    pub fn close(&mut self) -> bool {
        if !self.visible {
            return false;
        }
        self.visible = false;
        true
    }

    pub fn scroll_up(&mut self) -> bool {
        if self.scroll_line == 0 {
            return false;
        }
        self.scroll_line -= 1;
        true
    }

    pub fn scroll_down(&mut self) -> bool {
        let maximum = self.maximum_scroll_line();
        if self.scroll_line >= maximum {
            return false;
        }
        self.scroll_line += 1;
        true
    }

    pub fn scroll_wheel_lines(&mut self, delta: i32) -> bool {
        if delta == 0 || !self.has_scroll_controls() {
            return false;
        }
        let maximum = self.maximum_scroll_line() as i64;
        let next = (self.scroll_line as i64 - i64::from(delta)).clamp(0, maximum) as usize;
        if next == self.scroll_line {
            return false;
        }
        self.scroll_line = next;
        true
    }

    pub fn position_bar_top(&self) -> f32 {
        let maximum = self.maximum_scroll_line();
        if maximum == 0 {
            return NOTICE_SCROLL_TOP;
        }
        // Crystal uses integer division: `400 / (lines - MaximumLines)`.
        let interval = 400 / maximum;
        (NOTICE_SCROLL_TOP + (self.scroll_line * interval) as f32)
            .clamp(NOTICE_SCROLL_TOP, NOTICE_SCROLL_BOTTOM)
    }

    pub fn reset_session(&mut self) -> bool {
        if *self == Self::default() {
            return false;
        }
        *self = Self::default();
        true
    }

    fn maximum_scroll_line(&self) -> usize {
        self.lines.len().saturating_sub(NOTICE_MAXIMUM_LINES)
    }
}

fn normalize_notice_lines(message: &str) -> Vec<String> {
    message
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .flat_map(|line| wrap_notice_line(&strip_notice_markup(line)))
        .collect()
}

fn wrap_notice_line(line: &str) -> Vec<String> {
    if line.trim().is_empty() {
        return vec![String::new()];
    }

    let mut wrapped = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for word in line.split_whitespace() {
        let word_width = UnicodeWidthStr::width(word);
        if current.is_empty() {
            if word_width <= NOTICE_BODY_WRAP_COLUMNS {
                current.push_str(word);
                current_width = word_width;
            } else {
                push_split_word(word, &mut wrapped);
            }
            continue;
        }

        let next_width = current_width + 1 + word_width;
        if next_width <= NOTICE_BODY_WRAP_COLUMNS {
            current.push(' ');
            current.push_str(word);
            current_width = next_width;
            continue;
        }

        wrapped.push(std::mem::take(&mut current));
        current_width = 0;
        if word_width <= NOTICE_BODY_WRAP_COLUMNS {
            current.push_str(word);
            current_width = word_width;
        } else {
            push_split_word(word, &mut wrapped);
        }
    }

    if !current.is_empty() {
        wrapped.push(current);
    }

    if wrapped.is_empty() {
        vec![String::new()]
    } else {
        wrapped
    }
}

fn push_split_word(word: &str, wrapped: &mut Vec<String>) {
    let mut chunk = String::new();
    let mut chunk_width = 0usize;
    for ch in word.chars() {
        let ch_str = ch.to_string();
        let ch_width = UnicodeWidthStr::width(ch_str.as_str()).max(1);
        if !chunk.is_empty() && chunk_width + ch_width > NOTICE_BODY_WRAP_COLUMNS {
            wrapped.push(std::mem::take(&mut chunk));
            chunk_width = 0;
        }
        chunk.push(ch);
        chunk_width += ch_width;
    }
    if !chunk.is_empty() {
        wrapped.push(chunk);
    }
}

/// Crystal renders link `(text/http://...)` and colour `{text/Color}` runs as
/// overlay labels. Until link hit regions are separately modeled, preserve the
/// visible text and remove only the source markup rather than leaking syntax
/// into the dialog.
fn strip_notice_markup(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut cursor = 0;
    while cursor < line.len() {
        let remainder = &line[cursor..];
        let Some(relative_open) = remainder.find(|value| value == '(' || value == '{') else {
            output.push_str(remainder);
            break;
        };
        output.push_str(&remainder[..relative_open]);
        cursor += relative_open;
        let opener = line.as_bytes()[cursor];
        let close = if opener == b'(' { ')' } else { '}' };
        let Some(relative_end) = line[cursor + 1..].find(close) else {
            output.push_str(&line[cursor..]);
            break;
        };
        let end = cursor + 1 + relative_end;
        let content = &line[cursor + 1..end];
        let Some((text, action)) = content.split_once('/') else {
            output.push_str(&line[cursor..=end]);
            cursor = end + 1;
            continue;
        };
        let valid = opener == b'{' || action.to_ascii_lowercase().starts_with("http://");
        if valid {
            output.push_str(text);
        } else {
            output.push_str(&line[cursor..=end]);
        }
        cursor = end + 1;
    }
    output
}

#[derive(Component, Debug)]
pub struct CrystalNoticeRoot;

#[derive(Component, Debug)]
struct CrystalNoticePanel;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum CrystalNoticeAction {
    Close,
    Ok,
    Up,
    Down,
}

#[derive(Component, Debug, Clone, Copy)]
struct CrystalNoticeButtonFrames {
    library: &'static str,
    normal: u16,
    hover: u16,
    pressed: u16,
}

pub struct Mir2CrystalNoticePlugin;

impl Plugin for Mir2CrystalNoticePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NoticeDialogState>()
            .add_systems(Startup, spawn_notice_root)
            .add_systems(
                Update,
                (
                    consume_notice_buttons,
                    consume_notice_mouse_wheel,
                    render_notice_dialog,
                    sync_notice_button_visuals,
                )
                    .chain(),
            );
    }
}

fn spawn_notice_root(mut commands: Commands) {
    commands.spawn((
        CrystalNoticeRoot,
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
        GlobalZIndex(NOTICE_Z_INDEX),
    ));
}

fn render_notice_dialog(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    state: Res<NoticeDialogState>,
    mut roots: Query<(Entity, &mut Node), With<CrystalNoticeRoot>>,
) {
    if !state.is_changed() {
        return;
    }
    let Ok((root, mut root_node)) = roots.single_mut() else {
        return;
    };
    root_node.display = if state.is_open() {
        Display::Flex
    } else {
        Display::None
    };
    commands.entity(root).despawn_children();
    if !state.is_open() {
        return;
    }

    commands.entity(root).with_children(|stage| {
        stage
            .spawn((
                CrystalNoticePanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(NOTICE_LEFT),
                    top: Val::Px(NOTICE_TOP),
                    width: Val::Px(NOTICE_WIDTH),
                    height: Val::Px(NOTICE_HEIGHT),
                    // Crystal child controls are clipped by the parent image
                    // control. Bevy does not inherit that behavior unless the
                    // panel declares it explicitly, so an authored long line
                    // must never paint over the world outside this frame.
                    overflow: Overflow::clip(),
                    ..default()
                },
                ImageNode {
                    image: asset_server.load("original-ui/Prguse/961.png"),
                    ..default()
                },
            ))
            .with_children(|panel| {
                panel.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(30.0),
                        top: Val::Px(6.0),
                        width: Val::Px(250.0),
                        height: Val::Px(20.0),
                        ..default()
                    },
                    Text::new(state.title().to_owned()),
                    crystal_text_font(NOTICE_TITLE_FONT_PX),
                    TextColor(Color::srgb_u8(222, 184, 135)),
                ));

                for (line, text) in state.visible_lines().iter().enumerate() {
                    panel.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(NOTICE_BODY_LEFT),
                            top: Val::Px(50.0 + line as f32 * 20.0),
                            // Keep text before Crystal's scrollbar gutter.
                            // The upstream label is wider than its parent and
                            // relies on parent clipping; bounding each Bevy row
                            // as well makes that invariant explicit.
                            width: Val::Px(NOTICE_BODY_WIDTH),
                            height: Val::Px(20.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        Text::new(text.to_owned()),
                        crystal_text_font(NOTICE_BODY_FONT_PX),
                        TextColor(Color::WHITE),
                        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                    ));
                }

                spawn_notice_button(
                    panel,
                    &asset_server,
                    CrystalNoticeAction::Close,
                    CrystalNoticeButtonFrames {
                        library: "Prguse2",
                        normal: 360,
                        hover: 361,
                        pressed: 362,
                    },
                    (289.0, 3.0, 24.0, 21.0),
                );
                spawn_notice_button(
                    panel,
                    &asset_server,
                    CrystalNoticeAction::Ok,
                    CrystalNoticeButtonFrames {
                        library: "Title",
                        normal: 193,
                        hover: 194,
                        pressed: 195,
                    },
                    (120.0, 436.0, 68.0, 25.0),
                );

                if state.has_scroll_controls() {
                    spawn_notice_button(
                        panel,
                        &asset_server,
                        CrystalNoticeAction::Up,
                        CrystalNoticeButtonFrames {
                            library: "Prguse2",
                            normal: 470,
                            hover: 471,
                            pressed: 472,
                        },
                        (NOTICE_SCROLL_GUTTER_LEFT, 33.0, 16.0, 14.0),
                    );
                    spawn_notice_button(
                        panel,
                        &asset_server,
                        CrystalNoticeAction::Down,
                        CrystalNoticeButtonFrames {
                            library: "Prguse2",
                            normal: 473,
                            hover: 474,
                            pressed: 475,
                        },
                        (NOTICE_SCROLL_GUTTER_LEFT, 418.0, 16.0, 14.0),
                    );
                    panel.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(NOTICE_SCROLL_GUTTER_LEFT),
                            top: Val::Px(state.position_bar_top()),
                            width: Val::Px(12.0),
                            height: Val::Px(18.0),
                            ..default()
                        },
                        ImageNode {
                            image: asset_server.load("original-ui/Prguse2/205.png"),
                            ..default()
                        },
                    ));
                }
            });
    });
}

fn spawn_notice_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: CrystalNoticeAction,
    frames: CrystalNoticeButtonFrames,
    rect: (f32, f32, f32, f32),
) {
    parent.spawn((
        Button,
        action,
        frames,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(rect.0),
            top: Val::Px(rect.1),
            width: Val::Px(rect.2),
            height: Val::Px(rect.3),
            ..default()
        },
        ImageNode {
            image: asset_server.load(notice_asset(frames.library, frames.normal)),
            ..default()
        },
    ));
}

fn consume_notice_buttons(
    buttons: Query<(&Interaction, &CrystalNoticeAction), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<NoticeDialogState>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            CrystalNoticeAction::Close | CrystalNoticeAction::Ok => {
                state.close();
            }
            CrystalNoticeAction::Up => {
                state.scroll_up();
            }
            CrystalNoticeAction::Down => {
                state.scroll_down();
            }
        }
    }
}

fn consume_notice_mouse_wheel(
    mut wheel: MessageReader<MouseWheel>,
    mut state: ResMut<NoticeDialogState>,
) {
    if !state.is_open() {
        wheel.clear();
        return;
    }
    for event in wheel.read() {
        let lines = if event.y > 0.0 {
            event.y.ceil() as i32
        } else {
            event.y.floor() as i32
        };
        state.scroll_wheel_lines(lines);
    }
}

fn sync_notice_button_visuals(
    asset_server: Res<AssetServer>,
    mut buttons: Query<
        (&Interaction, &CrystalNoticeButtonFrames, &mut ImageNode),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, frames, mut image) in &mut buttons {
        let index = match interaction {
            Interaction::Pressed => frames.pressed,
            Interaction::Hovered => frames.hover,
            Interaction::None => frames.normal,
        };
        image.image = asset_server.load(notice_asset(frames.library, index));
    }
}

fn notice_asset(library: &str, index: u16) -> String {
    format!("original-ui/{library}/{index}.png")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(generation: u64, sequence: u64, message: impl Into<String>) -> NoticePacketUpdate {
        NoticePacketUpdate {
            generation,
            sequence,
            title: "Welcome".to_owned(),
            message: message.into(),
        }
    }

    #[test]
    fn exact_notice_geometry_matches_crystal_source() {
        assert_eq!((NOTICE_WIDTH, NOTICE_HEIGHT), (316.0, 466.0));
        assert_eq!((NOTICE_LEFT, NOTICE_TOP), (354.0, 100.0));
        assert_eq!(NOTICE_MAXIMUM_LINES, 19);
    }

    #[test]
    fn notice_body_is_clipped_before_crystal_scrollbar_gutter() {
        assert!(NOTICE_BODY_LEFT >= 0.0);
        assert!(NOTICE_BODY_LEFT + NOTICE_BODY_WIDTH <= NOTICE_SCROLL_GUTTER_LEFT);
        assert!(NOTICE_SCROLL_GUTTER_LEFT < NOTICE_WIDTH);
    }

    #[test]
    fn newer_notice_opens_once_and_close_does_not_reopen_from_same_snapshot() {
        let mut state = NoticeDialogState::default();
        let first = update(4, 1, "one\r\ntwo");
        assert!(state.observe(first.clone()));
        assert!(state.is_open());
        assert_eq!(state.lines(), &["one".to_owned(), "two".to_owned()]);
        assert!(state.close());
        assert!(!state.is_open());
        assert!(!state.observe(first));
        assert!(!state.is_open());
    }

    #[test]
    fn session_reset_allows_same_content_from_a_new_authoritative_delivery() {
        let mut state = NoticeDialogState::default();
        assert!(state.observe(update(7, 1, "candidate")));
        assert!(state.close());
        assert!(state.reset_session());
        assert!(state.observe(update(8, 1, "candidate")));
        assert!(state.is_open());
    }

    #[test]
    fn empty_notice_is_consumed_without_opening_blank_modal() {
        let mut state = NoticeDialogState::default();
        assert!(!state.observe(update(1, 1, " \r\n ")));
        assert!(!state.is_open());
        assert!(!state.observe(update(1, 1, "later mutation of duplicate")));
    }

    #[test]
    fn scroll_clamps_to_the_crystal_nineteen_line_window() {
        let mut state = NoticeDialogState::default();
        let message = (0..25)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\r\n");
        assert!(state.observe(update(1, 1, message)));
        assert!(state.has_scroll_controls());
        assert!(state.scroll_wheel_lines(-99));
        assert_eq!(state.scroll_line(), 6);
        assert_eq!(state.visible_lines().first().unwrap(), "line 6");
        assert!(!state.scroll_down());
        assert!(state.scroll_wheel_lines(99));
        assert_eq!(state.scroll_line(), 0);
    }

    #[test]
    fn source_colour_and_http_link_markup_preserve_visible_text() {
        assert_eq!(
            strip_notice_markup("{Warning/Red} (Rules/http://example.test)"),
            "Warning Rules"
        );
        assert_eq!(
            strip_notice_markup("(Unsafe/https://example.test)"),
            "(Unsafe/https://example.test)"
        );
    }

    #[test]
    fn notice_long_lines_wrap_before_the_scrollbar_gutter() {
        let lines = normalize_notice_lines(
            "By clicking close and continuing to play the game you are agreeing to the terms of service above.",
        );
        assert!(lines.len() > 1, "expected notice body to wrap");
        assert!(lines
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= NOTICE_BODY_WRAP_COLUMNS));
    }

    #[test]
    fn notice_wrap_preserves_blank_lines_and_breaks_oversized_tokens() {
        let lines = normalize_notice_lines("Line one\r\n\r\nSUPERCODESUPERCODESUPERCODESUPERCODE");
        assert_eq!(lines[0], "Line one");
        assert_eq!(lines[1], "");
        assert!(lines[2..]
            .iter()
            .all(|line| UnicodeWidthStr::width(line.as_str()) <= NOTICE_BODY_WRAP_COLUMNS));
    }
}

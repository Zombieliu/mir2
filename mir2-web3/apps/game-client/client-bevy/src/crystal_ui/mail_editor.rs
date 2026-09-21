//! Source LetterDialog message editing backed by the same shaped text layout
//! primitives as native friend/memo input.  Draft offsets stay UTF-8 byte
//! boundaries while the user-facing limit follows Crystal/.NET UTF-16 units.
use super::*;
use friend_dialog::text_editor::{CaretStop, EditResult, FriendTextEditor, TextLayout as EditorTextLayout, VisualLine};

pub(super) const MAIL_LETTER_BODY_RECT: CrystalRect = CrystalRect::new(15.0, 92.0, 202.0, 165.0);
/// `MailComposeParcelDialog` uses the same 202×165 edit viewport six pixels
/// lower than LetterDialog.  Keep layout shaping shared while the source
/// frame owns each control's real local origin.
pub(super) const MAIL_PARCEL_BODY_RECT: CrystalRect = CrystalRect::new(15.0, 98.0, 202.0, 165.0);
// Crystal's SendMail validation counts .NET string.Length (UTF-16 units).
pub(super) const MAIL_LETTER_BODY_LIMIT: usize = 500;
pub(super) const MAIL_LETTER_CONTENT_INSET: Vec2 = Vec2::splat(2.0);
pub(super) const MAIL_LETTER_CONTENT_SIZE: Vec2 = Vec2::new(
    MAIL_LETTER_BODY_RECT.width - MAIL_LETTER_CONTENT_INSET.x * 2.0,
    MAIL_LETTER_BODY_RECT.height - MAIL_LETTER_CONTENT_INSET.y * 2.0,
);

#[derive(Component)]
pub struct MailLetterEditText {
    pub revision: u64,
    pub text: String,
    pub viewport: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailComposition {
    value: String,
    cursor: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Default, Resource, PartialEq)]
pub struct MailLetterEditor {
    editor: Option<FriendTextEditor>,
    revision: u64,
    focused: bool,
    layout: EditorTextLayout,
    layout_text: String,
    visual_line: usize,
    captured_caret: Option<usize>,
    preferred_x: Option<f32>,
    scroll: Vec2,
    modifiers: [bool; 4],
    selecting: bool,
    composition: Option<MailComposition>,
    display_layout: EditorTextLayout,
    display_layout_text: String,
}

impl MailLetterEditor {
    pub fn active_editor(&self) -> Option<&FriendTextEditor> {
        self.editor.as_ref()
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn composition(&self) -> Option<&MailComposition> {
        self.composition.as_ref()
    }

    pub fn set_composition(&mut self, value: String, cursor: Option<(usize, usize)>) {
        self.modifiers = [false; 4];
        self.composition = (!value.is_empty()).then_some(MailComposition { value, cursor });
    }

    pub fn clear_composition(&mut self) {
        self.modifiers = [false; 4];
        self.composition = None;
        self.display_layout = EditorTextLayout::default();
        self.display_layout_text.clear();
    }

    /// Host clipboard and IME commits share the authoritative editor path, so
    /// the UTF-16 budget and grapheme-safe selection replacement stay identical.
    pub fn paste(&mut self, draft: &mut mir2_ui_core::state::MailComposeDraft, text: &str) {
        let result = self
            .editor
            .as_mut()
            .map(|editor| editor.insert_with_policy(
                text,
                friend_dialog::text_editor::InsertPolicy::FitPrefix,
            ))
            .unwrap_or(EditResult::Unchanged);
        self.commit(draft, result);
        self.sync_visual_line_for_caret();
        self.keep_caret_visible();
    }

    pub fn select_all(&mut self) {
        if let Some(editor) = self.editor.as_mut() {
            editor.select_all();
        }
    }

    pub fn selected_text(&self) -> &str {
        self.editor.as_ref().map_or("", FriendTextEditor::selected_text)
    }

    pub fn cut_selection(&mut self, draft: &mut mir2_ui_core::state::MailComposeDraft) {
        let result = self
            .editor
            .as_mut()
            .map(|editor| editor.delete(false))
            .unwrap_or(EditResult::Unchanged);
        self.commit(draft, result);
        self.sync_visual_line_for_caret();
        self.keep_caret_visible();
    }

    /// Coordinates are local to the text node. Its node already applies scroll,
    /// so native IME positioning must not subtract it again.
    pub fn ime_caret(&self) -> Option<Vec2> {
        let display = self.displayed_editor()?;
        if self.composition.is_some() {
            if let Some((x, y, height)) = self.display_caret(&display) {
                return Some(Vec2::new(x, y + height));
            }
            // The text system captures the preview in PostUpdate. Until that
            // first shaped result exists, retain the prior authoritative glyph
            // position instead of hiding the OS candidate window.
        }
        let authoritative = self.editor.as_ref()?;
        (self.layout_text == authoritative.text()).then(|| {
            self.layout
                .lines
                .get(self.visual_line)
                .and_then(|line| line.stops.iter().find(|stop| stop.byte == authoritative.caret()).map(|stop| {
                    Vec2::new(stop.x, line.y + line.height)
                }))
                .or_else(|| {
                    self.layout.lines.iter().find_map(|line| {
                        line.stops.iter().find(|stop| stop.byte == authoritative.caret()).map(|stop| {
                            Vec2::new(stop.x, line.y + line.height)
                        })
                    })
                })
        }).flatten()
    }

    pub(super) fn sync(&mut self, active: bool, message: Option<&str>) {
        let Some(message) = active.then_some(message).flatten() else {
            self.clear();
            return;
        };
        if self.editor.as_ref().is_none_or(|editor| editor.text() != message) {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            self.editor = Some(FriendTextEditor::new(
                message.to_owned(),
                MAIL_LETTER_BODY_LIMIT,
                true,
            ));
            self.revision = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.focused = true;
            self.layout = EditorTextLayout::default();
            self.layout_text.clear();
            self.visual_line = 0;
            self.captured_caret = None;
            self.preferred_x = None;
            self.scroll = Vec2::ZERO;
            self.modifiers = [false; 4];
            self.selecting = false;
            self.composition = None;
            self.display_layout = EditorTextLayout::default();
            self.display_layout_text.clear();
        }
    }

    pub(super) fn clear(&mut self) {
        self.editor = None;
        self.focused = false;
        self.layout = EditorTextLayout::default();
        self.layout_text.clear();
        self.visual_line = 0;
        self.captured_caret = None;
        self.preferred_x = None;
        self.scroll = Vec2::ZERO;
        self.modifiers = [false; 4];
        self.selecting = false;
        self.composition = None;
        self.display_layout = EditorTextLayout::default();
        self.display_layout_text.clear();
    }

    fn commit(&mut self, draft: &mut mir2_ui_core::state::MailComposeDraft, result: EditResult) {
        if result == EditResult::Changed {
            if let Some(editor) = self.editor.as_ref() {
                draft.message = editor.text().to_owned();
            }
            self.preferred_x = None;
        }
    }

    pub(super) fn edit_key(
        &mut self,
        event: &KeyboardInput,
        draft: &mut mir2_ui_core::state::MailComposeDraft,
    ) {
        let pressed = event.state == ButtonState::Pressed;
        match event.key_code {
            KeyCode::ControlLeft => self.modifiers[0] = pressed,
            KeyCode::ControlRight => self.modifiers[1] = pressed,
            KeyCode::ShiftLeft => self.modifiers[2] = pressed,
            KeyCode::ShiftRight => self.modifiers[3] = pressed,
            _ => {}
        }
        if !pressed || !self.focused {
            return;
        }
        let ctrl = self.modifiers[0] || self.modifiers[1];
        let shift = self.modifiers[2] || self.modifiers[3];
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let mut result = EditResult::Unchanged;
        match event.key_code {
            KeyCode::KeyA if ctrl => editor.select_all(),
            // Clipboard transport is host-owned. Its adapter applies only
            // after the exact live mail target is revalidated.
            KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyV if ctrl => {}
            KeyCode::Backspace => result = editor.delete(true),
            KeyCode::Delete => result = editor.delete(false),
            KeyCode::ArrowLeft | KeyCode::ArrowRight => {
                editor.horizontal(event.key_code == KeyCode::ArrowRight, shift);
                self.preferred_x = None;
            }
            KeyCode::Home | KeyCode::End => {
                let end = event.key_code == KeyCode::End;
                if !ctrl && self.layout_text == editor.text() {
                    if let Some(byte) = self.layout.line_edge(self.visual_line, end) {
                        editor.set_caret(byte, shift);
                    } else {
                        editor.home_end(end, false, shift);
                    }
                } else {
                    editor.home_end(end, ctrl, shift);
                }
                self.preferred_x = None;
            }
            KeyCode::ArrowUp | KeyCode::ArrowDown if self.layout_text == editor.text() => {
                let x = *self.preferred_x.get_or_insert_with(|| {
                    self.layout
                        .lines
                        .get(self.visual_line)
                        .and_then(|line| line.stops.iter().find(|stop| stop.byte == editor.caret()))
                        .map_or(0.0, |stop| stop.x)
                });
                if let Some((line, byte)) = self
                    .layout
                    .vertical(self.visual_line, event.key_code == KeyCode::ArrowDown, x)
                {
                    self.visual_line = line;
                    editor.set_caret(byte, shift);
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => result = editor.newline(),
            _ if !ctrl => {
                if let bevy::input::keyboard::Key::Character(chars) = &event.logical_key {
                    result = editor.insert_with_policy(
                        chars,
                        friend_dialog::text_editor::InsertPolicy::FitPrefix,
                    );
                }
            }
            _ => {}
        }
        self.commit(draft, result);
        self.sync_visual_line_for_caret();
        self.keep_caret_visible();
    }

    pub(super) fn pointer(&mut self, point: Vec2, extend: bool) {
        self.focused = true;
        let point = point + self.scroll;
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if self.layout_text == editor.text() && self.layout.valid_for(editor) {
            if let Some(byte) = self.layout.hit_test(point.x, point.y) {
                editor.set_caret(byte, extend);
                self.visual_line = self
                    .layout
                    .lines
                    .iter()
                    .position(|line| point.y >= line.y && point.y < line.y + line.height)
                    .unwrap_or(self.visual_line);
                self.preferred_x = None;
                self.keep_caret_visible();
            }
        }
    }

    pub(super) fn set_selecting(&mut self, selecting: bool) {
        self.selecting = selecting;
    }

    pub(super) fn selecting(&self) -> bool {
        self.selecting
    }

    /// Scrolls by source text lines without changing the authoritative caret
    /// or selection. The extent comes from the shaped layout, not a character
    /// count or guessed line height.
    pub(super) fn scroll_wheel_lines(&mut self, upward_lines: f32) -> bool {
        let Some(height) = self.visible_line_height() else {
            return false;
        };
        self.scroll_wheel_pixels(upward_lines * height)
    }

    /// `upward_pixels` is already in Crystal logical-stage units.
    pub(super) fn scroll_wheel_pixels(&mut self, upward_pixels: f32) -> bool {
        if !upward_pixels.is_finite() || upward_pixels == 0.0 {
            return false;
        }
        let Some(max_scroll) = self.max_scroll_y() else {
            return false;
        };
        let next = (self.scroll.y - upward_pixels).clamp(0.0, max_scroll);
        if (next - self.scroll.y).abs() <= f32::EPSILON {
            return false;
        }
        self.scroll.y = next;
        true
    }

    pub(super) fn render(&self, parent: &mut ChildSpawnerCommands) {
        self.render_at(parent, MAIL_LETTER_BODY_RECT);
    }

    pub(super) fn render_at(&self, parent: &mut ChildSpawnerCommands, body_rect: CrystalRect) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let display = self.displayed_editor().unwrap_or_else(|| editor.clone());
        let scroll = self.scroll;
        parent
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(body_rect.left),
                    top: Val::Px(body_rect.top),
                    width: Val::Px(body_rect.width),
                    height: Val::Px(body_rect.height),
                    border: UiRect::all(Val::Px(1.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                BorderColor::all(if self.focused {
                    Color::srgb(0.0, 1.0, 0.0)
                } else {
                    Color::NONE
                }),
                FocusPolicy::Block,
            ))
            .with_children(|field| {
                if self.composition.is_none() && self.layout_text == editor.text() {
                    for rect in self.layout.selection_rects(editor.selection()) {
                        field.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(MAIL_LETTER_CONTENT_INSET.x + rect.x - scroll.x),
                                top: Val::Px(MAIL_LETTER_CONTENT_INSET.y + rect.y - scroll.y),
                                width: Val::Px(rect.width),
                                height: Val::Px(rect.height),
                                ..default()
                            },
                            BackgroundColor(Color::srgb_u8(35, 85, 145)),
                        ));
                    }
                }
                field.spawn((
                    MailLetterEditText {
                        revision: self.revision,
                        text: display.text().to_owned(),
                        viewport: [MAIL_LETTER_CONTENT_SIZE.x, MAIL_LETTER_CONTENT_SIZE.y],
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(MAIL_LETTER_CONTENT_INSET.x - scroll.x),
                        top: Val::Px(MAIL_LETTER_CONTENT_INSET.y - scroll.y),
                        width: Val::Px(MAIL_LETTER_CONTENT_SIZE.x),
                        min_width: Val::Px(MAIL_LETTER_CONTENT_SIZE.x),
                        ..default()
                    },
                    Text::new(display.text()),
                    crate::crystal_ui::typography::crystal_text_font(
                        crate::crystal_ui::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX,
                    ),
                    TextColor(TEXT),
                    TextLayout::new(Justify::Left, LineBreak::AnyCharacter),
                ));
                if self.focused {
                    let caret = self
                        .composition
                        .as_ref()
                        .and_then(|_| self.display_caret(&display))
                        .or_else(|| {
                            self.layout
                                .lines
                                .get(self.visual_line)
                                .and_then(|line| {
                                    line.stops
                                        .iter()
                                        .find(|stop| stop.byte == editor.caret())
                                        .map(|stop| (stop.x, line.y, line.height))
                                })
                        })
                        .or_else(|| editor.text().is_empty().then_some((0.0, 0.0, 14.0)));
                    if let Some((x, y, height)) = caret {
                        field.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(MAIL_LETTER_CONTENT_INSET.x + x - scroll.x),
                                top: Val::Px(MAIL_LETTER_CONTENT_INSET.y + y - scroll.y),
                                width: Val::Px(1.0),
                                height: Val::Px(height),
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                        ));
                    }
                }
            });
    }

    fn keep_caret_visible(&mut self) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        if self.layout_text != editor.text() {
            return;
        }
        let Some(line) = self.layout.lines.get(self.visual_line).filter(|line| {
            line.stops.iter().any(|stop| stop.byte == editor.caret())
        }).or_else(|| {
            self.layout
                .lines
                .iter()
                .find(|line| line.stops.iter().any(|stop| stop.byte == editor.caret()))
        }) else {
            return;
        };
        if line.y < self.scroll.y {
            self.scroll.y = line.y;
        }
        if line.y + line.height > self.scroll.y + MAIL_LETTER_CONTENT_SIZE.y {
            self.scroll.y = line.y + line.height - MAIL_LETTER_CONTENT_SIZE.y;
        }
        self.scroll.y = self.scroll.y.max(0.0);
    }

    fn max_scroll_y(&self) -> Option<f32> {
        let editor = self.editor.as_ref()?;
        if self.layout_text != editor.text() || !self.layout.valid_for(editor) {
            return None;
        }
        let bottom = self
            .layout
            .lines
            .iter()
            .map(|line| line.y + line.height)
            .fold(0.0_f32, f32::max);
        Some((bottom - MAIL_LETTER_CONTENT_SIZE.y).max(0.0))
    }

    fn visible_line_height(&self) -> Option<f32> {
        let editor = self.editor.as_ref()?;
        if self.layout_text != editor.text() || !self.layout.valid_for(editor) {
            return None;
        }
        self.layout
            .lines
            .iter()
            .find(|line| self.scroll.y >= line.y && self.scroll.y < line.y + line.height)
            .or_else(|| self.layout.lines.first())
            .map(|line| line.height)
    }

    fn sync_visual_line_for_caret(&mut self) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        if self.layout_text != editor.text() {
            return;
        }
        if self
            .layout
            .lines
            .get(self.visual_line)
            .is_some_and(|line| line.stops.iter().any(|stop| stop.byte == editor.caret()))
        {
            return;
        }
        self.visual_line = self
            .layout
            .lines
            .iter()
            .position(|line| line.stops.iter().any(|stop| stop.byte == editor.caret()))
            .unwrap_or(self.visual_line);
    }

    fn displayed_editor(&self) -> Option<FriendTextEditor> {
        self.editor.as_ref().map(|editor| {
            self.composition
                .as_ref()
                .map(|composition| editor.composition_preview(&composition.value, composition.cursor))
                .unwrap_or_else(|| editor.clone())
        })
    }

    fn shape_layout(
        editor: &FriendTextEditor,
        block: &bevy::text::ComputedTextBlock,
        info: &bevy::text::TextLayoutInfo,
    ) -> EditorTextLayout {
        let scale = info.scale_factor.max(0.001);
        let mut layout = EditorTextLayout::default();
        for line in block.buffer().lines() {
            let metrics = line.metrics();
            let mut stops = Vec::new();
            for run in line.runs() {
                for cluster in run.visual_clusters() {
                    let Some(x) = cluster.visual_offset() else {
                        continue;
                    };
                    let range = cluster.text_range();
                    let mut bytes: Vec<_> = unicode_segmentation::UnicodeSegmentation::grapheme_indices(
                        editor.text(),
                        true,
                    )
                    .map(|(index, _)| index)
                    .chain(std::iter::once(editor.text().len()))
                    .filter(|byte| *byte >= range.start && *byte <= range.end)
                    .collect();
                    if cluster.is_rtl() {
                        bytes.reverse();
                    }
                    let count = bytes.len().saturating_sub(1).max(1) as f32;
                    for (index, byte) in bytes.into_iter().enumerate() {
                        stops.push(CaretStop {
                            byte,
                            x: (x + cluster.advance() * index as f32 / count) / scale,
                        });
                    }
                }
            }
            if stops.is_empty() {
                stops.push(CaretStop {
                    byte: line.text_range().start.min(editor.text().len()),
                    x: metrics.offset / scale,
                });
            }
            stops.sort_by(|a, b| a.x.total_cmp(&b.x));
            stops.dedup_by(|a, b| a.byte == b.byte && a.x == b.x);
            layout.lines.push(VisualLine {
                y: metrics.block_min_coord / scale,
                height: (metrics.block_max_coord - metrics.block_min_coord).max(1.0) / scale,
                stops,
            });
        }
        layout
    }

    pub(super) fn capture_layout(
        &mut self,
        tag: &MailLetterEditText,
        block: &bevy::text::ComputedTextBlock,
        info: &bevy::text::TextLayoutInfo,
    ) {
        if tag.revision != self.revision {
            return;
        }
        let Some(display) = self.displayed_editor() else {
            return;
        };
        if display.text() != tag.text {
            return;
        }
        let layout = Self::shape_layout(&display, block, info);
        if !layout.valid_for(&display) {
            return;
        }
        if self.composition.is_some() {
            self.accept_display_layout(layout, tag.text.clone(), display.caret());
        } else {
            self.accept_layout(layout, tag.text.clone());
        }
    }

    fn accept_layout(&mut self, layout: EditorTextLayout, text: String) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let caret = editor.caret();
        if layout
            .lines
            .get(self.visual_line)
            .is_none_or(|line| !line.stops.iter().any(|stop| stop.byte == caret))
        {
            self.visual_line = layout
                .lines
                .iter()
                .position(|line| line.stops.iter().any(|stop| stop.byte == caret))
                .unwrap_or(0);
        }
        // Layout capture runs every frame. Only a new draft or caret movement
        // should restore caret visibility; otherwise it would undo a manual
        // wheel position immediately after rendering.
        let caret_changed = self.captured_caret != Some(caret) || self.layout_text != text;
        self.layout = layout;
        self.layout_text = text;
        if caret_changed {
            self.keep_caret_visible();
        }
        self.clamp_scroll_to_extent();
        self.captured_caret = Some(caret);
    }

    fn accept_display_layout(
        &mut self,
        layout: EditorTextLayout,
        text: String,
        caret: usize,
    ) {
        self.display_layout = layout;
        self.display_layout_text = text;
        let Some(line) = self
            .display_layout
            .lines
            .iter()
            .find(|line| line.stops.iter().any(|stop| stop.byte == caret))
        else {
            return;
        };
        if line.y < self.scroll.y {
            self.scroll.y = line.y;
        }
        if line.y + line.height > self.scroll.y + MAIL_LETTER_CONTENT_SIZE.y {
            self.scroll.y = line.y + line.height - MAIL_LETTER_CONTENT_SIZE.y;
        }
        let bottom = self
            .display_layout
            .lines
            .iter()
            .map(|line| line.y + line.height)
            .fold(0.0_f32, f32::max);
        self.scroll.y = self
            .scroll
            .y
            .clamp(0.0, (bottom - MAIL_LETTER_CONTENT_SIZE.y).max(0.0));
    }

    fn display_caret(&self, display: &FriendTextEditor) -> Option<(f32, f32, f32)> {
        if self.display_layout_text != display.text()
            || !self.display_layout.valid_for(display)
        {
            return None;
        }
        self.display_layout.lines.iter().find_map(|line| {
            line.stops
                .iter()
                .find(|stop| stop.byte == display.caret())
                .map(|stop| (stop.x, line.y, line.height))
        })
    }

    fn clamp_scroll_to_extent(&mut self) {
        if let Some(max_scroll) = self.max_scroll_y() {
            self.scroll.y = self.scroll.y.clamp(0.0, max_scroll);
        }
    }

    #[cfg(test)]
    pub(super) fn install_layout(&mut self, lines: Vec<VisualLine>) {
        let text = self.editor.as_ref().map_or_else(String::new, |editor| editor.text().to_owned());
        // Test fixtures install already-shaped layout without reproducing the
        // renderer's initial caret-visibility transition.
        self.layout = EditorTextLayout { lines };
        self.layout_text = text;
        self.captured_caret = self.editor.as_ref().map(FriendTextEditor::caret);
    }

    #[cfg(test)]
    pub(super) fn scroll(&self) -> Vec2 {
        self.scroll
    }
}

pub(super) fn capture_layout_system(
    mut editor: ResMut<MailLetterEditor>,
    texts: Query<(
        &MailLetterEditText,
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
    )>,
) {
    for (tag, block, info) in &texts {
        editor.capture_layout(tag, block, info);
    }
}

#[cfg(test)]
#[path = "mail_editor_tests.rs"]
mod tests;

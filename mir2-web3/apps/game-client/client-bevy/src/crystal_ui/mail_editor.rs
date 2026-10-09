//! Source LetterDialog message editing backed by the same shaped text layout
//! primitives as native friend/memo input.  Draft offsets stay UTF-8 byte
//! boundaries while the user-facing limit follows Crystal/.NET UTF-16 units.
use bevy::prelude::*;
use bevy::input::{ButtonState, keyboard::KeyboardInput};
use bevy::ui::FocusPolicy;
use super::spec::CrystalRect;
const TEXT: Color = Color::srgb(0.95, 0.92, 0.82);
use super::text_editor::{CaretStop, EditResult, FriendTextEditor, TextLayout as EditorTextLayout, VisualLine};

pub(crate) const MAIL_LETTER_BODY_RECT: CrystalRect = CrystalRect::new(15.0, 92.0, 202.0, 165.0);
/// `MailComposeParcelDialog` uses the same 202×165 edit viewport six pixels
/// lower than LetterDialog.  Keep layout shaping shared while the source
/// frame owns each control's real local origin.
pub(crate) const MAIL_PARCEL_BODY_RECT: CrystalRect = CrystalRect::new(15.0, 98.0, 202.0, 165.0);
// Crystal's SendMail validation counts .NET string.Length (UTF-16 units).
pub(crate) const MAIL_LETTER_BODY_LIMIT: usize = 500;
pub(crate) const MAIL_LETTER_CONTENT_INSET: Vec2 = Vec2::splat(2.0);
const MAIL_LETTER_BORDER_WIDTH: f32 = 1.0;
pub(crate) const MAIL_LETTER_CONTENT_SIZE: Vec2 = Vec2::new(
    MAIL_LETTER_BODY_RECT.width - MAIL_LETTER_CONTENT_INSET.x * 2.0,
    MAIL_LETTER_BODY_RECT.height - MAIL_LETTER_CONTENT_INSET.y * 2.0,
);

#[derive(Component)] pub struct MailEditorViewport;
#[derive(Component)] pub struct MailEditorCaret;
#[derive(Component)] pub struct MailEditorSelection;
/// Ordering boundary after the canonical editor captures actual shaped UI text.
/// Hosts may publish geometry after this set without calling the private system.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MailEditorLayoutSet;
#[derive(Component)]
pub struct MailLetterEditText {
    pub revision: u64,
    pub text: String,
    pub viewport: [f32; 2],
    pub layout_revision: u64,
    pub focus_generation: u64,
    pub paint_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailComposition {
    value: String,
    cursor: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Default, Resource, PartialEq)]
pub struct MailLetterEditor {
    draft_clock: mir2_client_core::mail_compose::MailDraftClock,
    editor: Option<FriendTextEditor>,
    revision: u64,
    focus_generation: u64,
    paint_revision: u64,
    layout_revision: u64,
    content_size: Option<Vec2>,
    external_generation: Option<u64>,
    revision_exhausted: bool,
    focus_exhausted: bool,
    paint_exhausted: bool,
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
    fn bump_revision(&mut self) {
        self.bump_paint();
        if self.revision_exhausted { return; }
        match self.revision.checked_add(1).filter(|n|*n<=super::mail_compose_shared::SAFE) { Some(n) => self.revision=n, None => {self.revision=0;self.revision_exhausted=true;self.focused=false;} }
    }
    fn bump_selection_revision(&mut self, before: Option<FriendTextEditor>) {
        if before != self.editor { self.bump_revision(); }
    }
    fn bump_paint(&mut self) {
        if self.paint_exhausted{return;}
        match self.paint_revision.checked_add(1).filter(|n|*n<=super::mail_compose_shared::SAFE) {Some(n)=>self.paint_revision=n,None=>{self.paint_revision=0;self.paint_exhausted=true;self.focused=false;}}
    }
    pub fn paint_revision(&self)->u64 {self.paint_revision}
    pub fn focus_generation(&self) -> u64 { self.focus_generation }
    pub fn layout_revision(&self) -> u64 { self.layout_revision }
    pub fn content_size(&self) -> Vec2 { self.content_size.unwrap_or(MAIL_LETTER_CONTENT_SIZE) }
    pub fn mount_external(&mut self,message:&str,generation:u64,body_rect:CrystalRect,layout_revision:u64)->bool {
        if generation==0{return false;}self.bind_external_generation(generation);self.sync(true,Some(message));self.configure_view(body_rect,layout_revision)
    }
    pub fn common_pointer(&mut self,point:Vec2,extend:bool){self.pointer(point,extend);}
    pub fn retire_host(&mut self){self.clear();}
    pub fn layout_ready(&self)->bool {self.editor.as_ref().is_some_and(|e|self.layout_text==e.text()&&self.layout.valid_for(e))}
    pub fn scroll_offset(&self) -> Vec2 { self.scroll }
    /// External Web generation belongs to Core. This does not seed/reset/advance
    /// a second clock; Native continues to bind its accepted Arc clock.
    pub fn bind_external_generation(&mut self, generation: u64) { self.external_generation=Some(generation); }
    pub fn set_focused(&mut self, focused: bool) {
        if self.focus_exhausted||self.revision_exhausted||self.paint_exhausted {self.focused=false;return;}
        if self.focused != focused {
            match self.focus_generation.checked_add(1).filter(|n|*n<=super::mail_compose_shared::SAFE) {Some(n)=>self.focus_generation=n,None=>{self.focus_generation=0;self.focus_exhausted=true;}}
            self.focused = focused && self.focus_generation != 0;
            self.bump_paint();
            self.clear_composition();
            self.selecting=false;
        }
    }
    /// Prompt target switches retire old asynchronous captures even when body
    /// focus stays false. This never changes the accepted content clock.
    pub fn retire_focus_capture(&mut self) {
        if self.focus_exhausted{return;}
        match self.focus_generation.checked_add(1).filter(|n|*n<=super::mail_compose_shared::SAFE) {
            Some(n)=>self.focus_generation=n,
            None=>{self.focus_generation=0;self.focus_exhausted=true;self.focused=false;}
        }
        self.clear_composition();self.selecting=false;self.bump_paint();
    }
    /// Transient prompt raw/limit changes invalidate text captures while the
    /// same body Resource and accepted full-raw content clock remain intact.
    pub fn retire_text_capture(&mut self){self.bump_revision();self.clear_composition();self.selecting=false;}
    /// Host must supply a newer layout revision on resize/font/style change.
    /// Invalidation never changes accepted raw or its complete-draft clock.
    pub fn configure_view(&mut self, body_rect: CrystalRect, revision: u64) -> bool {
        let size=Vec2::new(body_rect.width,body_rect.height)-MAIL_LETTER_CONTENT_INSET*2.;
        if !body_rect.is_valid_hit_target() || !size.is_finite() || size.min_element()<=0.
            || revision==0 || revision<self.layout_revision
            || revision==self.layout_revision && self.content_size!=Some(size) {return false;}
        if revision!=self.layout_revision || self.content_size!=Some(size) {
            self.content_size=Some(size);self.layout_revision=revision;self.bump_paint();
            self.layout=EditorTextLayout::default();self.layout_text.clear();
            self.display_layout=EditorTextLayout::default();self.display_layout_text.clear();
            self.captured_caret=None;self.preferred_x=None;self.scroll=Vec2::ZERO;
        } true
    }
    pub fn set_dom_selection(&mut self, anchor: usize, caret: usize) -> Option<()> {
        let editor=self.editor.as_mut()?;
        let anchor=super::mail_compose_shared::utf16_to_byte(editor.text(),anchor)?;
        let caret=super::mail_compose_shared::utf16_to_byte(editor.text(),caret)?;
        if !editor.is_boundary(anchor)||!editor.is_boundary(caret){return None;}
        let before=editor.clone();editor.set_caret(anchor,false);editor.set_caret(caret,true);
        if before!=*editor {self.bump_revision();}
        self.sync_visual_line_for_caret();self.keep_caret_visible();Some(())
    }
    /// A whole replacement retains every raw byte even when Submit rejects its
    /// normalized UTF16 budget. Interactive paste is explicitly FitPrefix.
    pub fn replace_body_raw(&mut self, draft: &mut mir2_ui_core::state::MailComposeDraft, raw: &str) {
        if draft.message!=raw {
            if self.external_generation.is_none(){self.draft_clock.advance();}
            draft.message=raw.into();self.sync(true,Some(raw));
        }
    }
    pub fn common_key(&mut self,key:&str,control:bool,shift:bool,draft:&mut mir2_ui_core::state::MailComposeDraft)->Option<()> {
        let code=match key {"left"=>KeyCode::ArrowLeft,"right"=>KeyCode::ArrowRight,"up"=>KeyCode::ArrowUp,"down"=>KeyCode::ArrowDown,
            "home"=>KeyCode::Home,"end"=>KeyCode::End,"backspace"=>KeyCode::Backspace,"delete"=>KeyCode::Delete,"enter"=>KeyCode::Enter,
            "selectAll"=>KeyCode::KeyA,_=>return None};
        self.modifiers=[control,false,shift,false];
        self.edit_key(&KeyboardInput{key_code:code,logical_key:bevy::input::keyboard::Key::Character("".into()),state:ButtonState::Pressed,
            text:None,repeat:false,window:Entity::PLACEHOLDER},draft);
        self.modifiers=[false;4];Some(())
    }
    pub(crate) fn bind_draft_clock(&mut self,clock:mir2_client_core::mail_compose::MailDraftClock){self.draft_clock=clock;self.external_generation=None;}
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
        let next=(!value.is_empty()).then_some(MailComposition { value, cursor });
        if self.composition!=next {self.bump_paint();}
        self.composition=next;
    }

    pub fn clear_composition(&mut self) {
        self.modifiers = [false; 4];
        if self.composition.take().is_some(){self.bump_paint();}
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
                super::text_editor::InsertPolicy::FitPrefix,
            ))
            .unwrap_or(EditResult::Unchanged);
        self.commit(draft, result);
        self.sync_visual_line_for_caret();
        self.keep_caret_visible();
    }

    pub fn select_all(&mut self) {
        let before = self.editor.clone();
        if let Some(editor) = self.editor.as_mut() { editor.select_all(); }
        self.bump_selection_revision(before);
    }

    pub fn selected_text(&self) -> &str {
        self.editor.as_ref().map_or("", FriendTextEditor::selected_text)
    }

    pub fn cut_selection(&mut self, draft: &mut mir2_ui_core::state::MailComposeDraft) {
        // Cut deletes only copied selection; Delete retains forward-delete semantics.
        if self.editor.as_ref().is_none_or(|editor| editor.selection().is_empty()) {
            return;
        }
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

    pub(crate) fn sync(&mut self, active: bool, message: Option<&str>) {
        let Some(message) = active.then_some(message).flatten() else {
            self.clear();
            return;
        };
        if self.editor.as_ref().is_none_or(|editor| editor.text() != message) {

            self.editor = Some(FriendTextEditor::new(
                message.to_owned(),
                MAIL_LETTER_BODY_LIMIT,
                true,
            ));
            self.bump_revision();
            self.set_focused(true);
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

    pub(crate) fn clear(&mut self) {
        if self.editor.is_some(){self.bump_revision();}
        self.editor = None;
        self.content_size=None;self.layout_revision=0;
        self.set_focused(false);
        self.layout = EditorTextLayout::default();
        self.layout_text.clear();
        self.visual_line = 0;
        self.captured_caret = None;
        self.preferred_x = None;
        self.scroll = Vec2::ZERO;
        self.modifiers = [false; 4];
        self.selecting = false;
        if self.composition.take().is_some(){self.bump_paint();}
        self.display_layout = EditorTextLayout::default();
        self.display_layout_text.clear();
    }

    fn commit(&mut self, draft: &mut mir2_ui_core::state::MailComposeDraft, result: EditResult) {
        if result == EditResult::Changed {
            if let Some(editor) = self.editor.as_ref() {
                if draft.message != editor.text() {
                    if self.external_generation.is_none() { self.draft_clock.advance(); }
                    draft.message = editor.text().to_owned();
                    self.bump_revision();
                }
            }
            self.preferred_x = None;
        }
    }

    pub(crate) fn edit_key(
        &mut self,
        event: &KeyboardInput,
        draft: &mut mir2_ui_core::state::MailComposeDraft,
    ) {
        let before = self.editor.clone();
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
                        super::text_editor::InsertPolicy::FitPrefix,
                    );
                }
            }
            _ => {}
        }
        self.commit(draft, result);
        if result != EditResult::Changed { self.bump_selection_revision(before); }
        self.sync_visual_line_for_caret();
        self.keep_caret_visible();
    }

    pub(crate) fn pointer(&mut self, point: Vec2, extend: bool) {
        self.set_focused(true);
        let before = self.editor.clone();
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
        self.bump_selection_revision(before);
    }

    pub(crate) fn set_selecting(&mut self, selecting: bool) {
        self.selecting = selecting;
    }

    pub(crate) fn selecting(&self) -> bool {
        self.selecting
    }

    /// Scrolls by source text lines without changing the authoritative caret
    /// or selection. The extent comes from the shaped layout, not a character
    /// count or guessed line height.
    pub(crate) fn scroll_wheel_lines(&mut self, upward_lines: f32) -> bool {
        let Some(height) = self.visible_line_height() else {
            return false;
        };
        self.scroll_wheel_pixels(upward_lines * height)
    }

    /// `upward_pixels` is already in Crystal logical-stage units.
    pub(crate) fn scroll_wheel_pixels(&mut self, upward_pixels: f32) -> bool {
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
        self.bump_paint();
        true
    }

    pub(crate) fn render(&self, parent: &mut ChildSpawnerCommands) {
        self.render_at(parent, MAIL_LETTER_BODY_RECT);
    }

    pub(crate) fn render_at(&self, parent: &mut ChildSpawnerCommands, body_rect: CrystalRect) {
        self.render_with_style(parent, body_rect, crate::crystal_ui::typography::crystal_text_font(crate::crystal_ui::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX), TEXT);
    }
    pub fn render_with_style(&self, parent: &mut ChildSpawnerCommands, body_rect: CrystalRect, font: TextFont, color: Color) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let expected = Vec2::new(body_rect.width, body_rect.height) - MAIL_LETTER_CONTENT_INSET * 2.0;
        if !expected.is_finite() || expected.min_element() <= 0.0 || expected != self.content_size() { return; }
        let empty_caret_height=if self.layout_revision==0 {14.} else {match font.font_size {FontSize::Px(n)=>n,_=>14.}};
        let display = self.displayed_editor().unwrap_or_else(|| editor.clone());
        let scroll = self.scroll;
        // Absolute children start inside the border; keep shaped text and host
        // pointer/IME geometry at the same two-pixel viewport inset.
        let content_origin = MAIL_LETTER_CONTENT_INSET - Vec2::splat(MAIL_LETTER_BORDER_WIDTH);
        parent
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(body_rect.left),
                    top: Val::Px(body_rect.top),
                    width: Val::Px(body_rect.width),
                    height: Val::Px(body_rect.height),
                    border: UiRect::all(Val::Px(MAIL_LETTER_BORDER_WIDTH)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                BorderColor::all(if self.focused {
                    Color::srgb(0.0, 1.0, 0.0)
                } else {
                    Color::NONE
                }),
                MailEditorViewport,
                FocusPolicy::Block,
            ))
            .with_children(|field| {
                if self.composition.is_none() && self.layout_text == editor.text() {
                    for rect in self.layout.selection_rects(editor.selection()) {
                        let Some(rect) = self.visible_paint_rect(rect.x - scroll.x, rect.y - scroll.y, rect.width, rect.height) else { continue; };
                        field.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(content_origin.x + rect.left),
                                top: Val::Px(content_origin.y + rect.top),
                                width: Val::Px(rect.width),
                                height: Val::Px(rect.height),
                                ..default()
                            },
                            MailEditorSelection,
                            BackgroundColor(Color::srgb_u8(35, 85, 145)),
                        ));
                    }
                }
                field.spawn((
                    MailLetterEditText {
                        revision: self.revision,
                        text: display.text().to_owned(),
                        viewport: [self.content_size().x, self.content_size().y],
                        layout_revision: self.layout_revision,
                        focus_generation: self.focus_generation,
                        paint_revision: self.paint_revision,
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(content_origin.x - scroll.x),
                        top: Val::Px(content_origin.y - scroll.y),
                        width: Val::Px(self.content_size().x),
                        min_width: Val::Px(self.content_size().x),
                        ..default()
                    },
                    Text::new(display.text()),
                    font.clone(),
                    TextColor(color),
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
                        .or_else(|| editor.text().is_empty().then_some((0.0, 0.0, empty_caret_height)));
                    // Shaped line boxes may have negative leading. Paint only
                    // their visible fragment; glyph/layout coordinates stay intact.
                    if let Some(rect) = caret.and_then(|(x, y, height)| {
                        let width = 1.0_f32.min(self.content_size().x);
                        let x = (x - scroll.x).clamp(0.0, (self.content_size().x - width).max(0.0));
                        self.visible_paint_rect(x, y - scroll.y, width, height)
                    }) {
                        field.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(content_origin.x + rect.left),
                                top: Val::Px(content_origin.y + rect.top),
                                width: Val::Px(rect.width),
                                height: Val::Px(rect.height),
                                ..default()
                            },
                            MailEditorCaret,
                            BackgroundColor(Color::WHITE),
                        ));
                    }
                }
            });
    }

    fn visible_paint_rect(&self, x: f32, y: f32, width: f32, height: f32) -> Option<CrystalRect> {
        if ![x, y, width, height].into_iter().all(f32::is_finite) { return None; }
        let size = self.content_size();
        let left = x.max(0.0);
        let top = y.max(0.0);
        let right = (x + width).min(size.x);
        let bottom = (y + height).min(size.y);
        (right > left && bottom > top).then_some(CrystalRect::new(left, top, right - left, bottom - top))
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
        if line.y + line.height > self.scroll.y + self.content_size().y {
            self.scroll.y = line.y + line.height - self.content_size().y;
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
        Some((bottom - self.content_size().y).max(0.0))
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

    pub(crate) fn capture_layout(
        &mut self,
        tag: &MailLetterEditText,
        block: &bevy::text::ComputedTextBlock,
        info: &bevy::text::TextLayoutInfo,
    ) {
        if tag.revision != self.revision || tag.layout_revision != self.layout_revision || tag.focus_generation != self.focus_generation || tag.paint_revision != self.paint_revision
            || tag.viewport != [self.content_size().x, self.content_size().y] {
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
        if self.layout!=layout||self.layout_text!=text {self.bump_paint();}
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
        if self.display_layout!=layout||self.display_layout_text!=text {self.bump_paint();}
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
        if line.y + line.height > self.scroll.y + self.content_size().y {
            self.scroll.y = line.y + line.height - self.content_size().y;
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
            .clamp(0.0, (bottom - self.content_size().y).max(0.0));
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
    pub(crate) fn install_layout(&mut self, lines: Vec<VisualLine>) {
        let text = self.editor.as_ref().map_or_else(String::new, |editor| editor.text().to_owned());
        // Test fixtures install already-shaped layout without reproducing the
        // renderer's initial caret-visibility transition.
        self.layout = EditorTextLayout { lines };
        self.layout_text = text;
        self.captured_caret = self.editor.as_ref().map(FriendTextEditor::caret);
    }

    #[cfg(test)]
    pub(crate) fn scroll(&self) -> Vec2 {
        self.scroll
    }
}

pub(crate) fn capture_layout_system(
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

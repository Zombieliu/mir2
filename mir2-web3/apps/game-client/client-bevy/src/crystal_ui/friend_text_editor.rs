//! Renderer-independent friend input editor. Offsets are UTF-8 bytes; limits are
//! UTF-16 code units (Crystal/.NET string.Length). Layout must come from the font
//! renderer, including its actual wrapping, rather than estimated character widths.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriendTextEditor {
    text: String,
    caret: usize,
    anchor: usize,
    limit: usize,
    multiline: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditResult {
    Unchanged,
    Changed,
    OverBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertPolicy {
    /// Explicit application validation; retain the entire draft on overflow.
    RejectOverflow,
    /// WinForms MirInputBox.MaxLength: accept the fitting prefix of input.
    /// Never manufacture an invalid surrogate or split a Unicode grapheme.
    FitPrefix,
}

impl FriendTextEditor {
    pub fn insert_with_policy(&mut self, input: &str, policy: InsertPolicy) -> EditResult {
        if policy == InsertPolicy::RejectOverflow {
            return self.insert(input);
        }
        let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
        let clean: String = normalized
            .chars()
            .filter(|c| !c.is_control() || (*c == '\n' && self.multiline))
            .collect();
        let available = self
            .limit
            .saturating_sub(self.utf16_len() - self.selected_text().encode_utf16().count());
        let mut count = 0;
        let mut end = 0;
        for (index, g) in clean.grapheme_indices(true) {
            let units = g.encode_utf16().count();
            if count + units > available {
                break;
            }
            count += units;
            end = index + g.len();
        }
        self.insert(&clean[..end])
    }
    /// Existing authoritative drafts are preserved even if they exceed the limit.
    pub fn new(text: String, limit: usize, multiline: bool) -> Self {
        let caret = text.len();
        Self {
            text,
            caret,
            anchor: caret,
            limit,
            multiline,
        }
    }
    /// Preview composition without committing it or changing the authoritative selection.
    pub fn composition_preview(&self, value: &str, cursor: Option<(usize, usize)>) -> Self {
        let mut preview = self.clone();
        let selection = self.selection();
        preview.text.replace_range(selection.clone(), value);
        let (start, end) = cursor.unwrap_or((value.len(), value.len()));
        let boundary = |offset: usize| {
            let mut n = offset.min(value.len());
            while !value.is_char_boundary(n) {
                n -= 1;
            }
            n
        };
        preview.set_caret(selection.start + boundary(start), false);
        preview.set_caret(selection.start + boundary(end), true);
        preview
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn caret(&self) -> usize {
        self.caret
    }
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }
    pub fn utf16_len(&self) -> usize {
        self.text.encode_utf16().count()
    }
    pub fn within_budget(&self) -> bool {
        self.utf16_len() <= self.limit
    }
    fn boundaries(&self) -> impl Iterator<Item = usize> + '_ {
        self.text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(self.text.len()))
    }
    pub fn is_boundary(&self, byte: usize) -> bool {
        self.boundaries().any(|i| i == byte)
    }
    pub fn set_caret(&mut self, byte: usize, extend: bool) {
        self.caret = self
            .boundaries()
            .take_while(|i| *i <= byte)
            .last()
            .unwrap_or(0);
        if !extend {
            self.anchor = self.caret;
        }
    }
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
    }
    /// Atomic replacement: excess input preserves the entire draft and selection.
    /// Caller must display OverBudget and retain the modal; never close it on rejection.
    pub fn insert(&mut self, input: &str) -> EditResult {
        let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
        let clean: String = normalized
            .chars()
            .filter(|c| !c.is_control() || (*c == '\n' && self.multiline))
            .collect();
        if clean.is_empty() {
            return EditResult::Unchanged;
        }
        let range = self.selection();
        let next_len = self.utf16_len() - self.selected_text().encode_utf16().count()
            + clean.encode_utf16().count();
        if next_len > self.limit {
            return EditResult::OverBudget;
        }
        self.text.replace_range(range.clone(), &clean);
        // Insertion can join a combining sequence with its following grapheme.
        let desired = range.start + clean.len();
        let caret = self
            .boundaries()
            .find(|i| *i >= desired)
            .unwrap_or(self.text.len());
        self.caret = caret;
        self.anchor = self.caret;
        EditResult::Changed
    }
    pub fn newline(&mut self) -> EditResult {
        self.insert("\n")
    }
    pub fn delete(&mut self, backwards: bool) -> EditResult {
        let mut range = self.selection();
        if range.is_empty() {
            if backwards {
                range.start = self
                    .boundaries()
                    .take_while(|i| *i < self.caret)
                    .last()
                    .unwrap_or(0);
            } else {
                range.end = self
                    .boundaries()
                    .find(|i| *i > self.caret)
                    .unwrap_or(self.text.len());
            }
        }
        if range.is_empty() {
            return EditResult::Unchanged;
        }
        self.text.replace_range(range.clone(), "");
        self.set_caret(range.start, false);
        EditResult::Changed
    }
    pub fn horizontal(&mut self, right: bool, extend: bool) {
        let selection = self.selection();
        let target = if !extend && !selection.is_empty() {
            if right {
                selection.end
            } else {
                selection.start
            }
        } else if right {
            self.boundaries()
                .find(|i| *i > self.caret)
                .unwrap_or(self.text.len())
        } else {
            self.boundaries()
                .take_while(|i| *i < self.caret)
                .last()
                .unwrap_or(0)
        };
        self.set_caret(target, extend);
    }
    /// Logical line Home/End; use Layout::line_edge for wrapped visual lines.
    pub fn home_end(&mut self, end: bool, document: bool, extend: bool) {
        let target = if document {
            if end {
                self.text.len()
            } else {
                0
            }
        } else if end {
            self.text[self.caret..]
                .find('\n')
                .map(|i| self.caret + i)
                .unwrap_or(self.text.len())
        } else {
            self.text[..self.caret]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0)
        };
        self.set_caret(target, extend);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretStop {
    pub byte: usize,
    pub x: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct VisualLine {
    pub y: f32,
    pub height: f32,
    pub stops: Vec<CaretStop>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    pub lines: Vec<VisualLine>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl TextLayout {
    /// Reject stale byte offsets and non-finite geometry before hit testing.
    pub fn valid_for(&self, editor: &FriendTextEditor) -> bool {
        !self.lines.is_empty()
            && self.lines.iter().all(|l| {
                l.y.is_finite()
                    && l.height.is_finite()
                    && l.height > 0.
                    && !l.stops.is_empty()
                    && l.stops
                        .iter()
                        .all(|s| s.x.is_finite() && editor.is_boundary(s.byte))
            })
    }
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let line = self.lines.iter().min_by(|a, b| {
            let distance = |l: &VisualLine| (l.y - y).max(0.).max(y - l.y - l.height);
            distance(a).total_cmp(&distance(b))
        })?;
        line.stops
            .iter()
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .map(|s| s.byte)
    }
    /// At wrap boundaries the caller supplies the visual line chosen by pointer
    /// or caret affinity, since a single byte offset can occupy two visual lines.
    pub fn line_edge(&self, line: usize, end: bool) -> Option<usize> {
        let stops = &self.lines.get(line)?.stops;
        if end { stops.last() } else { stops.first() }.map(|s| s.byte)
    }
    pub fn vertical(&self, line: usize, down: bool, preferred_x: f32) -> Option<(usize, usize)> {
        let target = if down {
            (line + 1).min(self.lines.len().checked_sub(1)?)
        } else {
            line.saturating_sub(1)
        };
        let stop = self.lines.get(target)?.stops.iter().min_by(|a, b| {
            (a.x - preferred_x)
                .abs()
                .total_cmp(&(b.x - preferred_x).abs())
        })?;
        Some((target, stop.byte))
    }
    pub fn selection_rects(&self, selection: Range<usize>) -> Vec<SelectionRect> {
        let mut out = Vec::new();
        for line in &self.lines {
            // Consecutive visual stops support shaped variable-width and RTL runs.
            for pair in line.stops.windows(2) {
                let start = pair[0].byte.min(pair[1].byte);
                let end = pair[0].byte.max(pair[1].byte);
                if start < selection.end && end > selection.start && start != end {
                    out.push(SelectionRect {
                        x: pair[0].x.min(pair[1].x),
                        y: line.y,
                        width: (pair[1].x - pair[0].x).abs(),
                        height: line.height,
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "friend_text_editor_tests.rs"]
mod tests;

//! Crystal FriendDialog.cs and MemoDialog: selection is an authoritative character
//! identity. Modal targets never follow a subsequently selected row.
use mir2_protocol::{ClientFriend, ClientPacket, ServerPacket};
#[path = "friend_text_editor.rs"]
pub mod text_editor;
use text_editor::{EditResult, FriendTextEditor};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorTarget {
    Add(bool),
    Memo(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendAction {
    Close,
    Friends,
    Blocked,
    Previous,
    Next,
    Select(usize),
    Add,
    Remove,
    Memo,
    Mail,
    Whisper,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FriendModal {
    Add { blocked: bool, text: String },
    Remove { character_index: i32, name: String },
    Memo { character_index: i32, text: String },
}

#[derive(Debug, PartialEq)]
pub enum FriendEffect {
    Packet(ClientPacket),
    ComposeMail(String),
    Whisper(String),
    Offline,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct FriendDialogUi {
    pub open: bool,
    pub input_consumed: bool,
    pub blocked: bool,
    pub page: usize,
    pub start_index: usize,
    pub selected: Option<i32>,
    pub friends: Vec<ClientFriend>,
    pub modal: Option<FriendModal>,
    pub position: Option<[f32; 2]>,
    pub memo_position: Option<[f32; 2]>,
    pub editor: Option<FriendTextEditor>,
    pub editor_target: Option<EditorTarget>,
    pub editor_revision: u64,
    pub editor_focused: bool,
    pub composition: Option<super::text_input::Composition>,
    pub edit_notice: Option<String>,
    pub text_layout: text_editor::TextLayout,
    pub layout_text: String,
    pub visual_line: usize,
    pub captured_caret: Option<usize>,
    pub preferred_x: Option<f32>,
    pub text_scroll: [f32; 2],
    pub modifiers: [bool; 4],
}

impl FriendDialogUi {
    pub fn show(&mut self) -> Option<FriendEffect> {
        if self.open {
            return None;
        }
        self.open = true;
        Some(FriendEffect::Packet(ClientPacket::RefreshFriends))
    }
    pub fn hide(&mut self) {
        self.open = false;
        self.cancel_modal();
    }
    pub fn clear_session(&mut self) {
        *self = Self::default();
    }
    pub fn apply_packet(&mut self, packet: &ServerPacket) -> bool {
        let ServerPacket::FriendUpdate { friends } = packet else {
            return false;
        };
        self.friends = friends.clone();
        self.selected = None; // Original Update defaults to clearSelection=true.
        self.clamp_start();
        if let Some(
            FriendModal::Memo {
                character_index, ..
            }
            | FriendModal::Remove {
                character_index, ..
            },
        ) = &self.modal
        {
            if !self.friends.iter().any(|f| f.index == *character_index) {
                self.cancel_modal();
            }
        }
        true
    }
    pub fn filtered_count(&self) -> usize {
        self.friends
            .iter()
            .filter(|f| f.blocked == self.blocked)
            .count()
    }
    /// Matches the original displayed count (including its extra page at exactly 12).
    pub fn page_count(&self) -> usize {
        self.filtered_count() / 12 + 1
    }
    pub fn rows(&self) -> impl Iterator<Item = &ClientFriend> {
        self.friends
            .iter()
            .filter(|f| f.blocked == self.blocked)
            .skip(self.start_index)
            .take(12)
    }
    fn clamp_start(&mut self) {
        self.start_index = self
            .start_index
            .min(self.filtered_count().saturating_sub(1));
    }
    pub fn selected_friend(&self) -> Option<&ClientFriend> {
        self.friends
            .iter()
            .find(|f| Some(f.index) == self.selected && f.blocked == self.blocked)
    }
    pub fn action(&mut self, action: FriendAction) -> Option<FriendEffect> {
        if !self.open || self.modal.is_some() {
            return None;
        }
        match action {
            FriendAction::Close => self.hide(),
            FriendAction::Friends | FriendAction::Blocked => {
                self.blocked = action == FriendAction::Blocked;
                self.selected = None;
                self.clamp_start();
            }
            FriendAction::Previous | FriendAction::Next => {
                self.page = if action == FriendAction::Previous {
                    self.page.saturating_sub(1)
                } else {
                    (self.page + 1).min(self.friends.len() / 12)
                };
                self.start_index = self.page * 12;
                self.clamp_start();
                self.selected = None;
            }
            FriendAction::Select(row) => {
                let selected = self.rows().nth(row).map(|f| f.index);
                self.selected = selected;
            }
            FriendAction::Add => {
                self.modal = Some(FriendModal::Add {
                    blocked: self.blocked,
                    text: String::new(),
                })
            }
            FriendAction::Remove => {
                let f = self.selected_friend()?;
                self.modal = Some(FriendModal::Remove {
                    character_index: f.index,
                    name: f.name.clone(),
                });
            }
            FriendAction::Memo => {
                let f = self.selected_friend()?;
                self.modal = Some(FriendModal::Memo {
                    character_index: f.index,
                    text: f.memo.clone(),
                });
            }
            FriendAction::Mail => {
                return Some(FriendEffect::ComposeMail(
                    self.selected_friend()?.name.clone(),
                ))
            }
            FriendAction::Whisper => {
                let f = self.selected_friend()?;
                return Some(if f.online {
                    FriendEffect::Whisper(format!("/{} ", f.name))
                } else {
                    FriendEffect::Offline
                });
            }
        }
        None
    }
    /// Keep caret and selection across renders; only external draft/target changes rebuild.
    pub fn display_editor(&self) -> Option<FriendTextEditor> {
        self.editor.as_ref().map(|editor| {
            self.composition
                .as_ref()
                .map(|c| editor.composition_preview(&c.value, c.cursor))
                .unwrap_or_else(|| editor.clone())
        })
    }
    pub fn sync_editor(&mut self) {
        let draft = match &self.modal {
            Some(FriendModal::Add { blocked, text }) => {
                Some((EditorTarget::Add(*blocked), text.clone(), 50, false))
            }
            Some(FriendModal::Memo {
                character_index,
                text,
            }) => Some((
                EditorTarget::Memo(*character_index),
                text.clone(),
                32_767, // WinForms TextBox default MaxLength; server submission limit is separate.
                true,
            )),
            _ => None,
        };
        let Some((target, text, limit, multiline)) = draft else {
            self.composition = None;
            self.editor = None;
            self.editor_target = None;
            self.editor_focused = false;
            return;
        };
        if self.editor_target.as_ref() != Some(&target)
            || self.editor.as_ref().is_none_or(|e| e.text() != text)
        {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            self.composition = None;
            self.editor_revision = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.editor = Some(FriendTextEditor::new(text, limit, multiline));
            self.editor_target = Some(target);
            self.editor_focused = true;
            self.text_layout = Default::default();
            self.layout_text.clear();
            self.text_scroll = [0.; 2];
            self.visual_line = 0;
            self.preferred_x = None;
            self.edit_notice = None;
        }
    }
    pub fn commit_editor(&mut self, result: EditResult) {
        if result == EditResult::OverBudget {
            self.edit_notice = Some(
                if matches!(self.modal, Some(FriendModal::Memo { .. })) {
                    "Memo editor cannot exceed 32767 characters."
                } else {
                    "Name cannot exceed 50 characters."
                }
                .into(),
            );
            return;
        }
        if result == EditResult::Changed {
            let text = self.editor.as_ref().map(|e| e.text().to_owned());
            if let (Some(text), Some(draft)) = (text, self.modal_text_mut()) {
                *draft = text;
            }
            self.edit_notice = None;
            self.preferred_x = None;
        }
    }
    pub fn paste(&mut self, text: &str) -> EditResult {
        self.sync_editor();
        if !self.editor_focused {
            return EditResult::Unchanged;
        }
        let result = self
            .editor
            .as_mut()
            .map(|e| e.insert_with_policy(text, text_editor::InsertPolicy::FitPrefix))
            .unwrap_or(EditResult::Unchanged);
        self.commit_editor(result);
        result
    }
    pub fn modal_text_mut(&mut self) -> Option<&mut String> {
        match self.modal.as_mut()? {
            FriendModal::Add { text, .. } | FriendModal::Memo { text, .. } => Some(text),
            _ => None,
        }
    }
    /// Take only after transport accepts; failed sends retain the user's draft.
    pub fn modal_packet(&self) -> Option<ClientPacket> {
        match self.modal.as_ref()? {
            FriendModal::Add { blocked, text }
                if !text.is_empty() && text.encode_utf16().count() <= 50 =>
            {
                Some(ClientPacket::AddFriend {
                    name: text.clone(),
                    blocked: *blocked,
                })
            }
            FriendModal::Remove {
                character_index, ..
            } => self
                .friends
                .iter()
                .any(|f| f.index == *character_index)
                .then_some(ClientPacket::RemoveFriend {
                    character_index: *character_index,
                }),
            FriendModal::Memo {
                character_index,
                text,
            } => self
                .friends
                .iter()
                .any(|f| {
                    f.index == *character_index
                        && !text.is_empty()
                        && text.encode_utf16().count() <= 200
                })
                .then(|| ClientPacket::AddMemo {
                    character_index: *character_index,
                    memo: text.clone(),
                }),
            _ => None,
        }
    }
    pub fn cancel_modal(&mut self) {
        self.modal = None;
        self.editor = None;
        self.editor_target = None;
        self.editor_focused = false;
        self.edit_notice = None;
        self.modifiers = [false; 4];
    }
    pub fn restore_unsent(&mut self, packet: &ClientPacket) {
        if !self.open || self.modal.is_some() {
            return;
        }
        self.modal = match packet {
            ClientPacket::AddFriend { name, blocked } => Some(FriendModal::Add {
                blocked: *blocked,
                text: name.clone(),
            }),
            ClientPacket::AddMemo {
                character_index,
                memo,
            } if self.friends.iter().any(|f| f.index == *character_index) => {
                Some(FriendModal::Memo {
                    character_index: *character_index,
                    text: memo.clone(),
                })
            }
            ClientPacket::RemoveFriend { character_index } => self
                .friends
                .iter()
                .find(|f| f.index == *character_index)
                .map(|f| FriendModal::Remove {
                    character_index: *character_index,
                    name: f.name.clone(),
                }),
            _ => None,
        };
    }
}

#[cfg(feature = "native-ui")]
#[path = "friend_dialog_host.rs"]
pub mod host;
#[cfg(test)]
#[path = "friend_dialog_tests.rs"]
mod tests;
#[cfg(feature = "native-ui")]
#[path = "friend_dialog_render.rs"]
pub mod view;

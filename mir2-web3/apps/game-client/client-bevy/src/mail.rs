//! Authoritative mail model shared by every host.

//! Mirrors the simulation's `Stage5MailMessage` / `ClientMail` so the native
//! Windows Mail panel shows the same inbox as the browser. UI selection is kept
//! alongside the authoritative list but is not overwritten by the server.

use bevy::prelude::Resource;
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_MAIL_ATTACHMENTS: usize = 5;
pub const MAX_MAIL_MESSAGES: usize = 256;
pub const MAIL_PAGE_SIZE: usize = 10;

fn deserialize_bounded<'de, D, T, const MAX: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: de::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVisitor<T, const MAX: usize>(std::marker::PhantomData<T>);

    impl<'de, T, const MAX: usize> Visitor<'de> for BoundedVisitor<T, MAX>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "a sequence with at most {MAX} retained entries")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::with_capacity(MAX.min(sequence.size_hint().unwrap_or(0)));
            while let Some(value) = sequence.next_element::<T>()? {
                if values.len() < MAX {
                    values.push(value);
                }
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVisitor::<T, MAX>(std::marker::PhantomData))
}

fn deserialize_mail_attachments<'de, D>(deserializer: D) -> Result<Vec<MailAttachment>, D::Error>
where
    D: de::Deserializer<'de>,
{
    deserialize_bounded::<D, MailAttachment, MAX_MAIL_ATTACHMENTS>(deserializer)
}

fn deserialize_mail_messages<'de, D>(deserializer: D) -> Result<Vec<MailMessage>, D::Error>
where
    D: de::Deserializer<'de>,
{
    deserialize_bounded::<D, MailMessage, MAX_MAIL_MESSAGES>(deserializer)
}

/// A server-owned mail attachment. `unique_id` is optional only for legacy
/// Stage5 snapshots that contain display names instead of a wire UserItem;
/// packet-originated ClientMail keeps the concrete id and metadata intact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MailAttachment {
    pub unique_id: Option<u64>,
    pub item_index: Option<i32>,
    pub key: Option<String>,
    pub name: Option<String>,
    pub count: u16,
    pub current_dura: u16,
    pub max_dura: u16,
    pub soul_bound_id: i32,
    pub identified: bool,
    pub cursed: bool,
    pub gem_count: u16,
}

impl MailAttachment {
    pub fn label(&self) -> String {
        self.name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .or(self.key.as_deref().filter(|key| !key.trim().is_empty()))
            .map(str::to_owned)
            .or_else(|| self.item_index.map(|index| format!("Item #{index}")))
            .unwrap_or_else(|| "Item".to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MailOperationKind {
    Send,
    Read,
    Collect,
    Delete,
}

/// One-shot result transported through the existing mail model channel. The
/// runtime keeps this transient row long enough for pending reconciliation;
/// the overlay consumes and hides it from the inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailOperationFeedback {
    pub kind: MailOperationKind,
    pub success: bool,
    pub mail_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MailMessage {
    #[serde(alias = "mailId")]
    pub id: u64,
    #[serde(alias = "from", alias = "senderName")]
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub gold: u32,
    #[serde(default, deserialize_with = "deserialize_mail_attachments")]
    pub items: Vec<MailAttachment>,
    #[serde(default)]
    pub operation: Option<MailOperationFeedback>,
    pub claimed: bool,
    pub locked: bool,
    #[serde(alias = "opened")]
    pub read: bool,
}

impl MailMessage {
    pub fn has_attachment(&self) -> bool {
        self.gold > 0 || !self.items.is_empty()
    }

    pub fn attachment_summary(&self) -> String {
        let mut parts = Vec::new();
        if self.gold > 0 {
            parts.push(format!("{} Gold", self.gold));
        }
        if !self.items.is_empty() {
            parts.push(
                self.items
                    .iter()
                    .map(MailAttachment::label)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if parts.is_empty() {
            String::new()
        } else {
            parts.join(" · ")
        }
    }
}

#[derive(Debug, Clone, Default, Resource, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MailModel {
    #[serde(default, deserialize_with = "deserialize_mail_messages")]
    pub mails: Vec<MailMessage>,
    pub selected_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MailPageCursor {
    pub page: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailPage<'a> {
    pub page: usize,
    pub page_count: usize,
    pub entries: Vec<&'a MailMessage>,
}

impl MailModel {
    pub fn selected(&self) -> Option<&MailMessage> {
        self.selected_id.and_then(|id| {
            self.mails
                .iter()
                .find(|m| m.id == id && m.operation.is_none())
        })
    }

    pub fn unread_count(&self) -> usize {
        self.mails.iter().filter(|m| !m.read).count()
    }

    pub fn visible_mails(&self) -> Vec<&MailMessage> {
        self.mails
            .iter()
            .filter(|mail| mail.operation.is_none())
            .collect()
    }

    pub fn operation_feedback(&self) -> Option<&MailOperationFeedback> {
        self.mails.iter().find_map(|mail| mail.operation.as_ref())
    }

    /// Clamp local pagination and selection after an authoritative refresh.
    /// A delete is not guessed locally: the next server snapshot determines
    /// which rows remain, while this method prevents stale UI state.
    pub fn clamp_after_refresh(&mut self, cursor: &mut MailPageCursor) {
        self.selected_id = self.selected_id.filter(|id| {
            self.mails
                .iter()
                .any(|mail| mail.id == *id && mail.operation.is_none())
        });
        cursor.page = self.clamp_page(cursor.page);
    }

    pub fn page_count(&self) -> usize {
        self.page_count_for(MAIL_PAGE_SIZE)
    }

    pub fn page_count_for(&self, page_size: usize) -> usize {
        let page_size = page_size.max(1);
        self.visible_mails().len().div_ceil(page_size).max(1)
    }

    pub fn clamp_page(&self, page: usize) -> usize {
        page.min(self.page_count().saturating_sub(1))
    }

    pub fn page(&self, page: usize) -> MailPage<'_> {
        self.page_with_size(page, MAIL_PAGE_SIZE)
    }

    pub fn page_with_size(&self, page: usize, page_size: usize) -> MailPage<'_> {
        let page_size = page_size.max(1);
        let page_count = self.page_count_for(page_size);
        let page = page.min(page_count.saturating_sub(1));
        let start = page.saturating_mul(page_size);
        let entries = self
            .visible_mails()
            .into_iter()
            .skip(start)
            .take(page_size)
            .collect();
        MailPage {
            page,
            page_count,
            entries,
        }
    }

    /// Selection is identity-based. Unknown or operation-only rows are
    /// rejected so a stale pressed row cannot address a different message.
    pub fn select_visible(&mut self, id: u64) -> bool {
        if self.visible_mails().iter().any(|mail| mail.id == id) {
            self.selected_id = Some(id);
            true
        } else {
            self.selected_id = None;
            false
        }
    }
}

pub fn mail_attachment_label(item: &MailAttachment) -> String {
    item.label()
}

pub fn mail_claim_enabled(msg: &MailMessage) -> bool {
    !msg.claimed && !msg.locked && msg.has_attachment()
}

pub fn mail_delete_enabled(msg: &MailMessage) -> bool {
    !msg.locked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: u64, claimed: bool, locked: bool, gold: u32) -> MailMessage {
        MailMessage {
            id,
            sender: "System".to_owned(),
            subject: format!("Test {}", id),
            body: "Hello".to_owned(),
            gold,
            items: if gold > 0 {
                vec![MailAttachment {
                    name: Some("Gold".to_owned()),
                    ..Default::default()
                }]
            } else {
                vec![]
            },
            operation: None,
            claimed,
            locked,
            read: false,
        }
    }

    #[test]
    fn mail_claim_and_delete_rules() {
        let unclaimed = msg(1, false, false, 100);
        let claimed = msg(2, true, false, 100);
        let locked = msg(3, false, true, 100);
        let no_attach = msg(4, false, false, 0);
        assert!(mail_claim_enabled(&unclaimed));
        assert!(!mail_claim_enabled(&claimed));
        assert!(!mail_claim_enabled(&locked));
        assert!(!mail_claim_enabled(&no_attach));
        assert!(mail_delete_enabled(&unclaimed));
        assert!(!mail_delete_enabled(&locked));
    }

    #[test]
    fn serde_roundtrip() {
        let model = MailModel {
            mails: vec![msg(1, false, false, 10)],
            selected_id: Some(1),
        };
        let json = serde_json::to_string(&model).expect("serialize");
        let restored: MailModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(model, restored);
    }

    #[test]
    fn user_item_metadata_roundtrips_without_string_loss() {
        let attachment = MailAttachment {
            unique_id: Some(77),
            item_index: Some(1001),
            count: 2,
            current_dura: 8,
            max_dura: 10,
            soul_bound_id: 3,
            identified: true,
            cursed: true,
            gem_count: 1,
            ..Default::default()
        };
        let json = serde_json::to_string(&attachment).expect("serialize");
        let restored: MailAttachment = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, attachment);
    }

    #[test]
    fn mail_pages_are_ten_rows_and_clamp_after_refresh_delete() {
        let mut model = MailModel {
            mails: (0..21).map(|id| msg(id, false, false, 0)).collect(),
            selected_id: Some(20),
        };
        let mut cursor = MailPageCursor { page: 2 };
        assert_eq!(model.page_count(), 3);
        assert_eq!(model.page(2).entries.len(), 1);

        model.mails.retain(|mail| mail.id != 20);
        model.clamp_after_refresh(&mut cursor);
        assert_eq!(cursor.page, 1);
        assert_eq!(model.selected_id, None);
        assert_eq!(model.page(cursor.page).entries.len(), 10);
    }

    #[test]
    fn mail_deserialization_is_backward_compatible_and_bounded() {
        let oversized = serde_json::json!({
            "mails": (0..(MAX_MAIL_MESSAGES + 7)).map(|id| serde_json::json!({
                "mailId": id,
                "from": "System",
                "subject": "Subject",
                "body": "Body",
                "gold": 0,
                "items": (0..(MAX_MAIL_ATTACHMENTS + 3)).map(|_| serde_json::json!({"name":"Potion"})).collect::<Vec<_>>(),
                "claimed": false,
                "locked": false,
                "opened": false
            })).collect::<Vec<_>>()
        });
        let model: MailModel = serde_json::from_value(oversized).expect("legacy mail");
        assert_eq!(model.mails.len(), MAX_MAIL_MESSAGES);
        assert_eq!(model.mails[0].items.len(), MAX_MAIL_ATTACHMENTS);
        assert_eq!(model.mails[0].sender, "System");
    }

    #[test]
    fn stale_mail_selection_fails_closed() {
        let mut model = MailModel {
            mails: vec![msg(1, false, false, 0)],
            selected_id: Some(99),
        };
        assert!(!model.select_visible(99));
        assert_eq!(model.selected_id, None);
        assert!(model.select_visible(1));
        assert_eq!(model.selected_id, Some(1));
    }
}

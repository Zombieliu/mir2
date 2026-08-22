//! Authoritative mail model shared by every host.

//! Mirrors the simulation's `Stage5MailMessage` / `ClientMail` so the native
//! Windows Mail panel shows the same inbox as the browser. UI selection is kept
//! alongside the authoritative list but is not overwritten by the server.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const MAX_MAIL_ATTACHMENTS: usize = 5;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailMessage {
    pub id: u64,
    pub sender: String,
    pub subject: String,
    pub body: String,
    pub gold: u32,
    pub items: Vec<MailAttachment>,
    #[serde(default)]
    pub operation: Option<MailOperationFeedback>,
    pub claimed: bool,
    pub locked: bool,
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
    pub mails: Vec<MailMessage>,
    pub selected_id: Option<u64>,
}

impl MailModel {
    pub fn selected(&self) -> Option<&MailMessage> {
        self.selected_id
            .and_then(|id| self.mails.iter().find(|m| m.id == id))
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
}

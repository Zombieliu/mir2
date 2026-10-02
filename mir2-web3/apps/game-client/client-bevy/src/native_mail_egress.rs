//! Mail-only shared UI projection of frozen Windows 3d735745f.
//! No authentication, postage, eligibility, attachment custody, or settlement rules.
use crate::{crystal_ui::overlays::NativePlayerUiIntent, inventory::InventoryModel};
use serde::Serialize;

/// Closed public BrowserCommand subset, serialized losslessly before host I/O.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NativeMailCommand {
    ReadMail {
        #[serde(rename = "mailId")]
        mail_id: u64,
    },
    LockMail {
        #[serde(rename = "mailId")]
        mail_id: u64,
        lock: bool,
    },
    CollectParcel {
        #[serde(rename = "mailId")]
        mail_id: u64,
    },
    DeleteMail {
        #[serde(rename = "mailId")]
        mail_id: u64,
    },
    MailCost {
        gold: u32,
        #[serde(rename = "itemsIdx")]
        items_idx: [u64; 5],
        stamped: bool,
    },
    MailLockedItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        locked: bool,
    },
    SendMail {
        name: String,
        message: String,
        gold: u32,
        #[serde(rename = "itemsIdx")]
        items_idx: [u64; 5],
        stamped: bool,
    },
}

impl NativeMailCommand {
    pub fn command_type(&self) -> &'static str {
        match self {
            Self::ReadMail { .. } => "readMail",
            Self::LockMail { .. } => "lockMail",
            Self::CollectParcel { .. } => "collectParcel",
            Self::DeleteMail { .. } => "deleteMail",
            Self::MailCost { .. } => "mailCost",
            Self::MailLockedItem { .. } => "mailLockedItem",
            Self::SendMail { .. } => "sendMail",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeMailProjectionError {
    NotMail,
    MissingInventory,
    InvalidAttachments,
}

pub fn is_native_mail_intent(intent: &NativePlayerUiIntent) -> bool {
    matches!(
        intent,
        NativePlayerUiIntent::ReadMail { .. }
            | NativePlayerUiIntent::ClaimMail { .. }
            | NativePlayerUiIntent::DeleteMail { .. }
            | NativePlayerUiIntent::LockMail { .. }
            | NativePlayerUiIntent::MailCost { .. }
            | NativePlayerUiIntent::MailLockItem { .. }
            | NativePlayerUiIntent::SendMail { .. }
    )
}

/// Uses the current authoritative inventory, never inferred capacity or slot IDs.
pub fn project_native_mail_intent(
    intent: &NativePlayerUiIntent,
    inventory: Option<&InventoryModel>,
) -> Result<NativeMailCommand, NativeMailProjectionError> {
    use NativeMailProjectionError::{InvalidAttachments, MissingInventory, NotMail};
    let attachments = |ids: &[u64]| {
        mail_attachment_indices(inventory.ok_or(MissingInventory)?, ids).ok_or(InvalidAttachments)
    };
    Ok(match intent {
        NativePlayerUiIntent::ReadMail { mail_id } => {
            NativeMailCommand::ReadMail { mail_id: *mail_id }
        }
        NativePlayerUiIntent::LockMail { mail_id, lock } => NativeMailCommand::LockMail {
            mail_id: *mail_id,
            lock: *lock,
        },
        NativePlayerUiIntent::ClaimMail { mail_id } => {
            NativeMailCommand::CollectParcel { mail_id: *mail_id }
        }
        NativePlayerUiIntent::DeleteMail { mail_id } => {
            NativeMailCommand::DeleteMail { mail_id: *mail_id }
        }
        NativePlayerUiIntent::MailCost {
            gold,
            attachment_unique_ids,
            stamped,
        } => NativeMailCommand::MailCost {
            gold: *gold,
            items_idx: attachments(attachment_unique_ids)?,
            stamped: *stamped,
        },
        NativePlayerUiIntent::MailLockItem { unique_id, locked } => {
            attachments(&[*unique_id])?;
            NativeMailCommand::MailLockedItem {
                unique_id: *unique_id,
                locked: *locked,
            }
        }
        NativePlayerUiIntent::SendMail {
            recipient,
            message,
            gold,
            attachment_unique_ids,
            stamped,
        } => NativeMailCommand::SendMail {
            name: recipient.clone(),
            message: message.clone(),
            gold: *gold,
            items_idx: attachments(attachment_unique_ids)?,
            stamped: *stamped,
        },
        _ => return Err(NotMail),
    })
}

/// Frozen Windows helper: Bag1/Bag2 are container 0; belt/equipment/quest are not.
fn mail_attachment_indices(inventory: &InventoryModel, ids: &[u64]) -> Option<[u64; 5]> {
    if ids.len() > 5
        || ids.iter().any(|id| *id == 0)
        || ids
            .iter()
            .enumerate()
            .any(|(index, id)| ids[..index].contains(id))
    {
        return None;
    }

    let mut indices = [0_u64; 5];
    for (index, id) in ids.iter().enumerate() {
        let item = inventory
            .items
            .iter()
            .find(|item| item.unique_id == Some(*id))?;
        if item.container != 0 || item.slot >= u32::from(inventory.bag_slot_capacity()) {
            return None;
        }
        indices[index] = *id;
    }
    Some(indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crystal_ui::overlays::NativePlayerUiIntentQueue, inventory::ItemModel,
        pending_operations::PendingOperations,
    };
    use serde_json::{json, Value};
    fn inventory() -> InventoryModel {
        InventoryModel {
            capacity: 86,
            items: (1..=5)
                .map(|id| ItemModel {
                    unique_id: Some(id),
                    slot: 40 + id as u32,
                    container: 0,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn native_mail_egress_all_seven_wire_commands_match_windows_fields() {
        let inventory = inventory();
        let cases = [
            (
                NativePlayerUiIntent::ReadMail { mail_id: u64::MAX },
                json!({"type":"readMail","mailId":u64::MAX}),
            ),
            (
                NativePlayerUiIntent::ClaimMail { mail_id: 2 },
                json!({"type":"collectParcel","mailId":2}),
            ),
            (
                NativePlayerUiIntent::DeleteMail { mail_id: 3 },
                json!({"type":"deleteMail","mailId":3}),
            ),
            (
                NativePlayerUiIntent::LockMail {
                    mail_id: 4,
                    lock: false,
                },
                json!({"type":"lockMail","mailId":4,"lock":false}),
            ),
            (
                NativePlayerUiIntent::MailCost {
                    gold: u32::MAX,
                    attachment_unique_ids: vec![5, 1],
                    stamped: true,
                },
                json!({"type":"mailCost","gold":u32::MAX,"itemsIdx":[5,1,0,0,0],"stamped":true}),
            ),
            (
                NativePlayerUiIntent::MailLockItem {
                    unique_id: 5,
                    locked: false,
                },
                json!({"type":"mailLockedItem","uniqueId":5,"locked":false}),
            ),
            (
                NativePlayerUiIntent::SendMail {
                    recipient: "Friend".into(),
                    message: "邮件\nhello 👋".into(),
                    gold: 7,
                    attachment_unique_ids: vec![5, 4, 3, 2, 1],
                    stamped: true,
                },
                json!({"type":"sendMail","name":"Friend","message":"邮件\nhello 👋","gold":7,
                    "itemsIdx":[5,4,3,2,1],"stamped":true}),
            ),
        ];
        for (intent, expected) in cases {
            let command = project_native_mail_intent(&intent, Some(&inventory)).unwrap();
            let raw = serde_json::to_string(&command).unwrap();
            assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(), expected);
            assert_eq!(command.command_type(), expected["type"].as_str().unwrap());
        }
    }
    #[test]
    fn native_mail_egress_attachment_validation_keeps_frozen_windows_boundary() {
        let mut inventory = inventory();
        assert_eq!(
            mail_attachment_indices(&inventory, &[5, 1]),
            Some([5, 1, 0, 0, 0])
        );
        for ids in [vec![0], vec![1, 1], vec![99], vec![1, 2, 3, 4, 5, 6]] {
            assert_eq!(mail_attachment_indices(&inventory, &ids), None);
        }
        for container in [1, 2, 3] {
            inventory.items[0].container = container;
            assert_eq!(mail_attachment_indices(&inventory, &[1]), None);
        }
        inventory.items[0].container = 0;
        inventory.items[0].slot = 80;
        assert_eq!(mail_attachment_indices(&inventory, &[1]), None);
        inventory.items[0].slot = 47;
        inventory.capacity = 46;
        assert_eq!(mail_attachment_indices(&inventory, &[1]), None);
        inventory.capacity = 47; // malformed lengths do not prove expansion
        assert_eq!(mail_attachment_indices(&inventory, &[1]), None);
    }
    #[test]
    fn native_mail_egress_never_infers_empty_inventory_or_fabricates_other_commands() {
        let quote = NativePlayerUiIntent::MailCost {
            gold: 0,
            attachment_unique_ids: vec![],
            stamped: false,
        };
        assert_eq!(
            project_native_mail_intent(&quote, None),
            Err(NativeMailProjectionError::MissingInventory)
        );
        assert!(project_native_mail_intent(&quote, Some(&inventory())).is_ok());
        assert_eq!(
            project_native_mail_intent(&NativePlayerUiIntent::RefreshFriends, Some(&inventory())),
            Err(NativeMailProjectionError::NotMail)
        );
        assert!(!is_native_mail_intent(
            &NativePlayerUiIntent::RefreshFriends
        ));
    }
    #[test]
    fn native_mail_egress_bounded_queue_drain_preserves_other_and_remaining_mail_fifo() {
        let mut queue = NativePlayerUiIntentQueue::default();
        let all = [
            NativePlayerUiIntent::RefreshFriends,
            NativePlayerUiIntent::ReadMail { mail_id: 1 },
            NativePlayerUiIntent::MailCost {
                gold: 2,
                attachment_unique_ids: vec![],
                stamped: false,
            },
            NativePlayerUiIntent::ObservePlayer {
                name: "Friend".into(),
            },
            NativePlayerUiIntent::ReadMail { mail_id: 3 },
        ];
        for intent in all.clone() {
            assert!(queue.push_intent(intent));
        }
        assert!(queue.drain_mail_intents_bounded(0).is_empty());
        assert_eq!(queue.drain_mail_intents_bounded(1), [all[1].clone()]);
        assert_eq!(queue.drain_mail_intents_bounded(1), [all[2].clone()]);
        assert_eq!(
            queue.drain_intents(),
            [all[0].clone(), all[3].clone(), all[4].clone()]
        );
    }
    #[test]
    fn native_mail_egress_drain_and_serialization_never_settle_pending_claim() {
        let mut pending = PendingOperations::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        let intent = NativePlayerUiIntent::ClaimMail { mail_id: 1 };
        let key = intent.pending_key().unwrap();
        assert!(queue.push_pending_intent(&mut pending, intent));
        assert!(!queue
            .push_pending_intent(&mut pending, NativePlayerUiIntent::ClaimMail { mail_id: 2 }));
        let command =
            project_native_mail_intent(&queue.drain_mail_intents_bounded(1)[0], None).unwrap();
        assert!(serde_json::to_string(&command).is_ok());
        assert!(pending.contains(&key));
    }
}

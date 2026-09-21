//! Ordered, packet-authoritative parcel service events for the native UI.
//!
//! Mail postage responses have no request correlation in Crystal.  Keeping
//! them in an ordered inbox lets the UI issue one quote at a time instead of
//! guessing which local edit a later response belongs to.

use std::collections::VecDeque;

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const MAIL_SERVICE_INBOX_CAPACITY: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MailServiceEvent {
    /// Crystal `MailSendRequest`: prompt for a recipient and open the parcel
    /// composer only after the player accepts that prompt.
    OpenParcel,
    /// Crystal `MailCost`. This is a server calculation, never a local fee.
    Cost { cost: u32 },
    /// Crystal `MailLockedItem`, echoed after an item lock/unlock request.
    LockedItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        locked: bool,
    },
}

/// Bounded ordered inbox for parcel-service packets.
///
/// Fullness rejects the newest event rather than evicting an earlier response:
/// the UI uses a single-flight postage request, so preserving order is more
/// important than replacing a response with an unrelated later one.
#[derive(Debug, Resource, Default)]
pub struct MailServiceInbox {
    events: VecDeque<MailServiceEvent>,
}

impl MailServiceInbox {
    pub fn push(&mut self, event: MailServiceEvent) -> bool {
        if self.events.len() >= MAIL_SERVICE_INBOX_CAPACITY {
            return false;
        }
        self.events.push_back(event);
        true
    }

    pub fn pop(&mut self) -> Option<MailServiceEvent> {
        self.events.pop_front()
    }

    pub fn drain(&mut self) -> Vec<MailServiceEvent> {
        self.events.drain(..).collect()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_source_packet_events_with_stable_camel_case_fields() {
        assert_eq!(
            serde_json::to_string(&MailServiceEvent::LockedItem {
                unique_id: 77,
                locked: true,
            })
            .unwrap(),
            r#"{"kind":"lockedItem","uniqueId":77,"locked":true}"#
        );
        assert_eq!(
            serde_json::from_str::<MailServiceEvent>(r#"{"kind":"cost","cost":125}"#)
                .unwrap(),
            MailServiceEvent::Cost { cost: 125 }
        );
    }

    #[test]
    fn inbox_is_ordered_and_rejects_overflow_without_eviction() {
        let mut inbox = MailServiceInbox::default();
        assert!(inbox.push(MailServiceEvent::OpenParcel));
        for cost in 1..MAIL_SERVICE_INBOX_CAPACITY {
            assert!(inbox.push(MailServiceEvent::Cost {
                cost: cost as u32,
            }));
        }
        assert!(!inbox.push(MailServiceEvent::LockedItem {
            unique_id: 9,
            locked: true,
        }));
        assert_eq!(inbox.pop(), Some(MailServiceEvent::OpenParcel));
        assert_eq!(inbox.pop(), Some(MailServiceEvent::Cost { cost: 1 }));
        assert_eq!(inbox.len(), MAIL_SERVICE_INBOX_CAPACITY - 2);
        inbox.clear();
        assert!(inbox.is_empty());
    }
}

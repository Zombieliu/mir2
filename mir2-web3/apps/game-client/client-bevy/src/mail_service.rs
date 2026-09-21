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
/// Fullness rejects the newest ordinary event rather than evicting an earlier
/// response. One `Cost` has a dedicated reserve because it resolves the UI's
/// only in-flight, uncorrelated quote. While that reserve is occupied, later
/// parcel-service events are rejected so none can overtake the reserved Cost.
#[derive(Debug, Resource, Default)]
pub struct MailServiceInbox {
    events: VecDeque<MailServiceEvent>,
    cost_reserve: Option<u32>,
}

impl MailServiceInbox {
    pub fn push(&mut self, event: MailServiceEvent) -> bool {
        if self.cost_reserve.is_some() {
            return false;
        }
        if self.events.len() >= MAIL_SERVICE_INBOX_CAPACITY {
            if let MailServiceEvent::Cost { cost } = event {
                self.cost_reserve = Some(cost);
                return true;
            }
            return false;
        }
        self.events.push_back(event);
        true
    }

    pub fn pop(&mut self) -> Option<MailServiceEvent> {
        self.events
            .pop_front()
            .or_else(|| self.cost_reserve.take().map(|cost| MailServiceEvent::Cost { cost }))
    }

    pub fn drain(&mut self) -> Vec<MailServiceEvent> {
        let mut events = self.events.drain(..).collect::<Vec<_>>();
        if let Some(cost) = self.cost_reserve.take() {
            events.push(MailServiceEvent::Cost { cost });
        }
        events
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.cost_reserve = None;
    }

    pub fn len(&self) -> usize {
        self.events.len() + usize::from(self.cost_reserve.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.cost_reserve.is_none()
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
    fn inbox_reserves_one_cost_after_full_fifo_without_reordering() {
        let mut inbox = MailServiceInbox::default();
        assert!(inbox.push(MailServiceEvent::OpenParcel));
        for unique_id in 1..MAIL_SERVICE_INBOX_CAPACITY {
            assert!(inbox.push(MailServiceEvent::LockedItem {
                unique_id: unique_id as u64,
                locked: true,
            }));
        }
        assert!(inbox.push(MailServiceEvent::Cost { cost: 125 }));
        assert!(!inbox.push(MailServiceEvent::LockedItem {
            unique_id: 99,
            locked: true,
        }));
        assert!(!inbox.push(MailServiceEvent::Cost { cost: 250 }));
        assert_eq!(inbox.pop(), Some(MailServiceEvent::OpenParcel));
        let remaining = inbox.drain();
        assert_eq!(remaining.len(), MAIL_SERVICE_INBOX_CAPACITY);
        assert_eq!(
            remaining.last(),
            Some(&MailServiceEvent::Cost { cost: 125 })
        );
        inbox.clear();
        assert!(inbox.is_empty());
    }

    #[test]
    fn reserved_cost_keeps_inbox_nonempty_until_popped_or_cleared() {
        let mut inbox = MailServiceInbox::default();
        for _ in 0..MAIL_SERVICE_INBOX_CAPACITY {
            assert!(inbox.push(MailServiceEvent::OpenParcel));
        }
        assert!(inbox.push(MailServiceEvent::Cost { cost: 7 }));
        for _ in 0..MAIL_SERVICE_INBOX_CAPACITY {
            assert_eq!(inbox.pop(), Some(MailServiceEvent::OpenParcel));
        }
        assert!(!inbox.is_empty());
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox.pop(), Some(MailServiceEvent::Cost { cost: 7 }));
        assert!(inbox.is_empty());
    }
}

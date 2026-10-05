//! Ordered native parcel events. The local envelope identifies the producing
//! socket without changing the Crystal-compatible MailServiceEvent JSON.
use std::collections::VecDeque;
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const MAIL_SERVICE_INBOX_CAPACITY: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MailServiceEvent {
    OpenParcel,
    Cost { cost: u32 },
    LockedItem {
        #[serde(rename = "uniqueId")]
        unique_id: u64,
        locked: bool,
    },
}

/// NativeCommandFence transport identity. Compare the full pair: a UI reset
/// revision or a packed/wrapping integer cannot identify a producing socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MailServiceStreamEpoch {
    pub run: u64,
    pub connection: u64,
}
impl MailServiceStreamEpoch {
    pub fn is_valid(self) -> bool { self.run != 0 && self.connection != 0 }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailServiceStreamStarted { pub epoch: MailServiceStreamEpoch }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailServiceDelivery {
    pub epoch: MailServiceStreamEpoch,
    pub event: MailServiceEvent,
}
/// Exact local mirror of NativeCommandStamp plus its checked sequence/token.
/// No serde implementation: these fields never enter the gateway wire JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailQuoteTicket {
    pub run:u64,pub connection:u64,pub procedure:u64,pub owner_epoch:u64,
    pub scene_epoch:u64,pub cancellation:u64,pub actor:Option<u32>,pub map:Option<i32>,
    pub sequence:u64,pub local_quote_token:u64,
}
impl MailQuoteTicket {
    pub fn epoch(self)->MailServiceStreamEpoch {MailServiceStreamEpoch{run:self.run,connection:self.connection}}
    pub fn is_valid(self)->bool {self.epoch().is_valid()&&self.procedure!=0&&self.owner_epoch!=0&&self.scene_epoch!=0&&self.cancellation!=0&&self.sequence!=0&&self.local_quote_token!=0&&self.actor.is_some_and(|id|id!=0)}
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailQuoteOutcome {DefinitelyUnsent,Entered{at_ms:u64}}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailQuoteReceipt {pub ticket:MailQuoteTicket,pub outcome:MailQuoteOutcome}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailSendTicket {
    pub run:u64,pub connection:u64,pub procedure:u64,pub owner_epoch:u64,
    pub scene_epoch:u64,pub cancellation:u64,pub actor:Option<u32>,pub map:Option<i32>,
    pub sequence:u64,pub local_send_token:u64,
}
impl MailSendTicket {
    pub fn epoch(self)->MailServiceStreamEpoch {MailServiceStreamEpoch{run:self.run,connection:self.connection}}
    pub fn is_valid(self)->bool {MailQuoteTicket{run:self.run,connection:self.connection,procedure:self.procedure,owner_epoch:self.owner_epoch,scene_epoch:self.scene_epoch,cancellation:self.cancellation,actor:self.actor,map:self.map,sequence:self.sequence,local_quote_token:self.local_send_token}.is_valid()}
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailSendOutcome {DefinitelyUnsent,Entered,Flushed,Unknown}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailSendReceipt {pub ticket:MailSendTicket,pub outcome:MailSendOutcome}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailSendAcknowledgement {pub ticket:MailSendTicket,pub result:i32}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailServiceInboxMessage {
    StreamStarted(MailServiceStreamStarted),
    Delivery(MailServiceDelivery),
    QuoteReceipt(MailQuoteReceipt),
    SendReceipt(MailSendReceipt),
    SendAcknowledgement(MailSendAcknowledgement),
}

/// Bounded FIFO with a protected stream marker and one tagged Cost reserve.
/// The high-water survives drains and session resets. Same-stream markers
/// preserve entered/reserved replies; newer streams discard old deliveries.
#[derive(Debug, Resource, Default)]
pub struct MailServiceInbox {
    events: VecDeque<MailServiceInboxMessage>,
    terminal_tail: VecDeque<MailServiceInboxMessage>,
    stream_epoch: Option<MailServiceStreamEpoch>,
    stream_marker_pending: bool,
    stream_failed:bool,
}
impl MailServiceInbox {
    pub fn stream_epoch(&self) -> Option<MailServiceStreamEpoch> { self.stream_epoch }
    pub fn is_stream_failed(&self)->bool{self.stream_failed}
    pub fn fail_stream(&mut self,epoch:MailServiceStreamEpoch){if self.stream_epoch==Some(epoch){self.stream_failed=true;}}
    pub fn start_stream(&mut self, marker: MailServiceStreamStarted) -> bool {
        let epoch = marker.epoch;
        if !epoch.is_valid() || self.stream_epoch.is_some_and(|current| epoch < current) { return false; }
        if self.stream_epoch == Some(epoch) { return true; }
        self.events.clear(); self.terminal_tail.clear();
        self.stream_epoch = Some(epoch); self.stream_marker_pending = true;
        self.stream_failed=false;
        true
    }
    pub fn push_delivery(&mut self, delivery: MailServiceDelivery) -> bool {
        if self.stream_failed || !delivery.epoch.is_valid() || self.stream_epoch != Some(delivery.epoch) || self.terminal_tail.iter().any(is_cost) || matches!(delivery.event,MailServiceEvent::Cost{..})&&self.events.iter().any(is_cost) { return false; }
        if self.events.len() >= MAIL_SERVICE_INBOX_CAPACITY || !self.terminal_tail.is_empty() {
            if matches!(delivery.event,MailServiceEvent::Cost{..}) {
                self.terminal_tail.push_back(MailServiceInboxMessage::Delivery(delivery)); return true;
            }
            return false;
        }
        self.events.push_back(MailServiceInboxMessage::Delivery(delivery)); true
    }
    pub fn push_quote_receipt(&mut self,receipt:MailQuoteReceipt)->bool {
        if self.stream_failed||!receipt.ticket.is_valid()||self.stream_epoch!=Some(receipt.ticket.epoch())||self.terminal_tail.iter().any(|message|is_cost(message)||matches!(message,MailServiceInboxMessage::QuoteReceipt(_)))||self.events.iter().any(|message|is_cost(message)||matches!(message,MailServiceInboxMessage::QuoteReceipt(_))){return false;}
        if self.events.len()>=MAIL_SERVICE_INBOX_CAPACITY || !self.terminal_tail.is_empty() {self.terminal_tail.push_back(MailServiceInboxMessage::QuoteReceipt(receipt));}
        else {self.events.push_back(MailServiceInboxMessage::QuoteReceipt(receipt));}
        true
    }
    fn push_send_terminal(&mut self,message:MailServiceInboxMessage,ticket:MailSendTicket)->bool {
        if self.stream_failed||!ticket.is_valid()||self.stream_epoch!=Some(ticket.epoch()){return false;}
        let same_kind=|queued:&MailServiceInboxMessage|match (&message,queued){
            (MailServiceInboxMessage::SendReceipt(_),MailServiceInboxMessage::SendReceipt(_))=>true,
            (MailServiceInboxMessage::SendAcknowledgement(_),MailServiceInboxMessage::SendAcknowledgement(_))=>true,_=>false};
        let limit=if matches!(message,MailServiceInboxMessage::SendReceipt(_)){2}else{1};
        if self.events.iter().chain(self.terminal_tail.iter()).filter(|queued|same_kind(queued)).count()>=limit{return false;}
        if self.events.len()>=MAIL_SERVICE_INBOX_CAPACITY||!self.terminal_tail.is_empty(){self.terminal_tail.push_back(message);}else{self.events.push_back(message);}true
    }
    pub fn push_send_receipt(&mut self,receipt:MailSendReceipt)->bool{self.push_send_terminal(MailServiceInboxMessage::SendReceipt(receipt),receipt.ticket)}
    pub fn push_send_acknowledgement(&mut self,ack:MailSendAcknowledgement)->bool{
        matches!(ack.result,1|-1)&&self.push_send_terminal(MailServiceInboxMessage::SendAcknowledgement(ack),ack.ticket)
    }
    /// Controlled local unit-fixture convenience after explicit announcement.
    /// Production ingress only accepts epoch-tagged push_delivery.
    #[cfg(test)]
    pub fn push(&mut self, event: MailServiceEvent) -> bool {
        let Some(epoch) = self.stream_epoch else { return false; };
        self.push_delivery(MailServiceDelivery { epoch, event })
    }
    pub fn drain_ordered(&mut self) -> Vec<MailServiceInboxMessage> {
        let mut messages = Vec::new();
        if self.stream_marker_pending {
            self.stream_marker_pending = false;
            if let Some(epoch) = self.stream_epoch { messages.push(MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch })); }
        }
        messages.extend(self.events.drain(..));
        messages.extend(self.terminal_tail.drain(..));
        messages
    }
    pub fn pop(&mut self) -> Option<MailServiceEvent> {
        // Legacy event-only consumers cannot skip or discard a typed receipt.
        match self.events.front(){
            Some(MailServiceInboxMessage::Delivery(_))=>match self.events.pop_front().unwrap(){MailServiceInboxMessage::Delivery(delivery)=>Some(delivery.event),_=>unreachable!()},
            Some(_)=>None,
            None if self.terminal_tail.front().is_some_and(is_cost)=>match self.terminal_tail.pop_front().unwrap(){MailServiceInboxMessage::Delivery(delivery)=>Some(delivery.event),_=>unreachable!()},
            None=>None,
        }
    }
    pub fn drain(&mut self) -> Vec<MailServiceEvent> {
        self.stream_marker_pending=false;
        let mut events=Vec::new();while let Some(event)=self.pop(){events.push(event);}events
    }
    /// A local account/session reset cannot discard this socket's quote slot.
    /// Preserve tagged Cost and marker/high-water; clear OpenParcel/lock echoes.
    pub fn clear_session(&mut self) {
        self.events.retain(is_terminal);
    }
    /// Clear deliveries only; this never resets trusted transport high-water.
    pub fn clear(&mut self) {self.events.retain(|message|is_terminal(message)&&!is_cost(message));self.terminal_tail.retain(|message|!is_cost(message));}
    pub fn len(&self) -> usize { self.events.len() + self.terminal_tail.len() + usize::from(self.stream_marker_pending) }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
}
fn is_cost(message:&MailServiceInboxMessage)->bool {matches!(message,MailServiceInboxMessage::Delivery(MailServiceDelivery{event:MailServiceEvent::Cost{..},..}))}
fn is_terminal(message:&MailServiceInboxMessage)->bool {is_cost(message)||matches!(message,MailServiceInboxMessage::QuoteReceipt(_)|MailServiceInboxMessage::SendReceipt(_)|MailServiceInboxMessage::SendAcknowledgement(_))}

#[cfg(test)]
mod tests {
    use super::*;
    fn mail_send_ticket()->MailSendTicket{MailSendTicket{run:1,connection:1,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:74,local_send_token:20}}
    #[test]
    fn mail_send_terminal_tail_keeps_old_cost_before_entry_write_and_ack_across_reset(){
        let mut inbox=inbox();for _ in 0..MAIL_SERVICE_INBOX_CAPACITY{assert!(inbox.push(MailServiceEvent::OpenParcel));}
        let ticket=mail_send_ticket();let entry=MailSendReceipt{ticket,outcome:MailSendOutcome::Entered};let write=MailSendReceipt{ticket,outcome:MailSendOutcome::Flushed};let ack=MailSendAcknowledgement{ticket,result:1};
        assert!(inbox.push(MailServiceEvent::Cost{cost:70}));assert!(inbox.push_send_receipt(entry));assert!(inbox.push_send_receipt(write));assert!(inbox.push_send_acknowledgement(ack));
        assert!(!inbox.push_send_receipt(entry));assert!(!inbox.push_send_acknowledgement(ack));
        assert!(!inbox.push_quote_receipt(quote_receipt()),"only quote entry is forbidden behind its own Cost");
        inbox.clear_session();assert_eq!(inbox.drain_ordered(),vec![MailServiceInboxMessage::Delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::Cost{cost:70}}),MailServiceInboxMessage::SendReceipt(entry),MailServiceInboxMessage::SendReceipt(write),MailServiceInboxMessage::SendAcknowledgement(ack)]);
        assert!(inbox.push_send_receipt(entry));inbox.fail_stream(EPOCH);inbox.clear_session();assert!(!inbox.push_send_acknowledgement(ack));assert!(inbox.start_stream(MailServiceStreamStarted{epoch:EPOCH}));assert!(inbox.is_stream_failed());
        assert!(inbox.start_stream(MailServiceStreamStarted{epoch:MailServiceStreamEpoch{run:1,connection:2}}));assert!(!inbox.is_stream_failed());assert!(!inbox.push_send_receipt(entry));
    }
    fn quote_receipt()->MailQuoteReceipt {MailQuoteReceipt{ticket:MailQuoteTicket{run:1,connection:1,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:73,local_quote_token:19},outcome:MailQuoteOutcome::Entered{at_ms:500}}}
    #[test]
    fn mail_quote_receipt_reserve_precedes_cost_and_legacy_consumer_cannot_skip_it(){
        let mut inbox=inbox();for _ in 0..MAIL_SERVICE_INBOX_CAPACITY {assert!(inbox.push(MailServiceEvent::OpenParcel));}
        let receipt=quote_receipt();assert!(inbox.push_quote_receipt(receipt));assert!(inbox.push(MailServiceEvent::Cost{cost:70}));
        assert!(!inbox.push_quote_receipt(receipt));assert!(!inbox.push(MailServiceEvent::OpenParcel));assert_eq!(inbox.len(),MAIL_SERVICE_INBOX_CAPACITY+2);
        assert_eq!(inbox.drain().len(),MAIL_SERVICE_INBOX_CAPACITY);assert_eq!(inbox.pop(),None);
        assert_eq!(inbox.drain_ordered(),vec![MailServiceInboxMessage::QuoteReceipt(receipt),MailServiceInboxMessage::Delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::Cost{cost:70}})]);
    }
    #[test]
    fn mail_quote_receipt_and_cost_survive_session_reset_in_fifo_and_reserve(){
        for reserved in [false,true] {
            let mut inbox=inbox();if reserved{for _ in 0..MAIL_SERVICE_INBOX_CAPACITY{assert!(inbox.push(MailServiceEvent::OpenParcel));}}
            let receipt=quote_receipt();assert!(inbox.push_quote_receipt(receipt));assert!(inbox.push(MailServiceEvent::Cost{cost:70}));
            assert!(inbox.start_stream(MailServiceStreamStarted{epoch:EPOCH}));inbox.clear_session();
            assert_eq!(inbox.drain_ordered(),vec![MailServiceInboxMessage::QuoteReceipt(receipt),MailServiceInboxMessage::Delivery(MailServiceDelivery{epoch:EPOCH,event:MailServiceEvent::Cost{cost:70}})]);
        }
    }
    #[test]
    fn mail_quote_receipt_positive_ticket_stream_and_failure_reset_are_strict(){
        let mut inbox=MailServiceInbox::default();let receipt=quote_receipt();assert!(!inbox.push_quote_receipt(receipt));
        assert!(inbox.start_stream(MailServiceStreamStarted{epoch:EPOCH}));
        for field in 0..3 {let mut invalid=receipt;match field{0=>invalid.ticket.local_quote_token=0,1=>invalid.ticket.sequence=0,_=>invalid.ticket.run=0};assert!(!inbox.push_quote_receipt(invalid));}
        assert!(inbox.push_quote_receipt(receipt));inbox.fail_stream(EPOCH);inbox.clear_session();assert!(inbox.is_stream_failed());
        assert!(inbox.start_stream(MailServiceStreamStarted{epoch:EPOCH}));assert!(inbox.is_stream_failed());assert!(!inbox.push(MailServiceEvent::Cost{cost:70}));
        let newer=MailServiceStreamEpoch{run:1,connection:2};assert!(inbox.start_stream(MailServiceStreamStarted{epoch:newer}));assert!(!inbox.is_stream_failed());
        assert!(!inbox.push_quote_receipt(receipt));assert_eq!(inbox.drain_ordered(),vec![MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted{epoch:newer})]);
    }
    const EPOCH: MailServiceStreamEpoch = MailServiceStreamEpoch { run: 1, connection: 1 };
    fn inbox() -> MailServiceInbox {
        let mut inbox = MailServiceInbox::default();
        assert!(inbox.start_stream(MailServiceStreamStarted { epoch: EPOCH }));
        assert_eq!(inbox.drain_ordered(), vec![MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch: EPOCH })]);
        inbox
    }
    #[test]
    fn serializes_source_packet_events_with_stable_camel_case_fields() {
        assert_eq!(serde_json::to_string(&MailServiceEvent::OpenParcel).unwrap(), r#"{"kind":"openParcel"}"#);
        assert_eq!(serde_json::to_string(&MailServiceEvent::Cost { cost: 125 }).unwrap(), r#"{"kind":"cost","cost":125}"#);
        assert_eq!(serde_json::to_string(&MailServiceEvent::LockedItem { unique_id: 77, locked: true }).unwrap(), r#"{"kind":"lockedItem","uniqueId":77,"locked":true}"#);
        assert_eq!(serde_json::from_str::<MailServiceEvent>(r#"{"kind":"cost","cost":125}"#).unwrap(), MailServiceEvent::Cost { cost: 125 });
    }
    #[test]
    fn inbox_reserves_one_cost_after_full_fifo_without_reordering() {
        let mut inbox = inbox();
        assert!(inbox.push(MailServiceEvent::OpenParcel));
        for unique_id in 1..MAIL_SERVICE_INBOX_CAPACITY { assert!(inbox.push(MailServiceEvent::LockedItem { unique_id: unique_id as u64, locked: true })); }
        assert!(inbox.push(MailServiceEvent::Cost { cost: 125 }));
        assert!(!inbox.push(MailServiceEvent::LockedItem { unique_id: 99, locked: true }));
        assert!(!inbox.push(MailServiceEvent::Cost { cost: 250 }));
        assert_eq!(inbox.pop(), Some(MailServiceEvent::OpenParcel));
        let remaining = inbox.drain();
        assert_eq!(remaining.len(), MAIL_SERVICE_INBOX_CAPACITY);
        assert_eq!(remaining.last(), Some(&MailServiceEvent::Cost { cost: 125 }));
        inbox.clear(); assert!(inbox.is_empty()); assert_eq!(inbox.stream_epoch(), Some(EPOCH));
    }
    #[test]
    fn reserved_cost_keeps_inbox_nonempty_until_popped_or_cleared() {
        let mut inbox = inbox();
        for _ in 0..MAIL_SERVICE_INBOX_CAPACITY { assert!(inbox.push(MailServiceEvent::OpenParcel)); }
        assert!(inbox.push(MailServiceEvent::Cost { cost: 7 }));
        for _ in 0..MAIL_SERVICE_INBOX_CAPACITY { assert_eq!(inbox.pop(), Some(MailServiceEvent::OpenParcel)); }
        assert!(!inbox.is_empty()); assert_eq!(inbox.len(), 1);
        assert_eq!(inbox.pop(), Some(MailServiceEvent::Cost { cost: 7 })); assert!(inbox.is_empty());
    }
    #[test]
    fn stream_highwater_is_full_pair_and_same_marker_preserves_reserved_cost() {
        let mut inbox = MailServiceInbox::default();
        let delivery = |epoch| MailServiceDelivery { epoch, event: MailServiceEvent::Cost { cost: 7 } };
        assert!(!inbox.push_delivery(delivery(EPOCH)));
        for epoch in [MailServiceStreamEpoch { run: 0, connection: 1 }, MailServiceStreamEpoch { run: 1, connection: 0 }] {
            assert!(!inbox.start_stream(MailServiceStreamStarted { epoch })); assert!(!inbox.push_delivery(delivery(epoch)));
        }
        assert!(inbox.start_stream(MailServiceStreamStarted { epoch: EPOCH }));
        for _ in 0..MAIL_SERVICE_INBOX_CAPACITY { assert!(inbox.push(MailServiceEvent::OpenParcel)); }
        assert!(inbox.push_delivery(delivery(EPOCH)));
        assert!(inbox.start_stream(MailServiceStreamStarted { epoch: EPOCH }));
        inbox.clear_session();
        assert_eq!(inbox.drain_ordered(), vec![MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch: EPOCH }), MailServiceInboxMessage::Delivery(delivery(EPOCH))]);
        let high = MailServiceStreamEpoch { run: 1, connection: u64::MAX };
        assert!(inbox.start_stream(MailServiceStreamStarted { epoch: high })); assert!(inbox.push_delivery(delivery(high)));
        let next = MailServiceStreamEpoch { run: 2, connection: 1 };
        assert!(!inbox.push_delivery(delivery(next)), "future delivery needs an announced marker");
        assert!(inbox.start_stream(MailServiceStreamStarted { epoch: next }));
        assert!(!inbox.start_stream(MailServiceStreamStarted { epoch: high })); assert!(!inbox.push_delivery(delivery(high)));
        let last = MailServiceStreamEpoch { run: u64::MAX, connection: u64::MAX };
        assert!(inbox.start_stream(MailServiceStreamStarted { epoch: last })); assert!(inbox.push_delivery(delivery(last)));
        assert!(inbox.start_stream(MailServiceStreamStarted { epoch: last }));
        assert_eq!(inbox.drain_ordered(), vec![MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch: last }), MailServiceInboxMessage::Delivery(delivery(last))]);
    }
}

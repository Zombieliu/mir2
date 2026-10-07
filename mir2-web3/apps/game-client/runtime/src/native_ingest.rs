//! Cross-thread native ingestion for the Bevy runtime.
//!
//! On WASM, the JS host writes pending snapshots into `thread_local!` cells on
//! the same thread the Bevy loop runs, and the ingest systems drain them. A
//! native host runs a background WebSocket task on its own thread, so it cannot
//! write those thread-locals. This module provides a process-global queue that
//! the native host pushes JSON into and the Bevy main thread drains every
//! frame. The native side uses a bounded, coalescing shared queue
//! so a stalled render loop cannot turn gateway traffic into unbounded memory.
//!
//! The queue is only wired when a native host builds an app; the WASM path is
//! unchanged and still uses the thread-locals in `lib.rs`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::prelude::Resource;
use mir2_client_bevy::mail_service::{
    MailServiceDelivery, MailServiceEvent, MailServiceStreamEpoch, MailServiceStreamStarted,
    MailQuoteReceipt,MailSendReceipt,MailSendAcknowledgement,
};

/// Replaceable process-global queue used by the native host. A replaceable
/// slot matters for tests and for hosts that rebuild a Bevy app in-process;
/// a permanently fixed queue would point at the first, possibly dropped app.
static NATIVE_QUEUE: OnceLock<Mutex<Option<Arc<Mutex<NativeInboundBuffer>>>>> = OnceLock::new();

// Tests that install a process-global native queue must not run concurrently.
// Production never acquires this lock; it exists only to make the default
// parallel Rust test harness deterministic across this module and lib.rs.
#[cfg(test)]
static NATIVE_QUEUE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn native_queue_test_guard() -> std::sync::MutexGuard<'static, ()> {
    NATIVE_QUEUE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Keep native transport backpressure deterministic. Snapshot traffic gets a
/// large coalescing budget while the tail remains available to resets and
/// operation acknowledgements even when rendering stalls.
const MAX_NATIVE_MESSAGES: usize = 256;
const MAX_COALESCED_SNAPSHOTS: usize = 192;
const NON_CRITICAL_MESSAGE_LIMIT: usize = 224;
const MAX_OPERATION_ACK_MESSAGES: usize = 32;
const MAX_NATIVE_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_NATIVE_BUFFER_BYTES: usize = 128 * 1024 * 1024;

/// Read-only queue occupancy exposed to the opt-in native soak diagnostics.
/// The byte count includes owned `String`/pixel-vector capacity, matching the
/// admission accounting used by the bounded queue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct NativeInboundDiagnostics {
    pub(crate) message_count: usize,
    pub(crate) retained_bytes: usize,
}

/// A snapshot JSON pushed from a background native task.
#[derive(Debug, Clone)]
pub(crate) enum NativeInboundMessage {
    /// Complete owner economic checkpoint. Never coalesce, split or evict it.
    NpcEconomyBundle(crate::npc_purchase_economy::NativeNpcEconomyBundle),
    WorldState(String),
    EntityRenderState(String),
    EffectRenderState(String),
    LightingRenderState(String),
    MapRenderState(String),
    UiReadModel(String),
    /// Packet-first authoritative wallet patch. A later world snapshot may
    /// repeat the values, but the patch prevents a stale HUD after a credit
    /// or gold delta packet.
    WalletPatch(String),
    MapModel(String),
    EntityModelSet(String),
    InventoryModel(String),
    /// Exact Drop/Move/Merge/SplitItem1 ACK or NACK. This never mutates the
    /// inventory model; it only terminates a correlatable pending command.
    InventoryOperationAck(String),
    ChatLine(String),
    /// Clear every character/session read model at a session boundary. Session
    /// reset handling also applies SceneReset semantics in the runtime.
    /// This is deliberately separate from a typed model so logout cannot
    /// expose the previous account while the next snapshot is pending.
    DataReset,
    /// Clear all ordinary account/session data while retaining and consuming
    /// one exact GameShop receipt already accepted by the transport. The full
    /// typed receipt makes the boundary atomic even if the Bevy consumer
    /// drained the dedicated reserve immediately before this message arrived.
    DataResetPreservingExactGameShopReceipt(mir2_client_bevy::game_shop::GameShopReceipt),
    /// Clear only scene/world presentation state at a map boundary. Personal
    /// read models, login state, and UI pending operations remain intact.
    SceneReset,
    MailModel(String),
    /// Ordered Crystal parcel-service packets. Unlike mailbox snapshots these
    /// responses have no request ID and must never be coalesced.
    MailService(MailServiceDelivery),
    /// Protected local marker published before this socket's first packet.
    MailServiceStreamStarted(MailServiceStreamStarted),
    MailQuoteReceipt(MailQuoteReceipt),
    MailSendReceipt(MailSendReceipt),
    MailSendAcknowledgement(MailSendAcknowledgement),
    ShopModel(String),
    GameShopInfo(String),
    GameShopStock(String),
    /// Correlatable native purchase result; never evicted for snapshots.
    GameShopReceipt(String),
    /// Authoritative NPCGoods/NPCSell/NPCRepair/NPCSRepair service transition.
    NpcShopService(String),
    StorageModel(String),
    /// Replace only the authoritative storage item list. UserStorage packets
    /// do not carry storage size/password metadata, so they must not replace
    /// the complete StorageModel.
    StorageItems(String),
    /// Apply authoritative storage metadata from a storage result packet.
    StoragePatch(String),
    SkillModel(String),
    HeroModel(String),
    /// Sealed display source from a fresh owner snapshot; never a receipt.
    HeroOwnerSnapshot(crate::npc_purchase_economy::NativeHeroOwnerUpdate),
    HeroModelReceipt(String),
    SkillModelReceipt(String),
    SocialModel(String),
    EntityRenderAtlas {
        key: String,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
}

#[derive(Default)]
struct NativeInboundBuffer {
    active: bool,
    npc_economy_gate: Option<crate::npc_purchase_economy::NativeNpcEconomyGate>,
    pending: VecDeque<NativeInboundMessage>,
    /// Highest-priority single-slot receipt reserve. It is outside the normal
    /// critical FIFO so no snapshot/ACK/social flood can evict it.
    game_shop_receipt: Option<String>,
    /// One structurally-valid, uncorrelated parcel postage reply that could
    /// not enter the saturated critical FIFO. Keeping it outside the FIFO
    /// prevents a lost reply from permanently reserving the UI's single
    /// in-flight quote, without replacing any transaction receipt.
    mail_cost_reserve: Option<(MailServiceStreamEpoch, u32)>,
    mail_quote_receipt_reserve:Option<MailQuoteReceipt>,
    /// Bounded by per-kind quotas. Every reserved mail terminal joins this
    /// tail at acceptance, so a later send cannot overtake an old Cost.
    mail_terminal_tail:VecDeque<NativeInboundMessage>,
    mail_stream_failed:bool,
    mail_stream_epoch: Option<MailServiceStreamEpoch>,
    mail_stream_marker: Option<MailServiceStreamStarted>,
}

impl NativeInboundBuffer {
    fn enqueue(&mut self, message: NativeInboundMessage) -> bool {
        self.enqueue_with_limits(message, MAX_NATIVE_MESSAGE_BYTES, MAX_NATIVE_BUFFER_BYTES)
    }

    fn enqueue_with_limits(
        &mut self,
        message: NativeInboundMessage,
        max_message_bytes: usize,
        max_buffer_bytes: usize,
    ) -> bool {
        if !self.active {
            return false;
        }
        if let NativeInboundMessage::MailServiceStreamStarted(marker) = &message {
            return self.start_mail_stream(*marker, max_buffer_bytes);
        }
        if let NativeInboundMessage::MailService(delivery) = &message {
            if self.mail_stream_failed || !delivery.epoch.is_valid() || self.mail_stream_epoch != Some(delivery.epoch) {
                return false;
            }
        }
        if let NativeInboundMessage::MailQuoteReceipt(receipt)=&message {
            if self.mail_stream_failed||!receipt.ticket.is_valid()||self.mail_stream_epoch!=Some(receipt.ticket.epoch())||self.mail_cost_reserve.is_some()||self.mail_quote_receipt_reserve.is_some()||self.pending.iter().any(|queued|is_valid_mail_cost(queued)||matches!(queued,NativeInboundMessage::MailQuoteReceipt(_))){return false;}
        }
        let send_ticket=match &message{NativeInboundMessage::MailSendReceipt(receipt)=>Some(receipt.ticket),NativeInboundMessage::MailSendAcknowledgement(ack) if matches!(ack.result,1|-1)=>Some(ack.ticket),NativeInboundMessage::MailSendAcknowledgement(_)=>return false,_=>None};
        if let Some(ticket)=send_ticket {
            if self.mail_stream_failed||!ticket.is_valid()||self.mail_stream_epoch!=Some(ticket.epoch()){return false;}
            let limit=if matches!(message,NativeInboundMessage::MailSendReceipt(_)){2}else{1};
            if self.pending.iter().chain(self.mail_terminal_tail.iter()).filter(|queued|same_send_terminal_kind(&message,queued)).count()>=limit{return false;}
        }
        let message_bytes = native_message_bytes(&message);
        if message_bytes > max_message_bytes {
            return false;
        }
        if matches!(message, NativeInboundMessage::NpcEconomyBundle(_))
            && self.message_count() >= MAX_NATIVE_MESSAGES { return false; }
        // Ordinary traffic leaves room for the tagged terminal Cost reserve.
        // Its epoch is retained/accounted even under payload byte pressure.
        let max_buffer_bytes = if self.mail_stream_epoch.is_some()
            && self.mail_cost_reserve.is_none() && !is_valid_mail_cost(&message)
        {
            max_buffer_bytes.saturating_sub(std::mem::size_of::<(MailServiceStreamEpoch, u32)>())
        } else { max_buffer_bytes };
        let max_buffer_bytes=if self.mail_stream_epoch.is_some()&&self.mail_quote_receipt_reserve.is_none()&&!self.pending.iter().any(|queued|matches!(queued,NativeInboundMessage::MailQuoteReceipt(_)))&&!matches!(message,NativeInboundMessage::MailQuoteReceipt(_)) {
            max_buffer_bytes.saturating_sub(std::mem::size_of::<MailQuoteReceipt>())
        }else{max_buffer_bytes};
        let max_buffer_bytes=if self.mail_stream_epoch.is_some(){
            let receipts=self.pending.iter().chain(self.mail_terminal_tail.iter()).filter(|queued|matches!(queued,NativeInboundMessage::MailSendReceipt(_))).count()+usize::from(matches!(message,NativeInboundMessage::MailSendReceipt(_)));
            let acks=self.pending.iter().chain(self.mail_terminal_tail.iter()).filter(|queued|matches!(queued,NativeInboundMessage::MailSendAcknowledgement(_))).count()+usize::from(matches!(message,NativeInboundMessage::MailSendAcknowledgement(_)));
            max_buffer_bytes.saturating_sub(2_usize.saturating_sub(receipts)*std::mem::size_of::<MailSendReceipt>()).saturating_sub(1_usize.saturating_sub(acks)*std::mem::size_of::<MailSendAcknowledgement>())
        }else{max_buffer_bytes};

        // A bundle seals dedicated terminal payloads into the normal FIFO.
        // Cost's dedicated representation is smaller than MailServiceDelivery;
        // reserve that materialization delta before any eviction or mutation.
        if matches!(message, NativeInboundMessage::NpcEconomyBundle(_)) {
            let sealed_bytes = self.mail_terminal_tail.iter().fold(self.pending_bytes(), |bytes, terminal|
                bytes.saturating_add(native_message_bytes(terminal).saturating_sub(mail_reserved_bytes(terminal))));
            if sealed_bytes.saturating_add(message_bytes) > max_buffer_bytes { return false; }
        }

        let message = match message {
            NativeInboundMessage::GameShopReceipt(json) => {
                return self.enqueue_game_shop_receipt(json, message_bytes, max_buffer_bytes);
            }
            NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt) => {
                if !receipt.is_valid() {
                    return false;
                }
                let Ok(json) = serde_json::to_string(&receipt) else {
                    return false;
                };
                self.retire_npc_economy();
                self.pending.retain(|queued| is_process_lifetime_asset_message(queued) || is_protected_mail_terminal(queued));
                self.game_shop_receipt = Some(json);
                self.pending.push_back(
                    NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt),
                );
                return true;
            }
            other => other,
        };

        // A reserved Cost is ordered after every MailService event already in
        // `pending`. Do not admit a later service event ahead of it.
        if matches!(&message, NativeInboundMessage::MailService(_))
            && self.mail_cost_reserve.is_some()
        {
            return false;
        }
        if self.mail_quote_receipt_reserve.is_some()&&matches!(&message,NativeInboundMessage::MailService(_)) {
            return match &message {NativeInboundMessage::MailService(MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost}})=>self.reserve_mail_cost((*epoch,*cost),max_buffer_bytes),_=>false};
        }
        let mail_cost = match &message {
            NativeInboundMessage::MailService(delivery) => match delivery.event {
                MailServiceEvent::Cost { cost } => Some((delivery.epoch, cost)),
                _ => None,
            },
            _ => None,
        };
        // Crystal carries no quote request ID. The UI sends only one Cost
        // request at a time, so retaining a second queued Cost would make its
        // reply ambiguous and let a later receipt compete with the first.
        if mail_cost.is_some() && self.pending.iter().any(is_valid_mail_cost) {
            return false;
        }
        if !self.mail_terminal_tail.is_empty()&&is_mail_message(&message){
            return if is_protected_mail_terminal(&message){self.reserve_mail_terminal(message,max_buffer_bytes)}else{false};
        }

        // Reset barriers must never compete with snapshots or acknowledgements
        // for capacity. A newer DataReset dominates every queued model and
        // barrier. A newer SceneReset dominates queued scene presentation but
        // deliberately preserves personal/session models and DataReset.
        // Immutable entity atlas uploads survive both boundaries because the
        // native host sends them only once per process.
        match &message {
            NativeInboundMessage::DataReset => {
                self.retire_npc_economy();
                self.pending.retain(|queued| is_process_lifetime_asset_message(queued) || is_protected_mail_terminal(queued));
                self.game_shop_receipt = None;
                self.pending.push_back(message);
                return true;
            }
            NativeInboundMessage::SceneReset => {
                self.retire_npc_economy();
                self.pending.retain(|queued| {
                    !is_scene_resettable_message(queued)
                        && !matches!(queued, NativeInboundMessage::SceneReset)
                });
                while self.normal_message_count() >= MAX_NATIVE_MESSAGES
                    || self.pending_bytes().saturating_add(message_bytes) > max_buffer_bytes
                {
                    if !self.evict_oldest_non_ack_non_boundary()
                        && !self.evict_oldest_non_boundary()
                    {
                        return false;
                    }
                }
                self.pending.push_back(message);
                return true;
            }
            _ => {}
        }

        if is_coalescible_snapshot(&message) {
            // Never move a post-reset snapshot into the pre-reset segment.
            // The reset consumer must still be able to discard the old scene
            // without accidentally discarding the replacement as well.
            let segment_start = self
                .pending
                .iter()
                .rposition(|queued| {
                    matches!(
                        queued,
                        NativeInboundMessage::DataReset
                            | NativeInboundMessage::NpcEconomyBundle(_)
                            | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
                            | NativeInboundMessage::SceneReset
                    )
                })
                .map_or(0, |index| index + 1);
            if let Some(index) = self
                .pending
                .iter()
                .skip(segment_start)
                .position(|queued| same_coalescing_slot(queued, &message))
                .map(|index| index + segment_start)
            {
                self.pending.remove(index);
            }

            while self.coalesced_snapshot_count() >= MAX_COALESCED_SNAPSHOTS
                || self.normal_message_count() >= NON_CRITICAL_MESSAGE_LIMIT
            {
                if !self.evict_oldest_coalescible_snapshot() {
                    return false;
                }
            }
        } else if is_operation_ack(&message) {
            if self
                .pending
                .iter()
                .filter(|queued| is_operation_ack(queued))
                .count()
                >= MAX_OPERATION_ACK_MESSAGES
            {
                return false;
            }
            while self.normal_message_count() >= MAX_NATIVE_MESSAGES
                || self.pending_bytes().saturating_add(message_bytes) > max_buffer_bytes
            {
                if !self.evict_oldest_non_ack_non_boundary() {
                    return false;
                }
            }
        } else if is_critical_message(&message) {
            while self.normal_message_count() >= MAX_NATIVE_MESSAGES {
                if !self.evict_oldest_non_critical() {
                    if let Some(cost) = mail_cost {
                        return self.reserve_mail_cost(cost, max_buffer_bytes);
                    }
                    if let NativeInboundMessage::MailQuoteReceipt(receipt)=&message {return self.reserve_mail_quote_receipt(*receipt,max_buffer_bytes);}
                    if send_ticket.is_some(){return self.reserve_mail_terminal(message,max_buffer_bytes);}
                    return false;
                }
            }
        } else if self.normal_message_count() >= NON_CRITICAL_MESSAGE_LIMIT {
            return false;
        }

        while self.pending_bytes().saturating_add(message_bytes) > max_buffer_bytes {
            let evicted = if is_critical_message(&message) {
                self.evict_oldest_non_critical()
            } else {
                self.evict_oldest_coalescible_snapshot()
            };
            if !evicted {
                if let Some(cost) = mail_cost {
                    return self.reserve_mail_cost(cost, max_buffer_bytes);
                }
                if let NativeInboundMessage::MailQuoteReceipt(receipt)=&message {return self.reserve_mail_quote_receipt(*receipt,max_buffer_bytes);}
                    if send_ticket.is_some(){return self.reserve_mail_terminal(message,max_buffer_bytes);}
                return false;
            }
        }

        if let NativeInboundMessage::NpcEconomyBundle(bundle) = &message {
            if let Some(previous) = &self.npc_economy_gate {
                if !previous.same_gate(bundle.gate()) { previous.retire(); }
            }
            self.npc_economy_gate = Some(bundle.gate().clone());
            // Seal all earlier dedicated reserves into the prefix. Subsequent
            // reserves remain outside the FIFO until this barrier is consumed.
            if let Some(marker) = self.mail_stream_marker.take() {
                self.pending.push_front(NativeInboundMessage::MailServiceStreamStarted(marker));
            }
            if let Some(json) = self.game_shop_receipt.take() {
                self.pending.push_back(NativeInboundMessage::GameShopReceipt(json));
            }
            self.pending.append(&mut self.mail_terminal_tail);
            self.mail_cost_reserve = None;
            self.mail_quote_receipt_reserve = None;
        }
        self.pending.push_back(message);
        true
    }

    fn retire_npc_economy(&mut self) {
        if let Some(gate) = self.npc_economy_gate.take() { gate.retire(); }
        for message in &self.pending {
            if let NativeInboundMessage::NpcEconomyBundle(bundle) = message { bundle.retire(); }
        }
    }

    fn start_mail_stream(&mut self, marker: MailServiceStreamStarted, max_buffer_bytes: usize) -> bool {
        if !marker.epoch.is_valid() || self.mail_stream_epoch.is_some_and(|current| marker.epoch < current) {
            return false;
        }
        if self.mail_stream_epoch == Some(marker.epoch) { return true; }
        // The marker owns a dedicated fixed slot, so a full normal FIFO cannot
        // reject/evict it. Reserve bytes before publishing the new high-water.
        let marker_bytes = std::mem::size_of::<MailServiceStreamStarted>();
        let reserve_bytes=mail_terminal_reserve_bytes();
        if max_buffer_bytes < marker_bytes.saturating_add(reserve_bytes) { return false; }
        let target_bytes=max_buffer_bytes-reserve_bytes;
        // Preflight using sizes/indices only: never clone large atlas/JSON
        // payloads, and leave the old stream wholly intact on rejection.
        let mut retained_bytes=self.pending.iter()
            .filter(|message| !matches!(message,NativeInboundMessage::MailService(_)|NativeInboundMessage::MailQuoteReceipt(_)|NativeInboundMessage::MailSendReceipt(_)|NativeInboundMessage::MailSendAcknowledgement(_)))
            .fold(marker_bytes,|bytes,message|bytes.saturating_add(native_message_bytes(message)))
            .saturating_add(self.game_shop_receipt.as_ref().map_or(0,String::capacity));
        let mut evicted=Vec::new();
        for (index,message) in self.pending.iter().enumerate() {
            if retained_bytes<=target_bytes {break;}
            if !is_operation_ack(message) && !matches!(message,NativeInboundMessage::MailService(_)|NativeInboundMessage::MailQuoteReceipt(_)|NativeInboundMessage::MailSendReceipt(_)|NativeInboundMessage::MailSendAcknowledgement(_)
                | NativeInboundMessage::DataReset | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
                | NativeInboundMessage::SceneReset | NativeInboundMessage::MailServiceStreamStarted(_)
                | NativeInboundMessage::GameShopReceipt(_) | NativeInboundMessage::NpcEconomyBundle(_)) {
                retained_bytes=retained_bytes.saturating_sub(native_message_bytes(message));
                evicted.push(index);
            }
        }
        if retained_bytes>target_bytes {return false;}
        let mut index=0;
        self.pending.retain(|message| {
            let keep=!matches!(message,NativeInboundMessage::MailService(_)|NativeInboundMessage::MailQuoteReceipt(_)|NativeInboundMessage::MailSendReceipt(_)|NativeInboundMessage::MailSendAcknowledgement(_))&&!evicted.contains(&index);
            index+=1;keep
        });
        self.mail_cost_reserve = None;
        self.mail_quote_receipt_reserve=None;self.mail_terminal_tail.clear();self.mail_stream_failed=false;
        self.mail_stream_epoch = Some(marker.epoch);
        self.mail_stream_marker = Some(marker);
        true
    }

    fn reserve_mail_cost(&mut self, cost: (MailServiceStreamEpoch, u32), max_buffer_bytes: usize) -> bool {
        self.reserve_mail_terminal(NativeInboundMessage::MailService(MailServiceDelivery{epoch:cost.0,event:MailServiceEvent::Cost{cost:cost.1}}),max_buffer_bytes)
    }
    fn reserve_mail_quote_receipt(&mut self,receipt:MailQuoteReceipt,max_buffer_bytes:usize)->bool {
        self.reserve_mail_terminal(NativeInboundMessage::MailQuoteReceipt(receipt),max_buffer_bytes)
    }
    fn reserve_mail_terminal(&mut self,message:NativeInboundMessage,max_buffer_bytes:usize)->bool {
        let (_,kind_bytes)=mail_terminal_quota(&message);self.reserve_mail_terminal_with_kind_budget(message,max_buffer_bytes,kind_bytes)
    }
    fn reserve_mail_terminal_with_kind_budget(&mut self,message:NativeInboundMessage,max_buffer_bytes:usize,kind_bytes:usize)->bool {
        let (count_limit,_)=mail_terminal_quota(&message);
        let same_kind=self.mail_terminal_tail.iter().filter(|queued|mail_terminal_kind(queued)==mail_terminal_kind(&message));
        let count=same_kind.clone().count();let bytes=same_kind.map(mail_reserved_bytes).sum::<usize>();
        if count_limit==0||count>=count_limit||bytes.saturating_add(mail_reserved_bytes(&message))>kind_bytes{return false;}
        if self.pending_bytes().saturating_add(mail_reserved_bytes(&message))>max_buffer_bytes{return false;}
        match &message{
            NativeInboundMessage::MailService(MailServiceDelivery{epoch,event:MailServiceEvent::Cost{cost}})=>{if self.mail_cost_reserve.is_some(){return false;}self.mail_cost_reserve=Some((*epoch,*cost));}
            NativeInboundMessage::MailQuoteReceipt(receipt)=>{if self.mail_quote_receipt_reserve.is_some(){return false;}self.mail_quote_receipt_reserve=Some(*receipt);}
            NativeInboundMessage::MailSendReceipt(_)|NativeInboundMessage::MailSendAcknowledgement(_)=>{},_=>return false,
        }
        self.mail_terminal_tail.push_back(message);true
    }

    fn enqueue_game_shop_receipt(
        &mut self,
        json: String,
        message_bytes: usize,
        max_buffer_bytes: usize,
    ) -> bool {
        let Ok(receipt) =
            serde_json::from_str::<mir2_client_bevy::game_shop::GameShopReceipt>(&json)
        else {
            return false;
        };
        if !receipt.is_valid() {
            return false;
        }

        // The Windows connection owner performs the request correlation
        // before a receipt reaches this reserve. Once occupied, keep the
        // first accepted receipt until the runtime drains it. Replacing it
        // with a later structurally-valid but unrelated receipt can turn an
        // already-delivered exact acknowledgement into a permanent pending
        // purchase.
        if self.game_shop_receipt.is_some() {
            return false;
        }

        while self.pending.len().saturating_add(1) > MAX_NATIVE_MESSAGES
            || self.pending_bytes().saturating_add(message_bytes) > max_buffer_bytes
        {
            if !self.evict_oldest_non_boundary() {
                return false;
            }
        }
        self.game_shop_receipt = Some(json);
        true
    }

    fn message_count(&self) -> usize {
        self.normal_message_count().saturating_add(usize::from(self.mail_stream_marker.is_some()))
    }

    fn normal_message_count(&self) -> usize {
        self.pending
            .len()
            .saturating_add(usize::from(self.game_shop_receipt.is_some()))
            .saturating_add(self.mail_terminal_tail.len())
    }

    fn pending_bytes(&self) -> usize {
        self.pending
            .iter()
            .fold(0_usize, |total, message| {
                total.saturating_add(native_message_bytes(message))
            })
            .saturating_add(self.game_shop_receipt.as_ref().map_or(0, String::capacity))
            .saturating_add(self.mail_terminal_tail.iter().map(mail_reserved_bytes).sum::<usize>())
            .saturating_add(usize::from(self.mail_stream_marker.is_some()) * std::mem::size_of::<MailServiceStreamStarted>())
    }

    fn coalesced_snapshot_count(&self) -> usize {
        self.pending
            .iter()
            .filter(|message| is_coalescible_snapshot(message))
            .count()
    }

    fn evict_oldest_coalescible_snapshot(&mut self) -> bool {
        let Some(index) = self.pending.iter().position(is_coalescible_snapshot) else {
            return false;
        };
        self.pending.remove(index);
        true
    }

    fn evict_oldest_non_critical(&mut self) -> bool {
        let Some(index) = self
            .pending
            .iter()
            .position(|message| !is_critical_message(message))
        else {
            return false;
        };
        self.pending.remove(index);
        true
    }

    fn evict_oldest_non_ack_non_boundary(&mut self) -> bool {
        let Some(index) = self.pending.iter().position(|message| {
            !is_operation_ack(message)
                && !is_protected_mail_terminal(message)
                && !matches!(
                    message,
                    NativeInboundMessage::DataReset
                        | NativeInboundMessage::NpcEconomyBundle(_)
                        | NativeInboundMessage::GameShopReceipt(_)
                        | NativeInboundMessage::MailServiceStreamStarted(_)
                        | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
                        | NativeInboundMessage::SceneReset
                )
        }) else {
            return false;
        };
        self.pending.remove(index);
        true
    }

    fn evict_oldest_non_boundary(&mut self) -> bool {
        let Some(index) = self.pending.iter().position(|message| {
            !is_protected_mail_terminal(message)
                && !matches!(
                    message,
                    NativeInboundMessage::DataReset
                        | NativeInboundMessage::NpcEconomyBundle(_)
                        | NativeInboundMessage::GameShopReceipt(_)
                        | NativeInboundMessage::MailServiceStreamStarted(_)
                        | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
                        | NativeInboundMessage::SceneReset
                )
        }) else {
            return false;
        };
        self.pending.remove(index);
        true
    }
}

fn is_valid_mail_cost(message: &NativeInboundMessage) -> bool {
    matches!(message, NativeInboundMessage::MailService(MailServiceDelivery { event: MailServiceEvent::Cost { .. }, .. }))
}
fn is_protected_mail_terminal(message:&NativeInboundMessage)->bool {is_valid_mail_cost(message)||matches!(message,NativeInboundMessage::MailQuoteReceipt(_)|NativeInboundMessage::MailSendReceipt(_)|NativeInboundMessage::MailSendAcknowledgement(_))}
fn is_mail_message(message:&NativeInboundMessage)->bool{matches!(message,NativeInboundMessage::MailService(_)|NativeInboundMessage::MailServiceStreamStarted(_))||is_protected_mail_terminal(message)}
fn same_send_terminal_kind(a:&NativeInboundMessage,b:&NativeInboundMessage)->bool{matches!((a,b),(NativeInboundMessage::MailSendReceipt(_),NativeInboundMessage::MailSendReceipt(_))|(NativeInboundMessage::MailSendAcknowledgement(_),NativeInboundMessage::MailSendAcknowledgement(_)))}
fn mail_reserved_bytes(message:&NativeInboundMessage)->usize{if is_valid_mail_cost(message){std::mem::size_of::<(MailServiceStreamEpoch,u32)>()}else{native_message_bytes(message)}}
fn mail_terminal_reserve_bytes()->usize{std::mem::size_of::<(MailServiceStreamEpoch,u32)>()+std::mem::size_of::<MailQuoteReceipt>()+2*std::mem::size_of::<MailSendReceipt>()+std::mem::size_of::<MailSendAcknowledgement>()}
fn mail_terminal_kind(message:&NativeInboundMessage)->u8{match message{NativeInboundMessage::MailService(MailServiceDelivery{event:MailServiceEvent::Cost{..},..})=>1,NativeInboundMessage::MailQuoteReceipt(_)=>2,NativeInboundMessage::MailSendReceipt(_)=>3,NativeInboundMessage::MailSendAcknowledgement(_)=>4,_=>0}}
fn mail_terminal_quota(message:&NativeInboundMessage)->(usize,usize){let count=match mail_terminal_kind(message){1|2|4=>1,3=>2,_=>0};(count,count*mail_reserved_bytes(message))}

fn make_buffer() -> Arc<Mutex<NativeInboundBuffer>> {
    let buffer = Arc::new(Mutex::new(NativeInboundBuffer {
        active: true,
        npc_economy_gate: None,
        pending: VecDeque::new(),
        game_shop_receipt: None,
        mail_cost_reserve: None,
        mail_quote_receipt_reserve:None,mail_terminal_tail:VecDeque::new(),mail_stream_failed:false,
        mail_stream_epoch: None,
        mail_stream_marker: None,
    }));
    let mut slot = NATIVE_QUEUE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("native queue mutex should not be poisoned");
    if let Some(previous) = slot.replace(Arc::clone(&buffer)) {
        let mut previous = previous.lock().expect("native inbound buffer mutex should not be poisoned");
        previous.active = false;
        previous.retire_npc_economy();
        previous.pending.clear();
    }
    buffer
}

fn send_native(message: NativeInboundMessage) -> bool {
    let queue = NATIVE_QUEUE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("native queue mutex should not be poisoned")
        .clone();
    queue
        .map(|queue| {
            queue
                .lock()
                .expect("native inbound buffer mutex should not be poisoned")
                .enqueue(message)
        })
        .unwrap_or(false)
}

fn is_coalescible_snapshot(message: &NativeInboundMessage) -> bool {
    matches!(
        message,
        NativeInboundMessage::WorldState(_)
            | NativeInboundMessage::EntityRenderState(_)
            | NativeInboundMessage::EffectRenderState(_)
            | NativeInboundMessage::LightingRenderState(_)
            | NativeInboundMessage::MapRenderState(_)
            | NativeInboundMessage::UiReadModel(_)
            | NativeInboundMessage::MapModel(_)
            | NativeInboundMessage::EntityModelSet(_)
            | NativeInboundMessage::InventoryModel(_)
            | NativeInboundMessage::MailModel(_)
            | NativeInboundMessage::ShopModel(_)
            | NativeInboundMessage::StorageModel(_)
            | NativeInboundMessage::StorageItems(_)
            | NativeInboundMessage::HeroModel(_)
            | NativeInboundMessage::HeroOwnerSnapshot(_)
            | NativeInboundMessage::SkillModel(_)
            | NativeInboundMessage::EntityRenderAtlas { .. }
    )
}

fn same_coalescing_slot(left: &NativeInboundMessage, right: &NativeInboundMessage) -> bool {
    match (left, right) {
        (NativeInboundMessage::HeroOwnerSnapshot(left),NativeInboundMessage::HeroOwnerSnapshot(right))=>left.same_source(right),
        (NativeInboundMessage::WorldState(_), NativeInboundMessage::WorldState(_))
        | (
            NativeInboundMessage::EntityRenderState(_),
            NativeInboundMessage::EntityRenderState(_),
        )
        | (
            NativeInboundMessage::EffectRenderState(_),
            NativeInboundMessage::EffectRenderState(_),
        )
        | (
            NativeInboundMessage::LightingRenderState(_),
            NativeInboundMessage::LightingRenderState(_),
        )
        | (NativeInboundMessage::MapRenderState(_), NativeInboundMessage::MapRenderState(_))
        | (NativeInboundMessage::UiReadModel(_), NativeInboundMessage::UiReadModel(_))
        | (NativeInboundMessage::MapModel(_), NativeInboundMessage::MapModel(_))
        | (NativeInboundMessage::EntityModelSet(_), NativeInboundMessage::EntityModelSet(_))
        | (NativeInboundMessage::InventoryModel(_), NativeInboundMessage::InventoryModel(_))
        | (NativeInboundMessage::MailModel(_), NativeInboundMessage::MailModel(_))
        | (NativeInboundMessage::ShopModel(_), NativeInboundMessage::ShopModel(_))
        | (NativeInboundMessage::StorageModel(_), NativeInboundMessage::StorageModel(_))
        | (NativeInboundMessage::StorageItems(_), NativeInboundMessage::StorageItems(_))
        | (NativeInboundMessage::HeroModel(_), NativeInboundMessage::HeroModel(_))
        | (NativeInboundMessage::SkillModel(_), NativeInboundMessage::SkillModel(_)) => true,
        (
            NativeInboundMessage::EntityRenderAtlas { key: left, .. },
            NativeInboundMessage::EntityRenderAtlas { key: right, .. },
        ) => left == right,
        _ => false,
    }
}

fn is_critical_message(message: &NativeInboundMessage) -> bool {
    matches!(
        message,
        NativeInboundMessage::InventoryOperationAck(_)
            | NativeInboundMessage::NpcEconomyBundle(_)
            | NativeInboundMessage::DataReset
            | NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_)
            | NativeInboundMessage::SceneReset
            | NativeInboundMessage::WalletPatch(_)
            | NativeInboundMessage::GameShopInfo(_)
            | NativeInboundMessage::GameShopStock(_)
            | NativeInboundMessage::GameShopReceipt(_)
            | NativeInboundMessage::MailService(_)
            | NativeInboundMessage::MailServiceStreamStarted(_)
            | NativeInboundMessage::MailQuoteReceipt(_)
            | NativeInboundMessage::MailSendReceipt(_)
            | NativeInboundMessage::MailSendAcknowledgement(_)
            | NativeInboundMessage::NpcShopService(_)
            | NativeInboundMessage::StoragePatch(_)
            | NativeInboundMessage::SocialModel(_)
            | NativeInboundMessage::HeroModelReceipt(_)
            | NativeInboundMessage::SkillModelReceipt(_)
    )
}

fn is_operation_ack(message: &NativeInboundMessage) -> bool {
    matches!(
        message,
        NativeInboundMessage::InventoryOperationAck(_)
            | NativeInboundMessage::HeroModelReceipt(_)
            | NativeInboundMessage::SkillModelReceipt(_)
    )
}

fn native_message_bytes(message: &NativeInboundMessage) -> usize {
    match message {
        NativeInboundMessage::NpcEconomyBundle(bundle) => bundle.retained_bytes(),
        NativeInboundMessage::HeroOwnerSnapshot(update)=>update.retained_bytes(),
        NativeInboundMessage::WorldState(json)
        | NativeInboundMessage::EntityRenderState(json)
        | NativeInboundMessage::EffectRenderState(json)
        | NativeInboundMessage::LightingRenderState(json)
        | NativeInboundMessage::MapRenderState(json)
        | NativeInboundMessage::UiReadModel(json)
        | NativeInboundMessage::WalletPatch(json)
        | NativeInboundMessage::MapModel(json)
        | NativeInboundMessage::EntityModelSet(json)
        | NativeInboundMessage::InventoryModel(json)
        | NativeInboundMessage::InventoryOperationAck(json)
        | NativeInboundMessage::ChatLine(json)
        | NativeInboundMessage::MailModel(json)
        | NativeInboundMessage::ShopModel(json)
        | NativeInboundMessage::GameShopInfo(json)
        | NativeInboundMessage::GameShopStock(json)
        | NativeInboundMessage::GameShopReceipt(json)
        | NativeInboundMessage::NpcShopService(json)
        | NativeInboundMessage::StorageModel(json)
        | NativeInboundMessage::StorageItems(json)
        | NativeInboundMessage::StoragePatch(json)
        | NativeInboundMessage::HeroModel(json)
        | NativeInboundMessage::SkillModel(json)
        | NativeInboundMessage::HeroModelReceipt(json)
        | NativeInboundMessage::SkillModelReceipt(json)
        | NativeInboundMessage::SocialModel(json) => json.capacity(),
        NativeInboundMessage::EntityRenderAtlas { key, pixels, .. } => {
            key.capacity().saturating_add(pixels.capacity())
        }
        NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt) => {
            serde_json::to_string(receipt).map_or(usize::MAX, |json| json.len())
        }
        NativeInboundMessage::DataReset | NativeInboundMessage::SceneReset => 0,
        NativeInboundMessage::MailService(delivery) => std::mem::size_of_val(delivery),
        NativeInboundMessage::MailServiceStreamStarted(marker) => std::mem::size_of_val(marker),
        NativeInboundMessage::MailQuoteReceipt(receipt)=>std::mem::size_of_val(receipt),
        NativeInboundMessage::MailSendReceipt(receipt)=>std::mem::size_of_val(receipt),
        NativeInboundMessage::MailSendAcknowledgement(ack)=>std::mem::size_of_val(ack),
    }
}

/// Native-host entry point: push a world-state snapshot JSON to the Bevy loop.
///
/// Safe to call from any thread after the runtime app has been built. Returns
/// `false` when no runtime app is currently running (queue not registered).
pub fn push_native_world_state(json: String) -> bool {
    send_native(NativeInboundMessage::WorldState(json))
}

/// Admission only. The gate completion is emitted after all economic models
/// have been committed by the main-thread exclusive consumer.
pub fn push_native_npc_economy_bundle(bundle: crate::npc_purchase_economy::NativeNpcEconomyBundle) -> bool {
    send_native(NativeInboundMessage::NpcEconomyBundle(bundle))
}

/// Native-host entry point: push an entity-render-state snapshot JSON.
pub fn push_native_entity_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::EntityRenderState(json))
}

/// Native-host entry point: push a scene-effect render-state snapshot JSON.
///
/// The payload mirrors the WASM setMir2EffectRenderState contract so the
/// shared runtime renders effect sprites identically on Windows and Web.
pub fn push_native_effect_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::EffectRenderState(json))
}

/// Native-host entry point: push the bounded Crystal lighting render state.
/// The runtime owns validation and never lets a producer retain more than 200
/// map/entity light layers.
pub fn push_native_lighting_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::LightingRenderState(json))
}

/// Native-host entry point: push a map-render-state snapshot JSON.
pub fn push_native_map_render_state(json: String) -> bool {
    send_native(NativeInboundMessage::MapRenderState(json))
}

/// Native-host entry point: push a UI read model (HUD stats) JSON.
///
/// The payload mirrors `mir2-client-bevy::read_model::UiReadModel` so the shared
/// HUD renders the same values on every host.
pub fn push_native_ui_read_model(json: String) -> bool {
    send_native(NativeInboundMessage::UiReadModel(json))
}

/// Native-host entry point: apply a packet-first `{gold?, credit?}` wallet
/// patch to the shared read models.
pub fn push_native_wallet_patch(json: String) -> bool {
    send_native(NativeInboundMessage::WalletPatch(json))
}

/// Native-host entry point: push a map model (terrain patches) JSON.
///
/// The payload mirrors `mir2-client-bevy::map::MapModel` so the shared map
/// renderer draws the same terrain on every host.
pub fn push_native_map_model(json: String) -> bool {
    send_native(NativeInboundMessage::MapModel(json))
}

/// Native-host entry point: push an entity model set JSON.
///
/// The payload mirrors `mir2-client-bevy::entities::EntityModelSet` so the
/// shared entity renderer draws the same entities on every host.
pub fn push_native_entity_model_set(json: String) -> bool {
    send_native(NativeInboundMessage::EntityModelSet(json))
}

/// Native-host entry point: push an inventory model JSON.
///
/// The payload mirrors `mir2-client-bevy::inventory::InventoryModel`.
pub fn push_native_inventory_model(json: String) -> bool {
    send_native(NativeInboundMessage::InventoryModel(json))
}

/// Native-host entry point for a correlatable inventory operation ACK/NACK.
pub fn push_native_inventory_operation_ack(json: String) -> bool {
    send_native(NativeInboundMessage::InventoryOperationAck(json))
}

/// Native-host entry point: push a single chat line JSON.
///
/// The payload mirrors `mir2-client-bevy::chat::ChatLine`.
pub fn push_native_chat_line(json: String) -> bool {
    send_native(NativeInboundMessage::ChatLine(json))
}

/// Native-host entry point: clear character/session read models and pending UI
/// operations at logout or a disconnected session. Map changes use the
/// narrower [`push_native_scene_reset`] path.
pub fn push_native_data_reset() -> bool {
    send_native(NativeInboundMessage::DataReset)
}

/// Native-host entry point for a terminal session reset that must preserve one
/// exact, already-accepted GameShop result while clearing every other session
/// model. The receipt is validated before the queue is mutated.
pub fn push_native_data_reset_preserving_exact_game_shop_receipt(
    receipt: mir2_client_bevy::game_shop::GameShopReceipt,
) -> bool {
    send_native(NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt))
}

/// Native-host entry point: clear only retained scene/world presentation state.
pub fn push_native_scene_reset() -> bool {
    send_native(NativeInboundMessage::SceneReset)
}

/// Native-host entry point: push a mail model JSON.
///
/// The payload mirrors `mir2-client-bevy::mail::MailModel` (or
/// `mir2-client-bevy::crystal_ui::overlays::MailModel` when `native-ui` is
/// enabled) so the Windows Mail panel shows authoritative stage5 mail.
pub fn push_native_mail_model(json: String) -> bool {
    send_native(NativeInboundMessage::MailModel(json))
}

/// Native-host entry point for ordered Crystal `MailSendRequest`, `MailCost`,
/// and `MailLockedItem` events, stamped once by their producing socket.
pub fn push_native_mail_service(delivery: MailServiceDelivery) -> bool {
    send_native(NativeInboundMessage::MailService(delivery))
}

pub fn push_native_mail_service_stream_started(marker: MailServiceStreamStarted) -> bool {
    send_native(NativeInboundMessage::MailServiceStreamStarted(marker))
}
pub fn native_mail_stream_supersedes(epoch:MailServiceStreamEpoch)->bool {
    NATIVE_QUEUE.get().and_then(|slot|slot.lock().ok()?.clone()).and_then(|queue|queue.lock().ok()?.mail_stream_epoch).is_some_and(|current|current>epoch)
}
pub fn native_mail_stream_failed_epoch()->Option<MailServiceStreamEpoch> {
    let queue=NATIVE_QUEUE.get()?.lock().ok()?.clone()?;let state=queue.lock().ok()?;
    if state.mail_stream_failed{state.mail_stream_epoch}else{None}
}
pub fn push_native_mail_quote_receipt(receipt:MailQuoteReceipt)->bool {
    if send_native(NativeInboundMessage::MailQuoteReceipt(receipt)){return true;}
    // A real newer marker is the only alternative terminal for an old ticket.
    if native_mail_stream_supersedes(receipt.ticket.epoch()){return true;}
    if let Some(queue)=NATIVE_QUEUE.get().and_then(|slot|slot.lock().ok()?.clone()) {
        if let Ok(mut state)=queue.lock(){if state.mail_stream_epoch==Some(receipt.ticket.epoch()){state.mail_stream_failed=true;}}
    }
    false
}
fn publish_native_mail_send_terminal(message:NativeInboundMessage,epoch:MailServiceStreamEpoch,admit:impl FnOnce(NativeInboundMessage)->bool)->bool{
    if admit(message)||native_mail_stream_supersedes(epoch){return true;}
    if let Some(queue)=NATIVE_QUEUE.get().and_then(|slot|slot.lock().ok()?.clone()){if let Ok(mut state)=queue.lock(){if state.mail_stream_epoch==Some(epoch){state.mail_stream_failed=true;}}}false
}
pub fn push_native_mail_send_receipt(receipt:MailSendReceipt)->bool{publish_native_mail_send_terminal(NativeInboundMessage::MailSendReceipt(receipt),receipt.ticket.epoch(),send_native)}
pub fn push_native_mail_send_acknowledgement(ack:MailSendAcknowledgement)->bool{publish_native_mail_send_terminal(NativeInboundMessage::MailSendAcknowledgement(ack),ack.ticket.epoch(),send_native)}

/// Native-host entry point: push a shop model JSON.
///
/// The payload mirrors `mir2-client-bevy::shop::ShopModel`.
pub fn push_native_shop_model(json: String) -> bool {
    send_native(NativeInboundMessage::ShopModel(json))
}

/// Native-host entry point: upsert one authoritative GameShopInfo product.
pub fn push_native_game_shop_info(json: String) -> bool {
    send_native(NativeInboundMessage::GameShopInfo(json))
}

/// Native-host entry point: patch one authoritative GameShop stock level.
pub fn push_native_game_shop_stock(json: String) -> bool {
    send_native(NativeInboundMessage::GameShopStock(json))
}

/// Native-host entry point for an exact, receipt-correlated GameShop result.
pub fn push_native_game_shop_receipt(json: String) -> bool {
    send_native(NativeInboundMessage::GameShopReceipt(json))
}

/// Native-host entry point: select one authoritative NPC service surface.
pub fn push_native_npc_shop_service(json: String) -> bool {
    send_native(NativeInboundMessage::NpcShopService(json))
}

/// Native-host entry point: push a storage model JSON.
///
/// The payload mirrors `mir2-client-bevy::storage::StorageModel`.
pub fn push_native_storage_model(json: String) -> bool {
    send_native(NativeInboundMessage::StorageModel(json))
}

/// Native-host entry point: replace only the storage items from a Crystal
/// `UserStorage` packet. The JSON payload is `{ "items": [...] }` using the
/// shared inventory item shape.
pub fn push_native_storage_items(json: String) -> bool {
    send_native(NativeInboundMessage::StorageItems(json))
}

/// Native-host entry point: apply a partial storage metadata update from a
/// Crystal storage result packet.
pub fn push_native_storage_patch(json: String) -> bool {
    send_native(NativeInboundMessage::StoragePatch(json))
}

/// Native-host entry point: push a skill model JSON.
///
/// The payload mirrors `mir2-client-bevy::skill_model::SkillModel`.
pub fn push_native_hero_model(json: String) -> bool {
    let receipt = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .is_some_and(|v| {
            v.get("skillKeyAck").is_some_and(|ack| !ack.is_null())
                || v.get("itemResultReceipt")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
        });
    send_native(if receipt {
        NativeInboundMessage::HeroModelReceipt(json)
    } else {
        NativeInboundMessage::HeroModel(json)
    })
}

pub fn push_native_hero_owner_snapshot(update:crate::npc_purchase_economy::NativeHeroOwnerUpdate)->bool {
    send_native(NativeInboundMessage::HeroOwnerSnapshot(update))
}

pub fn push_native_skill_model(json: String) -> bool {
    let receipt = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|v| {
            v.get("skillKeyAck")
                .or_else(|| v.get("skill_key_ack"))
                .cloned()
        })
        .is_some_and(|v| !v.is_null());
    send_native(if receipt {
        NativeInboundMessage::SkillModelReceipt(json)
    } else {
        NativeInboundMessage::SkillModel(json)
    })
}

/// Native-host entry point: push authoritative Group/Guild/Trade state.
pub fn push_native_social_model(json: String) -> bool {
    send_native(NativeInboundMessage::SocialModel(json))
}

/// Native-host entry point: push a raw RGBA entity atlas image.
///
/// Mirrors the WASM `setMir2EntityRenderAtlas(key, width, height, pixels)`.
pub fn push_native_entity_render_atlas(
    key: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
) -> bool {
    send_native(NativeInboundMessage::EntityRenderAtlas {
        key,
        width,
        height,
        pixels,
    })
}

/// Bevy resource holding the consumer side of the native ingestion queue.
/// The Bevy loop is single-threaded; the mutex is only contended against the
/// background native producer.
#[derive(Resource)]
pub(crate) struct NativeInbound {
    buffer: Arc<Mutex<NativeInboundBuffer>>,
}

impl NativeInbound {
    pub(crate) fn mail_stream_failure(&self)->Option<MailServiceStreamEpoch>{let state=self.buffer.lock().ok()?;if state.mail_stream_failed{state.mail_stream_epoch}else{None}}
    pub(crate) fn fail_mail_stream(&self,epoch:MailServiceStreamEpoch){if let Ok(mut state)=self.buffer.lock(){if state.mail_stream_epoch==Some(epoch){state.mail_stream_failed=true;}}}
    pub(crate) fn new() -> Self {
        Self {
            buffer: make_buffer(),
        }
    }

    /// Snapshot queue occupancy without draining or changing admission state.
    /// This is called only by the opt-in 10-second native soak sampler.
    pub(crate) fn diagnostics(&self) -> NativeInboundDiagnostics {
        let state = self
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");
        NativeInboundDiagnostics {
            message_count: state.message_count(),
            retained_bytes: state.pending_bytes(),
        }
    }

    /// Drain only messages owned by one typed consumer while preserving all
    /// other variants for the later chained consumers in the same frame.
    pub(crate) fn drain_matching(
        &self,
        mut matches: impl FnMut(&NativeInboundMessage) -> bool,
        mut on_message: impl FnMut(NativeInboundMessage),
    ) {
        let matched = {
            let mut state = self
                .buffer
                .lock()
                .expect("native inbound mutex should not be poisoned");

            let mut matched = Vec::new();
            let barrier_pending = state.pending.iter().any(|message| matches!(message, NativeInboundMessage::NpcEconomyBundle(_)));
            if let Some(marker) = if barrier_pending { None } else { state.mail_stream_marker.take() } {
                let message = NativeInboundMessage::MailServiceStreamStarted(marker);
                if matches(&message) { matched.push(message); }
                else { state.mail_stream_marker = Some(marker); }
            }
            if let Some(json) = if barrier_pending { None } else { state.game_shop_receipt.take() } {
                let receipt = NativeInboundMessage::GameShopReceipt(json);
                if matches(&receipt) {
                    matched.push(receipt);
                } else if let NativeInboundMessage::GameShopReceipt(json) = receipt {
                    state.game_shop_receipt = Some(json);
                }
            }
            let mut retained = VecDeque::new();
            let mut mail_blocked=!barrier_pending && state.mail_stream_marker.is_some();
            while let Some(message) = state.pending.pop_front() {
                // A typed consumer must not skip the complete checkpoint and
                // drain a later model before the exclusive bundle consumer.
                if matches!(message, NativeInboundMessage::NpcEconomyBundle(_)) {
                    retained.push_back(message);
                    retained.append(&mut state.pending);
                    mail_blocked = true;
                    break;
                }
                let selected=matches(&message);
                if is_mail_message(&message)&&!selected{mail_blocked=true;}
                if selected && !(mail_blocked
                    && matches!(&message, NativeInboundMessage::MailService(_)|NativeInboundMessage::MailQuoteReceipt(_)|NativeInboundMessage::MailSendReceipt(_)|NativeInboundMessage::MailSendAcknowledgement(_))) {
                    matched.push(message);
                } else {
                    retained.push_back(message);
                }
            }
            state.pending = retained;
            let mut tail=VecDeque::new();
            while let Some(message)=state.mail_terminal_tail.pop_front(){
                if !mail_blocked&&matches(&message){
                    if is_valid_mail_cost(&message){state.mail_cost_reserve=None;}
                    if matches!(&message,NativeInboundMessage::MailQuoteReceipt(_)){state.mail_quote_receipt_reserve=None;}
                    matched.push(message);
                }else{mail_blocked=true;tail.push_back(message);}
            }
            state.mail_terminal_tail=tail;
            matched
        };

        for message in matched {
            on_message(message);
        }
    }

    pub(crate) fn take_npc_economy_front(&self) -> Option<crate::npc_purchase_economy::NativeNpcEconomyBundle> {
        let mut state = self.buffer.lock().ok()?;
        if !matches!(state.pending.front(), Some(NativeInboundMessage::NpcEconomyBundle(_))) { return None; }
        match state.pending.pop_front()? {
            NativeInboundMessage::NpcEconomyBundle(bundle) => Some(bundle),
            _ => unreachable!(),
        }
    }

    /// Drop typed models queued before reset barriers.
    ///
    /// A WebSocket task can enqueue a periodic snapshot immediately before a
    /// logout/map boundary. A SceneReset drops only scene presentation
    /// messages; a DataReset drops owner models but preserves this stream's
    /// Cost and protected marker/high-water. Messages queued after
    /// each barrier remain available for the next scene/session.
    pub(crate) fn discard_stale_data_before_latest_reset(&self) {
        let mut state = self
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");

        let mut retained = VecDeque::new();
        for message in state.pending.drain(..) {
            match &message {
                NativeInboundMessage::DataReset => {
                    retained.retain(|queued| !is_resettable_data_message(queued));
                    retained.push_back(message);
                }
                NativeInboundMessage::DataResetPreservingExactGameShopReceipt(_) => {
                    retained.retain(|queued| !is_resettable_data_message(queued));
                    retained.push_back(message);
                }
                NativeInboundMessage::SceneReset => {
                    retained.retain(|queued| !is_scene_resettable_message(queued));
                    retained.push_back(message);
                }
                _ => retained.push_back(message),
            }
        }
        state.pending = retained;
    }
}

impl Default for NativeInbound {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NativeInbound {
    fn drop(&mut self) {
        let mut buffer = self
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");
        buffer.active = false;
        buffer.retire_npc_economy();
        buffer.pending.clear();
        buffer.game_shop_receipt = None;
        buffer.mail_cost_reserve = None;
        buffer.mail_quote_receipt_reserve=None;buffer.mail_terminal_tail.clear();
        buffer.mail_stream_marker = None;
    }
}

fn is_scene_resettable_message(message: &NativeInboundMessage) -> bool {
    matches!(
        message,
        NativeInboundMessage::WorldState(_)
            | NativeInboundMessage::HeroOwnerSnapshot(_)
            | NativeInboundMessage::NpcEconomyBundle(_)
            | NativeInboundMessage::EntityRenderState(_)
            | NativeInboundMessage::EffectRenderState(_)
            | NativeInboundMessage::LightingRenderState(_)
            | NativeInboundMessage::MapRenderState(_)
            | NativeInboundMessage::MapModel(_)
            | NativeInboundMessage::EntityModelSet(_)
            | NativeInboundMessage::NpcShopService(_)
    )
}

fn is_resettable_data_message(message: &NativeInboundMessage) -> bool {
    if is_protected_mail_terminal(message) { return false; }
    matches!(
        message,
        NativeInboundMessage::WorldState(_)
            | NativeInboundMessage::HeroOwnerSnapshot(_)
            | NativeInboundMessage::NpcEconomyBundle(_)
            | NativeInboundMessage::EntityRenderState(_)
            | NativeInboundMessage::EffectRenderState(_)
            | NativeInboundMessage::LightingRenderState(_)
            | NativeInboundMessage::MapRenderState(_)
            | NativeInboundMessage::MapModel(_)
            | NativeInboundMessage::EntityModelSet(_)
            | NativeInboundMessage::UiReadModel(_)
            | NativeInboundMessage::WalletPatch(_)
            | NativeInboundMessage::InventoryModel(_)
            | NativeInboundMessage::InventoryOperationAck(_)
            | NativeInboundMessage::ChatLine(_)
            | NativeInboundMessage::MailModel(_)
            | NativeInboundMessage::MailService(_)
            | NativeInboundMessage::ShopModel(_)
            | NativeInboundMessage::GameShopInfo(_)
            | NativeInboundMessage::GameShopStock(_)
            | NativeInboundMessage::GameShopReceipt(_)
            | NativeInboundMessage::NpcShopService(_)
            | NativeInboundMessage::StorageModel(_)
            | NativeInboundMessage::StorageItems(_)
            | NativeInboundMessage::StoragePatch(_)
            | NativeInboundMessage::HeroModel(_)
            | NativeInboundMessage::SkillModel(_)
            | NativeInboundMessage::HeroModelReceipt(_)
            | NativeInboundMessage::SkillModelReceipt(_)
            | NativeInboundMessage::SocialModel(_)
    )
}

fn is_process_lifetime_asset_message(message: &NativeInboundMessage) -> bool {
    matches!(message, NativeInboundMessage::EntityRenderAtlas { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_receipt(request_id: &str) -> String {
        format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{request_id}","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency"}}"#
        )
    }

    fn typed_receipt(request_id: &str) -> mir2_client_bevy::game_shop::GameShopReceipt {
        serde_json::from_str(&valid_receipt(request_id)).expect("valid typed receipt")
    }

    fn active_buffer() -> NativeInboundBuffer {
        NativeInboundBuffer {
            active: true,
            npc_economy_gate: None,
            pending: VecDeque::new(),
            game_shop_receipt: None,
            mail_cost_reserve: None,
            mail_quote_receipt_reserve:None,mail_terminal_tail:VecDeque::new(),mail_stream_failed:false,
            mail_stream_epoch: None,
            mail_stream_marker: None,
        }
    }

    fn npc_bundle() -> (crate::npc_purchase_economy::NativeNpcEconomyGate, NativeInboundMessage) {
        let (gate, witness, projection) = crate::npc_purchase_economy::tests::fixture();
        let bundle = gate.prepare(witness, projection).unwrap();
        (gate, NativeInboundMessage::NpcEconomyBundle(bundle))
    }

    #[test]
    fn native_npc_economy_queue_barrier_prevents_later_typed_models_overtaking() {
        let (gate, bundle) = npc_bundle();
        let mut state = active_buffer();
        assert!(state.enqueue(NativeInboundMessage::InventoryModel("old".into())));
        assert!(state.enqueue(bundle));
        assert!(state.enqueue(NativeInboundMessage::UiReadModel("future-ui".into())));
        assert!(state.enqueue(NativeInboundMessage::InventoryModel("future".into())));
        let inbound = NativeInbound { buffer: Arc::new(Mutex::new(state)) };
        let mut drained = Vec::new();
        inbound.drain_matching(|m| matches!(m, NativeInboundMessage::UiReadModel(_)), |m| drained.push(m));
        assert!(drained.is_empty());assert!(inbound.take_npc_economy_front().is_none());
        inbound.drain_matching(|m| matches!(m, NativeInboundMessage::InventoryModel(_)), |m| drained.push(m));
        assert!(matches!(&drained[..], [NativeInboundMessage::InventoryModel(s)] if s == "old"));
        assert_eq!(gate.try_recv_applied(),None);
        let mut world = bevy::prelude::World::new(); world.insert_resource(inbound);
        crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
        assert!(gate.try_recv_applied().is_some());
        let inbound = world.resource::<NativeInbound>();
        let mut future = Vec::new();
        inbound.drain_matching(|m| matches!(m, NativeInboundMessage::InventoryModel(_)), |m| future.push(m));
        assert!(matches!(&future[..], [NativeInboundMessage::InventoryModel(s)] if s == "future"));
    }

    #[cfg(feature="native-npc-economy")]
    #[test]
    fn native_npc_economy_ordinary_hero_packet_keeps_source_xp_until_owner_withdrawal_or_reset() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::npc_purchase_economy::{apply_native_npc_economy_bundle,owner_hero_model,NativeNpcEconomySource};
        use mir2_client_bevy::hero_model::{HeroModel,HeroModelReceipts};
        let (gate,w,mut projection)=crate::npc_purchase_economy::tests::fixture();
        let mut owner:serde_json::Value=serde_json::from_str(&projection.owner_json).unwrap();
        owner["stage5Systems"]["hero"]=serde_json::json!({"name":"Hero","class":"Warrior","gender":"Male","level":2,
            "experience":77,"behaviour":0,"spawned":true,"autoPot":false,"autoHpPercent":30,"autoMpPercent":30,"hpItemIndex":0,"mpItemIndex":0});
        owner["heroMaxExperience"]=200.into();projection.owner_json=owner.to_string();
        projection.hero_json=serde_json::to_string(&owner_hero_model(&owner).unwrap()).unwrap();
        let mut world=bevy::prelude::World::new();
        world.insert_resource(crate::SceneResetRevision::default());
        world.insert_resource(mir2_client_bevy::pending_operations::SessionResetRevision::default());
        world.insert_resource(HeroModelReceipts::default());
        assert!(apply_native_npc_economy_bundle(&mut world,gate.prepare(w,projection.clone()).unwrap()));
        assert_eq!(gate.try_recv_applied().unwrap().witness(),w);
        let packet=mir2_protocol::HeroUserInformation{object_id:12,name:"Hero".into(),class:mir2_protocol::MirClass::Warrior,
            gender:mir2_protocol::MirGender::Male,level:2,hair:0,hp:10,mp:5,experience:1,max_experience:100,
            inventory:Some(vec![None;10]),equipment:Some(vec![None;14]),magics:vec![],auto_pot:false,
            auto_hp_percent:30,auto_mp_percent:30,hp_item_index:0,mp_item_index:0};
        let mut stale=HeroModel::default();assert!(stale.apply_packet("HeroInformation",&serde_json::json!({"info":packet})));
        let mut local=active_buffer();assert!(local.enqueue(NativeInboundMessage::HeroModel(serde_json::to_string(&stale).unwrap())));
        world.insert_resource(NativeInbound{buffer:Arc::new(Mutex::new(local))});
        world.run_system_once(crate::ingest_pending_hero_model).unwrap();
        let visible=world.resource::<HeroModel>().snapshot_read_model().unwrap();
        assert_eq!((visible.player.experience,visible.player.max_experience),(77,200));
        let display_gate=crate::npc_purchase_economy::NativeHeroOwnerGate::new(crate::npc_purchase_economy::NativeHeroOwnerEpoch{
            run:1,connection:1,procedure:1,owner_epoch:1,scene_epoch:1,cancellation:1,actor:3,map:0}).unwrap();
        let mut fresh_owner=owner.clone();fresh_owner["stage5Systems"]["hero"]["experience"]=88.into();
        let update=display_gate.prepare(&fresh_owner).unwrap();
        assert!(world.resource::<NativeInbound>().buffer.lock().unwrap().enqueue(NativeInboundMessage::HeroOwnerSnapshot(update)));
        world.run_system_once(crate::ingest_pending_hero_model).unwrap();
        world.resource::<NativeInbound>().buffer.lock().unwrap().enqueue(NativeInboundMessage::HeroModel(serde_json::to_string(&stale).unwrap()));
        world.run_system_once(crate::ingest_pending_hero_model).unwrap();
        assert_eq!(world.resource::<HeroModel>().snapshot_read_model().unwrap().player.experience,88);
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner()["stage5Systems"]["hero"]["experience"],77);
        assert!(gate.try_recv_applied().is_none());
        let (reset_gate,reset_w,_)=crate::npc_purchase_economy::tests::fixture();
        let mut reset_world=bevy::prelude::World::new();reset_world.insert_resource(crate::SceneResetRevision::default());
        assert!(apply_native_npc_economy_bundle(&mut reset_world,reset_gate.prepare(reset_w,projection.clone()).unwrap()));
        let reset_binding=reset_world.resource::<NativeNpcEconomySource>().binding();
        reset_world.resource_mut::<crate::SceneResetRevision>().0+=1;
        crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut reset_world);
        assert!(!reset_world.contains_resource::<NativeNpcEconomySource>());
        assert!(reset_world.resource::<HeroModel>().snapshot_identity.is_none());assert!(!reset_gate.is_current(reset_binding));
        // Explicit complete owner withdrawal removes the cached packet bootstrap.
        owner["stage5Systems"]["hero"]=serde_json::Value::Null;owner["heroMaxExperience"]=serde_json::Value::Null;
        projection.owner_json=owner.to_string();projection.hero_json=serde_json::to_string(&owner_hero_model(&owner).unwrap()).unwrap();
        let mut newer=w;newer.server_revision+=1;
        assert!(apply_native_npc_economy_bundle(&mut world,gate.prepare(newer,projection).unwrap()));
        world.resource::<NativeInbound>().buffer.lock().unwrap().enqueue(NativeInboundMessage::HeroModel(serde_json::to_string(&stale).unwrap()));
        world.run_system_once(crate::ingest_pending_hero_model).unwrap();
        assert!(world.resource::<HeroModel>().snapshot_identity.is_none());assert!(world.resource::<HeroModel>().info.is_none());
        // The real scene boundary clears source custody and cannot restore it.
        let binding=world.resource::<NativeNpcEconomySource>().binding();
        world.resource_mut::<crate::SceneResetRevision>().0+=1;
        crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
        assert!(!world.contains_resource::<NativeNpcEconomySource>());assert!(world.resource::<HeroModel>().snapshot_identity.is_none());
        assert!(!gate.is_current(binding));
    }

    #[test]
    fn native_npc_economy_fresh_hero_source_obeys_bundle_order_and_retirement() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::npc_purchase_economy::{NativeHeroOwnerEpoch,NativeHeroOwnerGate,owner_hero_model,NativeNpcEconomySource};
        use mir2_client_bevy::hero_model::{HeroModel,HeroModelReceipts};
        let (gate,w,mut projection)=crate::npc_purchase_economy::tests::fixture();
        let mut owner:serde_json::Value=serde_json::from_str(&projection.owner_json).unwrap();
        owner["stage5Systems"]["hero"]=serde_json::json!({"name":"Hero","class":"Warrior","gender":"Male","level":2,
            "experience":77,"behaviour":0,"spawned":true,"autoPot":false,"autoHpPercent":30,"autoMpPercent":30,"hpItemIndex":0,"mpItemIndex":0});
        owner["heroMaxExperience"]=200.into();
        let epoch=NativeHeroOwnerEpoch{run:1,connection:1,procedure:1,owner_epoch:1,scene_epoch:1,cancellation:1,actor:3,map:0};
        let display_gate=NativeHeroOwnerGate::new(epoch).unwrap();let mut local=active_buffer();
        assert!(local.enqueue(NativeInboundMessage::HeroOwnerSnapshot(display_gate.prepare(&owner).unwrap())));
        owner["stage5Systems"]["hero"]["experience"]=88.into();projection.owner_json=owner.to_string();
        projection.hero_json=serde_json::to_string(&owner_hero_model(&owner).unwrap()).unwrap();
        assert!(local.enqueue(NativeInboundMessage::NpcEconomyBundle(gate.prepare(w,projection).unwrap())));
        owner["stage5Systems"]["hero"]["experience"]=99.into();
        assert!(local.enqueue(NativeInboundMessage::HeroOwnerSnapshot(display_gate.prepare(&owner).unwrap())));
        let mut world=bevy::prelude::World::new();world.insert_resource(HeroModel::default());world.insert_resource(HeroModelReceipts::default());
        world.insert_resource(crate::SceneResetRevision::default());
        world.insert_resource(NativeInbound{buffer:Arc::new(Mutex::new(local))});
        world.run_system_once(crate::ingest_pending_hero_model).unwrap();
        assert_eq!(world.resource::<HeroModel>().snapshot_read_model().unwrap().player.experience,77);assert!(gate.try_recv_applied().is_none());
        crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
        assert_eq!(world.resource::<HeroModel>().snapshot_read_model().unwrap().player.experience,88);assert_eq!(gate.try_recv_applied().unwrap().witness(),w);
        world.run_system_once(crate::ingest_pending_hero_model).unwrap();
        assert_eq!(world.resource::<HeroModel>().snapshot_read_model().unwrap().player.experience,99);
        assert_eq!(world.resource::<NativeNpcEconomySource>().owner()["stage5Systems"]["hero"]["experience"],88);assert!(gate.try_recv_applied().is_none());
        let retired=display_gate.prepare(&owner).unwrap();display_gate.retire();
        world.resource::<NativeInbound>().buffer.lock().unwrap().enqueue(NativeInboundMessage::HeroOwnerSnapshot(retired));
        world.run_system_once(crate::ingest_pending_hero_model).unwrap();
        assert_eq!(world.resource::<HeroModel>().snapshot_read_model().unwrap().player.experience,99);
        assert!(display_gate.prepare(&owner).is_err());
        assert!(NativeHeroOwnerGate::new(NativeHeroOwnerEpoch{connection:u64::MAX,..epoch}).is_err());
        let wrong=NativeHeroOwnerGate::new(NativeHeroOwnerEpoch{actor:4,..epoch}).unwrap();assert!(wrong.prepare(&owner).is_err());
        world.resource_mut::<crate::SceneResetRevision>().0+=1;
        crate::npc_purchase_economy::apply_pending_native_npc_economy(&mut world);
        assert!(world.resource::<HeroModel>().snapshot_identity.is_none());assert!(!world.contains_resource::<NativeNpcEconomySource>());
    }

    #[test]
    fn native_npc_economy_queue_bundle_is_protected_under_snapshot_and_ack_pressure() {
        let (_,bundle)=npc_bundle();let mut state=active_buffer();assert!(state.enqueue(bundle));
        for n in 0..(MAX_NATIVE_MESSAGES*2) {
            let _=state.enqueue(NativeInboundMessage::NpcShopService(n.to_string()));
            let _=state.enqueue(NativeInboundMessage::InventoryModel(n.to_string()));
            let _=state.enqueue(NativeInboundMessage::InventoryOperationAck(n.to_string()));
        }
        assert_eq!(state.pending.iter().filter(|m| matches!(m,NativeInboundMessage::NpcEconomyBundle(_))).count(),1);
        assert!(state.message_count()<=MAX_NATIVE_MESSAGES);
        assert!(state.pending_bytes()<=MAX_NATIVE_BUFFER_BYTES);
        assert!(!state.evict_oldest_non_boundary() || state.pending.iter().any(|m|matches!(m,NativeInboundMessage::NpcEconomyBundle(_))));
    }

    #[test]
    fn native_npc_economy_queue_byte_limit_rejects_whole_bundle_without_ack() {
        let (gate,bundle)=npc_bundle();let mut state=active_buffer();let bytes=native_message_bytes(&bundle);
        assert!(!state.enqueue_with_limits(bundle.clone(),bytes-1,bytes*2));assert!(state.pending.is_empty());
        assert!(!state.enqueue_with_limits(bundle,bytes,bytes-1));assert!(state.pending.is_empty());
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_queue_reserved_cost_materialization_obeys_exact_byte_cap() {
        let (gate,bundle)=npc_bundle();let mut state=active_buffer();start_mail(&mut state);
        assert!(state.reserve_mail_cost((MAIL_EPOCH,70),MAX_NATIVE_BUFFER_BYTES));
        let before_bytes=state.pending_bytes();let before_count=state.message_count();
        let delta=std::mem::size_of::<MailServiceDelivery>()-std::mem::size_of::<(MailServiceStreamEpoch,u32)>();
        assert!(delta>0);
        let bundle_bytes=native_message_bytes(&bundle);
        let unused_quotas=std::mem::size_of::<MailQuoteReceipt>()+2*std::mem::size_of::<MailSendReceipt>()+std::mem::size_of::<MailSendAcknowledgement>();
        let exact_cap=before_bytes+bundle_bytes+delta+unused_quotas;
        assert!(!state.enqueue_with_limits(bundle.clone(),bundle_bytes,exact_cap-1));
        assert_eq!(state.pending_bytes(),before_bytes);assert_eq!(state.message_count(),before_count);
        assert_eq!(state.mail_cost_reserve,Some((MAIL_EPOCH,70)));assert_eq!(state.mail_terminal_tail.len(),1);
        assert!(state.pending.is_empty());assert!(state.mail_stream_marker.is_some());assert_eq!(gate.try_recv_applied(),None);
        assert!(state.enqueue_with_limits(bundle,bundle_bytes,exact_cap));
        assert_eq!(state.pending_bytes(),exact_cap-unused_quotas);
        assert!(state.pending_bytes()<=exact_cap);assert_eq!(state.mail_cost_reserve,None);assert!(state.mail_terminal_tail.is_empty());
        assert!(matches!(&state.pending[0],NativeInboundMessage::MailServiceStreamStarted(_)));
        assert!(is_valid_mail_cost(&state.pending[1]));assert!(matches!(&state.pending[2],NativeInboundMessage::NpcEconomyBundle(_)));
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_queue_data_and_scene_reset_drop_bundle_without_completion() {
        for reset in [NativeInboundMessage::DataReset,NativeInboundMessage::SceneReset] {
            let (gate,bundle)=npc_bundle();let mut state=active_buffer();assert!(state.enqueue(bundle));
            assert!(state.enqueue(reset));assert!(!state.pending.iter().any(|m|matches!(m,NativeInboundMessage::NpcEconomyBundle(_))));
            assert_eq!(gate.try_recv_applied(),None);
            let (_,witness,p)=crate::npc_purchase_economy::tests::fixture();
            assert_eq!(gate.prepare(witness,p).unwrap_err(),crate::npc_purchase_economy::NativeNpcEconomyError::Retired);
        }
    }

    #[test]
    fn native_npc_economy_queue_drop_retires_queued_custody_gracefully() {
        let (gate,bundle)=npc_bundle();let mut state=active_buffer();assert!(state.enqueue(bundle));
        let buffer=Arc::new(Mutex::new(state));let inbound=NativeInbound{buffer:Arc::clone(&buffer)};drop(inbound);
        let state=buffer.lock().unwrap();assert!(!state.active);assert!(state.pending.is_empty());
        assert_eq!(gate.try_recv_applied(),None);
    }

    #[test]
    fn native_npc_economy_queue_later_receipt_reserve_waits_for_barrier() {
        let (_,bundle)=npc_bundle();let mut state=active_buffer();assert!(state.enqueue(bundle));
        assert!(state.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("later"))));
        let inbound=NativeInbound{buffer:Arc::new(Mutex::new(state))};
        let mut receipt=None;
        inbound.drain_matching(|m|matches!(m,NativeInboundMessage::GameShopReceipt(_)),|m|receipt=Some(m));
        assert!(receipt.is_none());assert!(inbound.take_npc_economy_front().is_some());
        inbound.drain_matching(|m|matches!(m,NativeInboundMessage::GameShopReceipt(_)),|m|receipt=Some(m));
        assert!(receipt.is_some());
    }

    const MAIL_EPOCH: MailServiceStreamEpoch = MailServiceStreamEpoch { run: 1, connection: 1 };
    fn mail(event: MailServiceEvent) -> NativeInboundMessage {
        NativeInboundMessage::MailService(MailServiceDelivery { epoch: MAIL_EPOCH, event })
    }
    fn start_mail(buffer: &mut NativeInboundBuffer) {
        assert!(buffer.enqueue(NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted { epoch: MAIL_EPOCH })));
    }
    fn drain_mail(inbound: &NativeInbound) -> Vec<mir2_client_bevy::mail_service::MailServiceInboxMessage> {
        use mir2_client_bevy::mail_service::MailServiceInboxMessage;
        let mut delivered = Vec::new();
        inbound.drain_matching(
            |message| is_mail_message(message),
            |message| match message {
                NativeInboundMessage::MailServiceStreamStarted(marker) => delivered.push(MailServiceInboxMessage::StreamStarted(marker)),
                NativeInboundMessage::MailService(delivery) => delivered.push(MailServiceInboxMessage::Delivery(delivery)),
                NativeInboundMessage::MailQuoteReceipt(receipt)=>delivered.push(MailServiceInboxMessage::QuoteReceipt(receipt)),
                NativeInboundMessage::MailSendReceipt(receipt)=>delivered.push(MailServiceInboxMessage::SendReceipt(receipt)),
                NativeInboundMessage::MailSendAcknowledgement(ack)=>delivered.push(MailServiceInboxMessage::SendAcknowledgement(ack)),
                _ => unreachable!(),
            },
        );
        delivered
    }
    fn cost_delivery(cost: u32) -> mir2_client_bevy::mail_service::MailServiceInboxMessage {
        mir2_client_bevy::mail_service::MailServiceInboxMessage::Delivery(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::Cost { cost } })
    }
    fn quote_receipt()->MailQuoteReceipt {use mir2_client_bevy::mail_service::{MailQuoteTicket,MailQuoteOutcome};MailQuoteReceipt{ticket:MailQuoteTicket{run:MAIL_EPOCH.run,connection:MAIL_EPOCH.connection,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:73,local_quote_token:19},outcome:MailQuoteOutcome::Entered{at_ms:500}}}
    fn mail_send_receipt(outcome:mir2_client_bevy::mail_service::MailSendOutcome)->MailSendReceipt{
        use mir2_client_bevy::mail_service::MailSendTicket;MailSendReceipt{ticket:MailSendTicket{run:MAIL_EPOCH.run,connection:MAIL_EPOCH.connection,procedure:1,owner_epoch:2,scene_epoch:1,cancellation:1,actor:Some(7),map:Some(0),sequence:74,local_send_token:20},outcome}
    }
    #[test]
    fn mail_send_combined_small_byte_tail_refusal_poison_preserves_every_accepted_terminal(){
        use mir2_client_bevy::mail_service::MailSendOutcome;
        let _guard=native_queue_test_guard();let inbound=NativeInbound::new();
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));
        for index in 0..MAX_NATIVE_MESSAGES{assert!(push_native_social_model(index.to_string()));}
        let quote=quote_receipt();let entry=mail_send_receipt(MailSendOutcome::Entered);let write=mail_send_receipt(MailSendOutcome::Flushed);
        assert!(push_native_mail_quote_receipt(quote));assert!(push_native_mail_service(MailServiceDelivery{epoch:MAIL_EPOCH,event:MailServiceEvent::Cost{cost:70}}));
        assert!(push_native_mail_send_receipt(entry));assert!(push_native_mail_send_receipt(write));
        let ack=MailSendAcknowledgement{ticket:entry.ticket,result:1};
        let (before_bytes,before_count,budget)={let state=inbound.buffer.lock().unwrap();(state.pending_bytes(),state.message_count(),state.pending_bytes()+std::mem::size_of::<MailSendAcknowledgement>()-1)};
        assert!(!publish_native_mail_send_terminal(NativeInboundMessage::MailSendAcknowledgement(ack),MAIL_EPOCH,|message|inbound.buffer.lock().unwrap().enqueue_with_limits(message,budget,budget)));
        {let state=inbound.buffer.lock().unwrap();assert_eq!(state.pending_bytes(),before_bytes);assert_eq!(state.message_count(),before_count);assert!(state.mail_stream_failed);assert_eq!(state.mail_terminal_tail.len(),4);}
        assert!(push_native_data_reset());assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));assert_eq!(inbound.mail_stream_failure(),Some(MAIL_EPOCH));
        assert!(!push_native_mail_send_acknowledgement(ack));
        let newer=MailServiceStreamEpoch{run:MAIL_EPOCH.run,connection:MAIL_EPOCH.connection+1};assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:newer}));assert_eq!(inbound.mail_stream_failure(),None);
        assert!(push_native_mail_send_acknowledgement(ack),"a newer stream is the exact old-ticket terminal");
    }
    #[test]
    fn mail_send_stale_discard_reset_classifier_and_partial_drain_preserve_critical_fifo(){
        use mir2_client_bevy::mail_service::{MailSendOutcome,MailServiceInboxMessage};
        for reset in [NativeInboundMessage::DataReset,NativeInboundMessage::DataResetPreservingExactGameShopReceipt(typed_receipt("gs-mail-send-discard"))]{
            let _guard=native_queue_test_guard();let inbound=NativeInbound::new();
            assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));
            assert!(push_native_mail_service(MailServiceDelivery{epoch:MAIL_EPOCH,event:MailServiceEvent::LockedItem{unique_id:7,locked:true}}));
            assert!(push_native_mail_service(MailServiceDelivery{epoch:MAIL_EPOCH,event:MailServiceEvent::Cost{cost:70}}));
            let entry=mail_send_receipt(MailSendOutcome::Entered);let write=mail_send_receipt(MailSendOutcome::Flushed);let ack=MailSendAcknowledgement{ticket:entry.ticket,result:1};
            assert!(push_native_mail_send_receipt(entry));assert!(push_native_mail_send_receipt(write));assert!(push_native_mail_send_acknowledgement(ack));
            // Inject a barrier only to isolate the actual stale-discard layer
            // from enqueue's separate protected-terminal retention.
            inbound.buffer.lock().unwrap().pending.push_back(reset);inbound.discard_stale_data_before_latest_reset();
            let mut skipped=0;inbound.drain_matching(|message|matches!(message,NativeInboundMessage::MailSendReceipt(_)|NativeInboundMessage::MailSendAcknowledgement(_)),|_|skipped+=1);assert_eq!(skipped,0,"no terminal skips the protected marker or earlier Cost");
            assert_eq!(drain_mail(&inbound),vec![marker_delivery(),cost_delivery(70),MailServiceInboxMessage::SendReceipt(entry),MailServiceInboxMessage::SendReceipt(write),MailServiceInboxMessage::SendAcknowledgement(ack)]);
        }
    }
    #[test]
    fn mail_each_critical_kind_has_independent_count_and_fixed_payload_byte_quota(){
        use mir2_client_bevy::mail_service::MailSendOutcome;
        let mut buffer=active_buffer();start_mail(&mut buffer);for index in 0..MAX_NATIVE_MESSAGES{assert!(buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string())));}
        let quote=quote_receipt();let entry=mail_send_receipt(MailSendOutcome::Entered);let write=mail_send_receipt(MailSendOutcome::Flushed);let ack=MailSendAcknowledgement{ticket:entry.ticket,result:1};
        assert!(buffer.enqueue(NativeInboundMessage::MailQuoteReceipt(quote)));assert!(!buffer.enqueue(NativeInboundMessage::MailQuoteReceipt(quote)));
        assert!(buffer.enqueue(mail(MailServiceEvent::Cost{cost:70})));assert!(!buffer.enqueue(mail(MailServiceEvent::Cost{cost:71})));
        assert!(buffer.enqueue(NativeInboundMessage::MailSendReceipt(entry)));assert!(buffer.enqueue(NativeInboundMessage::MailSendReceipt(write)));assert!(!buffer.enqueue(NativeInboundMessage::MailSendReceipt(mail_send_receipt(MailSendOutcome::Unknown))));
        assert!(buffer.enqueue(NativeInboundMessage::MailSendAcknowledgement(ack)));assert!(!buffer.enqueue(NativeInboundMessage::MailSendAcknowledgement(ack)));
        assert_eq!(buffer.mail_terminal_tail.len(),5);assert_eq!(buffer.mail_terminal_tail.iter().map(mail_reserved_bytes).sum::<usize>(),mail_terminal_reserve_bytes());
        for kind in 1..=4{let records=buffer.mail_terminal_tail.iter().filter(|message|mail_terminal_kind(message)==kind).collect::<Vec<_>>();let (count,bytes)=mail_terminal_quota(records[0]);assert_eq!(records.len(),count);assert_eq!(records.iter().map(|message|mail_reserved_bytes(message)).sum::<usize>(),bytes);}
    }
    #[test]
    fn mail_single_typed_terminal_exceeding_its_own_byte_reserve_refuses_then_poison(){
        use mir2_client_bevy::mail_service::MailSendOutcome;
        let entry=mail_send_receipt(MailSendOutcome::Entered);let ack=MailSendAcknowledgement{ticket:entry.ticket,result:1};
        for message in [NativeInboundMessage::MailQuoteReceipt(quote_receipt()),mail(MailServiceEvent::Cost{cost:70}),NativeInboundMessage::MailSendReceipt(entry),NativeInboundMessage::MailSendAcknowledgement(ack)]{
            let _guard=native_queue_test_guard();let inbound=NativeInbound::new();assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));
            for index in 0..MAX_NATIVE_MESSAGES{assert!(push_native_social_model(index.to_string()));}
            let dedicated_bytes=mail_reserved_bytes(&message)-1;let before={let state=inbound.buffer.lock().unwrap();(state.pending_bytes(),state.message_count())};
            // Only this controlled dedicated payload-byte budget is reduced.
            // Global bytes/count have room; reuse production reserve and the
            // production terminal-publication refusal/poison path.
            assert!(!publish_native_mail_send_terminal(message,MAIL_EPOCH,|message|inbound.buffer.lock().unwrap().reserve_mail_terminal_with_kind_budget(message,MAX_NATIVE_BUFFER_BYTES,dedicated_bytes)));
            {let state=inbound.buffer.lock().unwrap();assert_eq!((state.pending_bytes(),state.message_count()),before);assert!(state.mail_terminal_tail.is_empty());assert!(state.mail_stream_failed);}
            assert!(push_native_data_reset());assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));assert_eq!(inbound.mail_stream_failure(),Some(MAIL_EPOCH));
            let newer=MailServiceStreamEpoch{run:1,connection:2};assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:newer}));assert_eq!(inbound.mail_stream_failure(),None);
        }
    }
    #[test]
    fn mail_quote_receipt_then_cost_have_protected_fifo_reserves_and_reset_retention(){
        for preserving_shop in [false,true]{
            let _guard=native_queue_test_guard();let inbound=NativeInbound::new();
            assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));
            for index in 0..MAX_NATIVE_MESSAGES{assert!(push_native_social_model(index.to_string()));}
            let receipt=quote_receipt();assert!(push_native_mail_quote_receipt(receipt));
            assert!(push_native_mail_service(MailServiceDelivery{epoch:MAIL_EPOCH,event:MailServiceEvent::Cost{cost:70}}));
            {let state=inbound.buffer.lock().unwrap();assert_eq!(state.mail_quote_receipt_reserve,Some(receipt));assert_eq!(state.mail_cost_reserve,Some((MAIL_EPOCH,70)));assert_eq!(state.message_count(),MAX_NATIVE_MESSAGES+3);}
            let mut skipped=0;inbound.drain_matching(|message|matches!(message,NativeInboundMessage::MailService(_)),|_|skipped+=1);assert_eq!(skipped,0);
            if preserving_shop{assert!(push_native_data_reset_preserving_exact_game_shop_receipt(typed_receipt("gs-quote")));}else{assert!(push_native_data_reset());}
            inbound.discard_stale_data_before_latest_reset();
            assert_eq!(drain_mail(&inbound),vec![marker_delivery(),mir2_client_bevy::mail_service::MailServiceInboxMessage::QuoteReceipt(receipt),cost_delivery(70)]);
        }
    }
    #[test]
    fn mail_quote_pending_receipt_blocks_partial_consumer_and_stale_discard_keeps_order(){
        let _guard=native_queue_test_guard();let inbound=NativeInbound::new();
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));drain_mail(&inbound);
        assert!(push_native_mail_service(MailServiceDelivery{epoch:MAIL_EPOCH,event:MailServiceEvent::LockedItem{unique_id:7,locked:true}}));
        let receipt=quote_receipt();assert!(push_native_mail_quote_receipt(receipt));assert!(push_native_mail_service(MailServiceDelivery{epoch:MAIL_EPOCH,event:MailServiceEvent::Cost{cost:70}}));
        for barrier in [NativeInboundMessage::DataReset,NativeInboundMessage::DataResetPreservingExactGameShopReceipt(typed_receipt("gs-stale-quote"))]{inbound.buffer.lock().unwrap().pending.push_back(barrier);}
        inbound.discard_stale_data_before_latest_reset();
        let mut skipped=0;inbound.drain_matching(|message|matches!(message,NativeInboundMessage::MailService(_)),|_|skipped+=1);assert_eq!(skipped,0,"Cost cannot skip a queued typed entry receipt");
        assert_eq!(drain_mail(&inbound),vec![mir2_client_bevy::mail_service::MailServiceInboxMessage::QuoteReceipt(receipt),cost_delivery(70)]);
    }
    #[test]
    fn mail_quote_receipt_bytes_are_accounted_and_new_marker_clears_only_old_stream(){
        let mut buffer=active_buffer();let receipt=quote_receipt();
        let budget=std::mem::size_of::<MailServiceStreamStarted>()+mail_terminal_reserve_bytes();
        let retained=std::mem::size_of::<MailServiceStreamStarted>()+std::mem::size_of::<MailQuoteReceipt>()+std::mem::size_of::<(MailServiceStreamEpoch,u32)>();
        assert!(buffer.enqueue_with_limits(NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted{epoch:MAIL_EPOCH}),budget,budget));
        assert!(buffer.enqueue_with_limits(NativeInboundMessage::MailQuoteReceipt(receipt),budget,budget));
        assert!(buffer.enqueue_with_limits(mail(MailServiceEvent::Cost{cost:70}),budget,budget));assert_eq!(buffer.pending_bytes(),retained,"unused Send quotas are headroom, not retained payload bytes");
        assert!(buffer.enqueue_with_limits(NativeInboundMessage::SceneReset,budget,budget));assert_eq!(buffer.pending_bytes(),retained);
        start_mail(&mut buffer);assert_eq!(buffer.pending_bytes(),retained);
        let newer=MailServiceStreamEpoch{run:1,connection:2};assert!(buffer.enqueue_with_limits(NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted{epoch:newer}),budget,budget));
        assert_eq!(buffer.mail_quote_receipt_reserve,None);assert_eq!(buffer.mail_cost_reserve,None);assert!(!buffer.pending.iter().any(is_protected_mail_terminal));
    }
    #[test]
    fn mail_quote_critical_failure_latches_until_true_new_marker(){
        let _guard=native_queue_test_guard();let inbound=NativeInbound::new();let receipt=quote_receipt();
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));
        // Deliberately reverse the required production order to force refusal.
        assert!(push_native_mail_service(MailServiceDelivery{epoch:MAIL_EPOCH,event:MailServiceEvent::Cost{cost:70}}));
        assert!(!push_native_mail_quote_receipt(receipt));assert_eq!(inbound.mail_stream_failure(),Some(MAIL_EPOCH));
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));assert_eq!(inbound.mail_stream_failure(),Some(MAIL_EPOCH));
        assert!(push_native_data_reset());assert_eq!(inbound.mail_stream_failure(),Some(MAIL_EPOCH));
        let newer=MailServiceStreamEpoch{run:1,connection:2};assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:newer}));assert_eq!(inbound.mail_stream_failure(),None);
        assert!(push_native_mail_quote_receipt(receipt),"real newer stream is an explicit old-ticket terminal");
    }
    fn marker_delivery() -> mir2_client_bevy::mail_service::MailServiceInboxMessage {
        mir2_client_bevy::mail_service::MailServiceInboxMessage::StreamStarted(MailServiceStreamStarted { epoch: MAIL_EPOCH })
    }

    #[test]
    fn mail_service_events_are_critical_ordered_and_owner_events_removed_by_data_reset() {
        let mut buffer = active_buffer(); start_mail(&mut buffer);
        for index in 0..NON_CRITICAL_MESSAGE_LIMIT { assert!(buffer.enqueue(NativeInboundMessage::ChatLine(index.to_string()))); }
        assert!(buffer.enqueue(mail(MailServiceEvent::OpenParcel)));
        assert!(buffer.enqueue(mail(MailServiceEvent::LockedItem { unique_id: 77, locked: true })));
        assert_eq!(buffer.pending.len(), NON_CRITICAL_MESSAGE_LIMIT + 2, "service events bypass ordinary limit without coalescing");
        assert!(buffer.enqueue(mail(MailServiceEvent::Cost { cost: 125 })));
        assert!(buffer.enqueue(NativeInboundMessage::DataReset));
        assert_eq!(buffer.pending.len(), 2);
        assert!(is_valid_mail_cost(&buffer.pending[0]));
        assert!(matches!(&buffer.pending[1], NativeInboundMessage::DataReset));
        assert_eq!(buffer.mail_stream_epoch, Some(MAIL_EPOCH));
        assert_eq!(buffer.mail_stream_marker, Some(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
    }

    #[test]
    fn saturated_native_fifo_reserves_one_tagged_mail_cost_after_marker_and_fifo() {
        let _guard = native_queue_test_guard(); let inbound = NativeInbound::new();
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
        assert!(push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::OpenParcel }));
        for index in 1..MAX_NATIVE_MESSAGES { assert!(push_native_social_model(index.to_string())); }
        assert!(push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::Cost { cost: 125 } }));
        assert!(!push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::Cost { cost: 250 } }));
        assert!(!push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::LockedItem { unique_id: 77, locked: true } }));
        assert_eq!(inbound.diagnostics().message_count, MAX_NATIVE_MESSAGES + 2);
        assert_eq!(drain_mail(&inbound), vec![marker_delivery(),
            mir2_client_bevy::mail_service::MailServiceInboxMessage::Delivery(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::OpenParcel }), cost_delivery(125)]);
    }

    #[test]
    fn queued_mail_cost_survives_full_fifo_game_shop_receipt_and_keeps_its_position() {
        let _guard = native_queue_test_guard(); let inbound = NativeInbound::new();
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
        assert!(push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::Cost { cost: 125 } }));
        assert!(!push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::Cost { cost: 250 } }));
        for index in 0..MAX_NATIVE_MESSAGES - 1 { assert!(push_native_social_model(index.to_string())); }
        assert!(push_native_game_shop_receipt(valid_receipt("gs-queued-cost")));
        assert_eq!(drain_mail(&inbound), vec![marker_delivery(), cost_delivery(125)]);
        let mut receipts = Vec::new();
        inbound.drain_matching(|message| matches!(message, NativeInboundMessage::GameShopReceipt(_)), |message| {
            if let NativeInboundMessage::GameShopReceipt(json) = message { receipts.push(json); }
        });
        assert_eq!(receipts, vec![valid_receipt("gs-queued-cost")]);
    }

    #[test]
    fn queued_mail_cost_survives_full_fifo_operation_ack() {
        let _guard = native_queue_test_guard(); let inbound = NativeInbound::new();
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
        assert!(push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::Cost { cost: 125 } }));
        for index in 0..MAX_NATIVE_MESSAGES - 1 { assert!(push_native_social_model(index.to_string())); }
        assert!(push_native_inventory_operation_ack(r#"{"kind":"item","id":7}"#.to_owned()));
        assert_eq!(drain_mail(&inbound), vec![marker_delivery(), cost_delivery(125)]);
        let mut acknowledgements = 0;
        inbound.drain_matching(|message| matches!(message, NativeInboundMessage::InventoryOperationAck(_)), |_| acknowledgements += 1);
        assert_eq!(acknowledgements, 1);
    }

    #[test]
    fn mail_cost_reserve_does_not_displace_a_game_shop_receipt() {
        let mut buffer = active_buffer(); start_mail(&mut buffer);
        for index in 0..MAX_NATIVE_MESSAGES { assert!(buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string()))); }
        assert!(buffer.enqueue(mail(MailServiceEvent::Cost { cost: 125 })));
        assert_eq!(buffer.mail_cost_reserve, Some((MAIL_EPOCH, 125)));
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("gs-mail"))));
        assert!(buffer.game_shop_receipt.as_deref().is_some_and(|json| json.contains("gs-mail")));
        assert_eq!(buffer.mail_cost_reserve, Some((MAIL_EPOCH, 125)));
    }

    #[test]
    fn both_data_reset_enqueue_branches_keep_same_stream_cost_reserve_and_highwater() {
        for preserve_shop in [false, true] {
            let _guard = native_queue_test_guard(); let inbound = NativeInbound::new();
            assert!(push_native_mail_service_stream_started(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
            for index in 0..MAX_NATIVE_MESSAGES { assert!(push_native_social_model(format!("old-{index}"))); }
            assert!(push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::Cost { cost: 1 } }));
            if preserve_shop { assert!(push_native_data_reset_preserving_exact_game_shop_receipt(typed_receipt("gs-reset"))); }
            else { assert!(push_native_data_reset()); }
            assert!(!push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event: MailServiceEvent::Cost { cost: 2 } }));
            assert!(push_native_mail_service_stream_started(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
            inbound.discard_stale_data_before_latest_reset();
            assert_eq!(drain_mail(&inbound), vec![marker_delivery(), cost_delivery(1)]);
            assert_eq!(inbound.buffer.lock().unwrap().mail_stream_epoch, Some(MAIL_EPOCH));
        }
    }

    #[test]
    fn stale_discard_preserves_pre_reset_cost_and_marker_but_clears_ordinary_mail() {
        for reset in [NativeInboundMessage::DataReset, NativeInboundMessage::DataResetPreservingExactGameShopReceipt(typed_receipt("gs-discard"))] {
            let _guard = native_queue_test_guard(); let inbound = NativeInbound::new();
            assert!(push_native_mail_service_stream_started(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
            for event in [MailServiceEvent::OpenParcel, MailServiceEvent::LockedItem { unique_id: 7, locked: true }, MailServiceEvent::Cost { cost: 125 }] {
                assert!(push_native_mail_service(MailServiceDelivery { epoch: MAIL_EPOCH, event }));
            }
            // Exercise the independent stale-data pass; enqueue's retention
            // must not mask a regression in is_resettable_data_message.
            inbound.buffer.lock().unwrap().pending.push_back(reset);
            inbound.discard_stale_data_before_latest_reset();
            assert_eq!(drain_mail(&inbound), vec![marker_delivery(), cost_delivery(125)]);
        }
    }

    #[test]
    fn native_stream_requires_announced_positive_full_pair_and_new_marker_drops_reserve() {
        let mut buffer = active_buffer();
        assert!(!buffer.enqueue(mail(MailServiceEvent::Cost { cost: 1 })));
        for epoch in [MailServiceStreamEpoch { run: 0, connection: 1 }, MailServiceStreamEpoch { run: 1, connection: 0 }] {
            assert!(!buffer.enqueue(NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted { epoch })));
            assert!(!buffer.enqueue(NativeInboundMessage::MailService(MailServiceDelivery { epoch, event: MailServiceEvent::Cost { cost: 1 } })));
        }
        start_mail(&mut buffer);
        for index in 0..MAX_NATIVE_MESSAGES { assert!(buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string()))); }
        assert!(buffer.enqueue(mail(MailServiceEvent::Cost { cost: 1 })));
        start_mail(&mut buffer); assert_eq!(buffer.mail_cost_reserve, Some((MAIL_EPOCH, 1)));
        let high = MailServiceStreamEpoch { run: 1, connection: u64::MAX };
        assert!(!buffer.enqueue(NativeInboundMessage::MailService(MailServiceDelivery { epoch: high, event: MailServiceEvent::Cost { cost: 2 } })));
        for epoch in [high, MailServiceStreamEpoch { run: 2, connection: 1 }, MailServiceStreamEpoch { run: u64::MAX, connection: u64::MAX }] {
            assert!(buffer.enqueue(NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted { epoch })));
            assert_eq!(buffer.mail_cost_reserve, None);
            assert!(!buffer.enqueue(mail(MailServiceEvent::Cost { cost: 1 })));
            assert!(!buffer.enqueue(NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted { epoch: MAIL_EPOCH })));
            assert!(buffer.enqueue(NativeInboundMessage::MailService(MailServiceDelivery { epoch, event: MailServiceEvent::Cost { cost: 2 } })));
            assert!(buffer.enqueue(NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted { epoch })));
            assert_eq!(buffer.mail_cost_reserve, Some((epoch, 2)));
        }
        assert_eq!(buffer.message_count(), MAX_NATIVE_MESSAGES + 2);
    }

    #[test]
    fn reserved_tagged_cost_and_marker_are_byte_accounted_and_survive_scene_reset() {
        let mut buffer = active_buffer();
        let budget=mail_terminal_reserve_bytes()+256;
        let payload_bytes=budget-std::mem::size_of::<MailServiceStreamStarted>()-mail_terminal_reserve_bytes();
        assert!(buffer.enqueue_with_limits(NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted { epoch: MAIL_EPOCH }), budget, budget));
        assert!(buffer.enqueue_with_limits(NativeInboundMessage::SocialModel("x".repeat(payload_bytes)), budget, budget));
        assert!(buffer.enqueue_with_limits(mail(MailServiceEvent::Cost { cost: 125 }), budget, budget));
        assert_eq!(buffer.mail_cost_reserve, Some((MAIL_EPOCH, 125)));
        assert_eq!(buffer.pending_bytes(), payload_bytes + std::mem::size_of::<MailServiceStreamStarted>() + std::mem::size_of::<(MailServiceStreamEpoch, u32)>());
        assert!(buffer.pending_bytes() <= budget);
        assert!(buffer.enqueue_with_limits(NativeInboundMessage::SceneReset, budget, budget));
        assert_eq!(buffer.mail_cost_reserve, Some((MAIL_EPOCH, 125)));
        assert_eq!(buffer.mail_stream_marker, Some(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
    }

    #[test]
    fn marker_byte_preflight_preserves_ack_before_replaceable_snapshot() {
        let ack_json = r#"{"requestId":73,"success":true}"#.to_owned();
        let snapshot_json = r#"{"tick":17}"#.to_owned();
        let marker_bytes = std::mem::size_of::<MailServiceStreamStarted>();
        let reserve_bytes = mail_terminal_reserve_bytes();
        let budget = ack_json.capacity() + marker_bytes + reserve_bytes;
        // All operation-ACK families use the same protected admission rule.
        for ack in [
            NativeInboundMessage::InventoryOperationAck(ack_json.clone()),
            NativeInboundMessage::HeroModelReceipt(ack_json.clone()),
            NativeInboundMessage::SkillModelReceipt(ack_json.clone()),
        ] {
            let mut buffer = active_buffer();
            let ack_bytes = native_message_bytes(&ack);
            assert!(buffer.enqueue_with_limits(ack.clone(), budget, budget));
            assert!(buffer.enqueue_with_limits(
                NativeInboundMessage::WorldState(snapshot_json.clone()), budget, budget,
            ));
            assert!(buffer.enqueue_with_limits(
                NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted { epoch: MAIL_EPOCH }),
                budget, budget,
            ));
            assert_eq!(buffer.pending.len(), 1);
            assert_eq!(format!("{:?}", buffer.pending.front().unwrap()), format!("{ack:?}"));
            assert_eq!(buffer.pending_bytes(), ack_bytes + marker_bytes);
            assert_eq!(buffer.pending_bytes() + reserve_bytes, budget);
            assert_eq!(buffer.mail_stream_epoch, Some(MAIL_EPOCH));
            assert_eq!(buffer.mail_stream_marker, Some(MailServiceStreamStarted { epoch: MAIL_EPOCH }));
        }
    }

    #[test]
    fn marker_byte_preflight_refusal_leaves_protected_stream_unchanged() {
        let mut buffer = active_buffer();
        start_mail(&mut buffer);
        assert!(buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
            r#"{"requestId":73,"success":true}"#.to_owned(),
        )));
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("gs-protected"))));
        let reserve_bytes = mail_terminal_reserve_bytes();
        let budget = buffer.pending_bytes() + reserve_bytes;
        assert!(buffer.enqueue_with_limits(mail(MailServiceEvent::Cost { cost: 125 }), budget, budget));
        assert_eq!(buffer.mail_cost_reserve, Some((MAIL_EPOCH, 125)));
        assert!(buffer.enqueue_with_limits(NativeInboundMessage::SceneReset, budget, budget));
        let before_pending = format!("{:?}", buffer.pending);
        let before_bytes = buffer.pending_bytes();
        let before_count = buffer.message_count();
        let before_receipt = buffer.game_shop_receipt.clone();
        let before_cost = buffer.mail_cost_reserve;
        let before_marker = buffer.mail_stream_marker;
        let before_epoch = buffer.mail_stream_epoch;
        let newer = MailServiceStreamEpoch { run: MAIL_EPOCH.run, connection: MAIL_EPOCH.connection + 1 };
        // This reaches eviction preflight: fixed marker/reserve slots fit, but
        // accepted ACK/receipt bytes cannot leave the final one byte of room.
        assert!(budget - 1 >= std::mem::size_of::<MailServiceStreamStarted>() + reserve_bytes);
        assert!(!buffer.enqueue_with_limits(
            NativeInboundMessage::MailServiceStreamStarted(MailServiceStreamStarted { epoch: newer }),
            budget, budget - 1,
        ));
        assert_eq!(format!("{:?}", buffer.pending), before_pending);
        assert_eq!(buffer.pending_bytes(), before_bytes);
        assert_eq!(buffer.message_count(), before_count);
        assert_eq!(buffer.game_shop_receipt, before_receipt);
        assert_eq!(buffer.mail_cost_reserve, before_cost);
        assert_eq!(buffer.mail_stream_marker, before_marker);
        assert_eq!(buffer.mail_stream_epoch, before_epoch);
        assert!(buffer.active);
    }

    #[test]
    fn protected_marker_starts_on_full_fifo_and_delivery_cannot_overtake_it() {
        let _guard=native_queue_test_guard();let inbound=NativeInbound::new();
        for index in 0..MAX_NATIVE_MESSAGES {assert!(push_native_social_model(index.to_string()));}
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));
        assert!(push_native_mail_service(MailServiceDelivery{epoch:MAIL_EPOCH,event:MailServiceEvent::Cost{cost:125}}));
        let mut count=0;
        inbound.drain_matching(|message|matches!(message,NativeInboundMessage::MailService(_)),|_|count+=1);
        assert_eq!(count,0,"an incomplete consumer cannot skip the protected marker");
        assert_eq!(drain_mail(&inbound),vec![marker_delivery(),cost_delivery(125)]);
        assert!(push_native_mail_service_stream_started(MailServiceStreamStarted{epoch:MAIL_EPOCH}));
        assert!(drain_mail(&inbound).is_empty(),"same marker stays idempotent after it was consumed");
        assert_eq!(inbound.buffer.lock().unwrap().mail_stream_epoch,Some(MAIL_EPOCH));
    }

    #[test]
    fn hero_receipts_retain_each_snapshot_and_reset_with_session() {
        let mut buffer = active_buffer();
        assert!(buffer.enqueue(NativeInboundMessage::HeroModel("old".into())));
        for id in [51, 52] {
            assert!(buffer.enqueue(NativeInboundMessage::HeroModelReceipt(id.to_string())));
        }
        for serial in 0..1000 {
            assert!(buffer.enqueue(NativeInboundMessage::HeroModel(serial.to_string())));
        }
        assert_eq!(buffer.pending.len(), 3);
        assert!(matches!(&buffer.pending[0], NativeInboundMessage::HeroModelReceipt(v) if v=="51"));
        assert!(matches!(&buffer.pending[1], NativeInboundMessage::HeroModelReceipt(v) if v=="52"));
        assert!(matches!(&buffer.pending[2], NativeInboundMessage::HeroModel(v) if v=="999"));
        assert!(buffer.enqueue(NativeInboundMessage::DataReset));
        assert!(!buffer.pending.iter().any(|v| matches!(
            v,
            NativeInboundMessage::HeroModel(_) | NativeInboundMessage::HeroModelReceipt(_)
        )));
    }

    #[test]
    fn skill_receipt_is_not_coalesced_with_newer_skill_snapshots() {
        let mut buffer = active_buffer();
        let receipt = r#"{"skillKeyAck":{"requestId":73},"skills":[{"hotkey":16}]}"#.to_owned();
        assert!(buffer.enqueue(NativeInboundMessage::SkillModel("old".into())));
        assert!(buffer.enqueue(NativeInboundMessage::SkillModelReceipt(receipt.clone())));
        for serial in 0..1000 {
            assert!(buffer.enqueue(NativeInboundMessage::SkillModel(serial.to_string())));
        }
        let models: Vec<_> = buffer.pending.iter().collect();
        assert_eq!(models.len(), 2);
        assert!(
            matches!(models[0], NativeInboundMessage::SkillModelReceipt(json) if json == &receipt)
        );
        assert!(matches!(models[1], NativeInboundMessage::SkillModel(json) if json == "999"));
    }

    #[test]
    fn high_frequency_snapshots_are_coalesced_to_the_latest_value() {
        let mut buffer = active_buffer();
        for index in 0..10_000 {
            assert!(buffer.enqueue(NativeInboundMessage::WorldState(index.to_string())));
        }

        assert_eq!(buffer.pending.len(), 1);
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::WorldState(json)) if json == "9999"
        ));
    }

    #[test]
    fn non_critical_event_flood_is_bounded_and_reports_backpressure() {
        let mut buffer = active_buffer();
        for index in 0..NON_CRITICAL_MESSAGE_LIMIT {
            assert!(buffer.enqueue(NativeInboundMessage::ChatLine(index.to_string())));
        }

        assert!(!buffer.enqueue(NativeInboundMessage::ChatLine("overflow".to_owned())));
        assert_eq!(buffer.pending.len(), NON_CRITICAL_MESSAGE_LIMIT);
    }

    #[test]
    fn critical_ack_uses_reserved_capacity_after_non_critical_flood() {
        let mut buffer = active_buffer();
        for index in 0..NON_CRITICAL_MESSAGE_LIMIT {
            assert!(buffer.enqueue(NativeInboundMessage::ChatLine(index.to_string())));
        }

        assert!(buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
            "ack".to_owned()
        )));
        assert_eq!(buffer.pending.len(), NON_CRITICAL_MESSAGE_LIMIT + 1);
        assert!(buffer.pending.iter().any(|message| matches!(
            message,
            NativeInboundMessage::InventoryOperationAck(json) if json == "ack"
        )));
    }

    #[test]
    fn game_shop_receipt_has_an_independent_slot_and_survives_snapshot_flood() {
        let mut buffer = active_buffer();
        for index in 0..NON_CRITICAL_MESSAGE_LIMIT {
            assert!(buffer.enqueue(NativeInboundMessage::ChatLine(index.to_string())));
        }
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("gs-1"),)));
        assert!(buffer
            .game_shop_receipt
            .as_deref()
            .is_some_and(|json| json.contains("\"requestId\":\"gs-1\"")));
        assert_eq!(buffer.message_count(), NON_CRITICAL_MESSAGE_LIMIT + 1);
    }

    #[test]
    fn all_critical_flood_cannot_evict_the_reserved_game_shop_receipt() {
        let mut buffer = active_buffer();
        for index in 0..MAX_NATIVE_MESSAGES {
            assert!(buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string())));
        }
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("gs-1"),)));
        assert_eq!(buffer.message_count(), MAX_NATIVE_MESSAGES);
        for index in 0..1_000 {
            let _ = buffer.enqueue(NativeInboundMessage::SocialModel(format!("late-{index}")));
        }
        assert_eq!(buffer.message_count(), MAX_NATIVE_MESSAGES);
        assert!(buffer
            .game_shop_receipt
            .as_deref()
            .is_some_and(|json| json.contains("\"requestId\":\"gs-1\"")));
    }

    #[test]
    fn exact_receipt_cannot_be_replaced_by_later_valid_wrong_receipts() {
        let mut buffer = active_buffer();
        assert!(
            buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt(
                "gs-exact"
            ),))
        );
        for index in 1..=1_000 {
            assert!(
                !buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt(
                    &format!("gs-wrong-{index}")
                ),))
            );
        }
        assert_eq!(buffer.message_count(), 1);
        assert!(buffer
            .game_shop_receipt
            .as_deref()
            .is_some_and(|json| json.contains("\"requestId\":\"gs-exact\"")));
    }

    #[test]
    fn malformed_and_oversized_receipts_never_enter_the_reserve() {
        let mut buffer = active_buffer();
        assert!(!buffer.enqueue(NativeInboundMessage::GameShopReceipt("not-json".to_owned(),)));
        assert!(buffer.game_shop_receipt.is_none());

        let oversized = format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"gs-1","success":false,"gIndex":31,"quantity":2,"priceType":1,"code":"insufficientCurrency","padding":"{}"}}"#,
            "x".repeat(1_024),
        );
        assert!(!buffer.enqueue_with_limits(
            NativeInboundMessage::GameShopReceipt(oversized),
            512,
            2_048,
        ));
        assert!(buffer.game_shop_receipt.is_none());
    }

    #[test]
    fn scene_reset_preserves_receipt_reserve_but_data_reset_clears_it() {
        let mut buffer = active_buffer();
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(valid_receipt("gs-1"),)));
        assert!(buffer.enqueue(NativeInboundMessage::SceneReset));
        assert!(buffer.game_shop_receipt.is_some());
        assert!(buffer.enqueue(NativeInboundMessage::DataReset));
        assert!(buffer.game_shop_receipt.is_none());
        assert_eq!(buffer.message_count(), 1);
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::DataReset)
        ));
    }

    #[test]
    fn reset_barriers_preserve_process_lifetime_entity_atlases() {
        fn atlas(key: &str) -> NativeInboundMessage {
            NativeInboundMessage::EntityRenderAtlas {
                key: key.to_owned(),
                width: 1,
                height: 1,
                pixels: vec![0, 0, 0, 0],
            }
        }

        let mut scene = active_buffer();
        assert!(scene.enqueue(atlas("starter:p1")));
        assert!(scene.enqueue(NativeInboundMessage::WorldState("old".to_owned())));
        assert!(scene.enqueue(NativeInboundMessage::SceneReset));
        assert_eq!(scene.pending.len(), 2);
        assert!(matches!(
            scene.pending.front(),
            Some(NativeInboundMessage::EntityRenderAtlas { key, .. }) if key == "starter:p1"
        ));
        assert!(matches!(
            scene.pending.back(),
            Some(NativeInboundMessage::SceneReset)
        ));

        let mut data = active_buffer();
        assert!(data.enqueue(atlas("starter:p2")));
        assert!(data.enqueue(NativeInboundMessage::WorldState("old".to_owned())));
        assert!(data.enqueue(NativeInboundMessage::DataReset));
        assert_eq!(data.pending.len(), 2);
        assert!(matches!(
            data.pending.front(),
            Some(NativeInboundMessage::EntityRenderAtlas { key, .. }) if key == "starter:p2"
        ));
        assert!(matches!(
            data.pending.back(),
            Some(NativeInboundMessage::DataReset)
        ));
    }

    #[test]
    fn preserving_data_reset_atomically_rehydrates_and_keeps_exact_receipt() {
        let mut buffer = active_buffer();
        let receipt = typed_receipt("gs-preserved");
        assert!(buffer.enqueue(NativeInboundMessage::GameShopReceipt(
            serde_json::to_string(&receipt).unwrap(),
        )));
        // Model the render thread draining the reserve immediately before the
        // connection owner crosses the terminal boundary.
        buffer.game_shop_receipt = None;
        assert!(buffer.enqueue(NativeInboundMessage::WorldState("old-account".to_owned(),)));

        assert!(
            buffer.enqueue(NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt),)
        );
        assert_eq!(buffer.pending.len(), 1);
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::DataResetPreservingExactGameShopReceipt(receipt))
                if receipt.request_id == "gs-preserved"
        ));
        assert!(buffer
            .game_shop_receipt
            .as_deref()
            .is_some_and(|json| json.contains("\"requestId\":\"gs-preserved\"")));

        assert!(buffer.enqueue(NativeInboundMessage::DataReset));
        assert!(buffer.game_shop_receipt.is_none());
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::DataReset)
        ));
    }

    #[test]
    fn operation_ack_and_reset_survive_other_critical_message_floods() {
        let mut buffer = active_buffer();
        for index in 0..MAX_NATIVE_MESSAGES {
            assert!(buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string())));
        }
        assert!(buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
            "ack".to_owned()
        )));
        assert_eq!(buffer.pending.len(), MAX_NATIVE_MESSAGES);
        assert!(buffer.pending.iter().any(|message| matches!(
            message,
            NativeInboundMessage::InventoryOperationAck(json) if json == "ack"
        )));

        assert!(buffer.enqueue(NativeInboundMessage::DataReset));
        assert_eq!(buffer.pending.len(), 1);
        assert!(matches!(
            buffer.pending.front(),
            Some(NativeInboundMessage::DataReset)
        ));

        let mut scene_buffer = active_buffer();
        for index in 0..MAX_OPERATION_ACK_MESSAGES {
            assert!(
                scene_buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
                    index.to_string()
                ))
            );
        }
        assert!(
            !scene_buffer.enqueue(NativeInboundMessage::InventoryOperationAck(
                "overflow".to_owned()
            ))
        );
        for index in MAX_OPERATION_ACK_MESSAGES..MAX_NATIVE_MESSAGES {
            assert!(scene_buffer.enqueue(NativeInboundMessage::SocialModel(index.to_string())));
        }
        assert!(scene_buffer.enqueue(NativeInboundMessage::SceneReset));
        assert_eq!(scene_buffer.pending.len(), MAX_NATIVE_MESSAGES);
        assert!(matches!(
            scene_buffer.pending.back(),
            Some(NativeInboundMessage::SceneReset)
        ));
        assert_eq!(
            scene_buffer
                .pending
                .iter()
                .filter(|message| is_operation_ack(message))
                .count(),
            MAX_OPERATION_ACK_MESSAGES,
            "SceneReset must not discard operation ACKs to obtain capacity"
        );
    }

    #[test]
    fn single_message_and_total_payload_bytes_are_bounded() {
        let mut buffer = active_buffer();
        assert!(!buffer.enqueue_with_limits(
            NativeInboundMessage::EntityRenderAtlas {
                key: "atlas".to_owned(),
                width: 1,
                height: 1,
                pixels: vec![0; 8],
            },
            12,
            32,
        ));
        assert!(buffer.pending.is_empty());

        assert!(buffer.enqueue_with_limits(
            NativeInboundMessage::EntityRenderAtlas {
                key: "a".to_owned(),
                width: 1,
                height: 1,
                pixels: vec![0; 7],
            },
            8,
            10,
        ));
        assert!(buffer.enqueue_with_limits(
            NativeInboundMessage::WorldState("12345".to_owned()),
            8,
            10,
        ));
        assert!(buffer.pending_bytes() <= 10);
        assert_eq!(
            buffer.pending.len(),
            1,
            "old snapshot is evicted by byte pressure"
        );
    }

    #[test]
    fn snapshot_coalescing_keeps_post_reset_order() {
        let _native_queue_guard = native_queue_test_guard();
        let inbound = NativeInbound::new();
        assert!(push_native_world_state("old".to_owned()));
        assert!(push_native_scene_reset());
        assert!(push_native_world_state("new".to_owned()));

        inbound.discard_stale_data_before_latest_reset();
        let state = inbound
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");
        assert_eq!(state.pending.len(), 2);
        assert!(matches!(
            state.pending.front(),
            Some(NativeInboundMessage::SceneReset)
        ));
        assert!(matches!(
            state.pending.back(),
            Some(NativeInboundMessage::WorldState(json)) if json == "new"
        ));
    }

    #[test]
    fn type_specific_consumers_preserve_messages_for_later_consumers() {
        let _native_queue_guard = native_queue_test_guard();
        let inbound = NativeInbound::new();
        assert!(push_native_world_state("world".to_owned()));
        assert!(push_native_ui_read_model("ui".to_owned()));

        let mut worlds = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::WorldState(_)),
            |message| worlds.push(message),
        );

        let mut ui_models = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::UiReadModel(_)),
            |message| ui_models.push(message),
        );

        assert_eq!(worlds.len(), 1);
        assert_eq!(ui_models.len(), 1);
    }

    #[test]
    fn rebuilding_runtime_invalidates_only_the_previous_buffer() {
        let _native_queue_guard = native_queue_test_guard();
        let previous = NativeInbound::new();
        assert!(push_native_world_state("old".to_owned()));
        let previous_buffer = Arc::clone(&previous.buffer);
        let current = NativeInbound::new();
        assert!(
            !previous
                .buffer
                .lock()
                .expect("native inbound mutex should not be poisoned")
                .active
        );
        drop(previous);
        assert!(
            previous_buffer
                .lock()
                .expect("native inbound mutex should not be poisoned")
                .pending
                .is_empty(),
            "dropping a runtime must release retained payload memory"
        );

        assert!(push_native_world_state("current".to_owned()));
        let mut worlds = Vec::new();
        current.drain_matching(
            |message| matches!(message, NativeInboundMessage::WorldState(_)),
            |message| worlds.push(message),
        );
        assert!(matches!(
            worlds.as_slice(),
            [NativeInboundMessage::WorldState(json)] if json == "current"
        ));
    }

    #[test]
    fn reset_discards_queued_stale_models_but_keeps_models_after_reset() {
        let _native_queue_guard = native_queue_test_guard();
        let inbound = NativeInbound::new();
        assert!(push_native_mail_model(r#"{"mails":[]}"#.to_owned()));
        assert!(push_native_data_reset());
        assert!(push_native_mail_model(r#"{"mails":[{"id":1}]}"#.to_owned()));

        inbound.discard_stale_data_before_latest_reset();

        let mut resets = 0;
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::DataReset),
            |_| resets += 1,
        );
        let mut models = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::MailModel(_)),
            |message| models.push(message),
        );

        assert_eq!(resets, 1);
        assert_eq!(models.len(), 1);
        assert!(matches!(
            &models[0],
            NativeInboundMessage::MailModel(json) if json.contains("\"id\":1")
        ));
    }

    #[test]
    fn scene_reset_discards_old_scene_messages_but_keeps_personal_models() {
        let _native_queue_guard = native_queue_test_guard();
        let inbound = NativeInbound::new();
        assert!(push_native_world_state("old-world".to_owned()));
        assert!(push_native_inventory_model(
            r#"{"gold":10,"items":[]}"#.to_owned()
        ));
        assert!(push_native_social_model(
            r#"{"group":{"active":true},"guild":{},"trade":{}}"#.to_owned()
        ));
        assert!(push_native_scene_reset());
        assert!(push_native_world_state("new-world".to_owned()));
        assert!(push_native_inventory_model(
            r#"{"gold":20,"items":[]}"#.to_owned()
        ));

        inbound.discard_stale_data_before_latest_reset();

        let mut worlds = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::WorldState(_)),
            |message| worlds.push(message),
        );
        let mut inventories = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::InventoryModel(_)),
            |message| inventories.push(message),
        );

        assert_eq!(worlds.len(), 1);
        assert!(
            matches!(&worlds[0], NativeInboundMessage::WorldState(json) if json == "new-world")
        );
        assert_eq!(inventories.len(), 2);

        let mut social = Vec::new();
        inbound.drain_matching(
            |message| matches!(message, NativeInboundMessage::SocialModel(_)),
            |message| social.push(message),
        );
        assert_eq!(social.len(), 1, "social state is personal, not scene-local");
    }

    #[test]
    fn lighting_snapshot_coalesces_and_scene_reset_drops_only_the_stale_generation() {
        let _native_queue_guard = native_queue_test_guard();
        let inbound = NativeInbound::new();
        assert!(push_native_lighting_render_state("old-1".to_owned()));
        assert!(push_native_lighting_render_state("old-2".to_owned()));
        assert!(push_native_scene_reset());
        assert!(push_native_lighting_render_state("new".to_owned()));

        inbound.discard_stale_data_before_latest_reset();

        let state = inbound
            .buffer
            .lock()
            .expect("native inbound mutex should not be poisoned");
        assert_eq!(state.pending.len(), 2);
        assert!(matches!(
            state.pending.front(),
            Some(NativeInboundMessage::SceneReset)
        ));
        assert!(matches!(
            state.pending.back(),
            Some(NativeInboundMessage::LightingRenderState(json)) if json == "new"
        ));
    }
}

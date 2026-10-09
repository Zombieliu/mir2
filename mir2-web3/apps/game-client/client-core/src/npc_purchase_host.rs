//! Long-lived, single-writer custody for NPC purchase receipt ledgers.
//!
//! Keep this host outside disposable UI facades. Connection and Begin tickets
//! prove only local callback custody; they do not authenticate a server. The
//! adapter calls `bind` only for an authenticated, accepted server Begin whose
//! echoed control ID matches the saved ticket on its original connection.
//! Receipts and complete applied witnesses never create or select authority.
//! No method sends, retries, charges, delivers, resets or evicts actor history.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::npc_purchase_receipt::{
    ActorKey, NpcPurchaseReceiptLedger, Observation, OperationKey, OwnerScope,
    PendingPurchase, PurchaseIntent, PurchaseReceipt, RecoveryQuery, SnapshotWitness,
};

pub const MAX_RETAINED_ACTORS: usize = 64;
static LAST_HOST: AtomicUsize = AtomicUsize::new(0);

/// Opaque local physical-connection custody, never a server producer identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionToken { host: usize, sequence: u64 }

/// Latest outstanding Begin on one physical connection. Only the host mints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginTicket { connection: ConnectionToken, sequence: u64 }
impl BeginTicket {
    /// Send as the strict wire control request ID and require its exact echo.
    pub fn sequence(self) -> u64 { self.sequence }
    pub fn connection(self) -> ConnectionToken { self.connection }
}

/// Server authority accepted for this specific connection and Begin epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurchaseBinding { ticket: BeginTicket, actor: ActorKey, producer_scope: OwnerScope }
impl PurchaseBinding {
    pub fn connection(self) -> ConnectionToken { self.ticket.connection }
    pub fn actor(self) -> ActorKey { self.actor }
    pub fn producer_scope(self) -> OwnerScope { self.producer_scope }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindError { StaleTicket, ActorCapacity, RetiredProducer }

#[derive(Debug)]
struct ActorCustody {
    ledger: NpcPurchaseReceiptLedger,
    producer_connection: ConnectionToken,
    reservation_binding: Option<PurchaseBinding>,
}

/// Bounded actor retention; not Clone and no mutable ledger escape hatch.
#[derive(Debug)]
pub struct NpcPurchaseReceiptHost {
    host: usize,
    capacity: usize,
    last_connection: u64,
    last_begin: u64,
    connection: Option<ConnectionToken>,
    begin: Option<BeginTicket>,
    current: Option<PurchaseBinding>,
    actors: BTreeMap<ActorKey, ActorCustody>,
}

impl NpcPurchaseReceiptHost {
    /// Returns None only if the checked host identity allocator is exhausted.
    pub fn new() -> Option<Self> { Self::with_capacity(MAX_RETAINED_ACTORS) }

    /// Smaller capacities are useful for bounded embedders. Never exceeds 64.
    pub fn with_capacity(capacity: usize) -> Option<Self> {
        if capacity == 0 || capacity > MAX_RETAINED_ACTORS { return None; }
        let previous = LAST_HOST.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
            |value| value.checked_add(1)).ok()?;
        Some(Self { host: previous + 1, capacity, last_connection: 0, last_begin: 0,
            connection: None, begin: None, current: None, actors: BTreeMap::new() })
    }

    pub fn retained_actor_count(&self) -> usize { self.actors.len() }
    pub fn capacity(&self) -> usize { self.capacity }
    pub fn connection(&self) -> Option<ConnectionToken> { self.connection }
    pub fn current_binding(&self) -> Option<PurchaseBinding> { self.current }
    pub fn ledger(&self, actor: ActorKey) -> Option<&NpcPurchaseReceiptLedger> {
        self.actors.get(&actor).map(|custody| &custody.ledger)
    }
    pub fn pending(&self, actor: ActorKey) -> Option<PendingPurchase> {
        self.ledger(actor)?.pending()
    }
    pub fn pending_current(&self) -> Option<PendingPurchase> {
        self.pending(self.current?.actor)
    }

    /// Replace physical transport availability, keeping every actor's custody.
    /// The adapter must call this for every new socket, even on the same account.
    pub fn open_connection(&mut self) -> Option<ConnectionToken> {
        let sequence = self.last_connection.checked_add(1)?;
        let connection = ConnectionToken { host: self.host, sequence };
        self.withdraw_selected_authority();
        self.last_connection = sequence;
        self.connection = Some(connection);
        self.begin = None;
        self.current = None;
        Some(connection)
    }

    /// Disable current capability on selection/logout/capability withdrawal.
    /// Leaves the physical transport available for a fresh accepted Begin.
    pub fn withdraw_current(&mut self, connection: ConnectionToken) -> bool {
        if self.connection != Some(connection) { return false; }
        self.withdraw_selected_authority();
        self.begin = None;
        self.current = None;
        true
    }

    /// Retire disconnected callbacks; none can restore this physical token.
    pub fn withdraw_connection(&mut self, connection: ConnectionToken) -> bool {
        if !self.withdraw_current(connection) { return false; }
        self.connection = None;
        true
    }

    fn withdraw_selected_authority(&mut self) {
        if let Some(binding) = self.current.take() {
            if let Some(custody) = self.actors.get_mut(&binding.actor) {
                custody.ledger.withdraw_authority();
            }
        }
    }

    /// Mint latest control request custody, withdrawing the selected authority
    /// and applied evidence without releasing any original economic operation.
    pub fn request_begin(&mut self, connection: ConnectionToken) -> Option<BeginTicket> {
        if self.connection != Some(connection) { return None; }
        let sequence = self.last_begin.checked_add(1)?;
        let ticket = BeginTicket { connection, sequence };
        self.withdraw_selected_authority();
        self.last_begin = sequence;
        self.begin = Some(ticket);
        Some(ticket)
    }

    /// Trusted adapter only: authenticated server actor/producer and exact echoed
    /// ticket. A failed bind changes no binding, ticket, ledger or allocator.
    /// Same-owner Begin may reuse a producer, but never its old applied evidence.
    /// Capacity refusal retains every old ledger, operation and allocator; Begin
    /// request already withdrew availability before this accepted response.
    pub fn bind(&mut self, ticket: BeginTicket, actor: ActorKey, producer_scope: OwnerScope)
        -> Result<PurchaseBinding, BindError>
    {
        if self.connection != Some(ticket.connection) || self.begin != Some(ticket) {
            return Err(BindError::StaleTicket);
        }
        if let Some(custody) = self.actors.get_mut(&actor) {
            if !custody.ledger.rebind(actor, producer_scope) {
                return Err(BindError::RetiredProducer);
            }
            custody.ledger.withdraw_authority();
            custody.producer_connection = ticket.connection;
        } else {
            if self.actors.len() >= self.capacity { return Err(BindError::ActorCapacity); }
            self.actors.insert(actor, ActorCustody {
                ledger: NpcPurchaseReceiptLedger::new(actor, producer_scope),
                producer_connection: ticket.connection,
                reservation_binding: None,
            });
        }
        let binding = PurchaseBinding { ticket, actor, producer_scope };
        self.current = Some(binding);
        self.begin = None;
        Ok(binding)
    }

    fn current_custody_mut(&mut self, binding: PurchaseBinding) -> Option<&mut ActorCustody> {
        if self.current != Some(binding) || self.connection != Some(binding.connection()) { return None; }
        let custody = self.actors.get_mut(&binding.actor)?;
        (custody.producer_connection == binding.connection()
            && custody.ledger.producer_scope() == binding.producer_scope).then_some(custody)
    }

    pub fn reserve(&mut self, binding: PurchaseBinding, intent: PurchaseIntent) -> Option<OperationKey> {
        let custody = self.current_custody_mut(binding)?;
        let key = custody.ledger.reserve(intent)?;
        custody.reservation_binding = Some(binding);
        Some(key)
    }

    /// Adjacent to the adapter's one physical send; still does not send itself.
    pub fn begin_entry(&mut self, binding: PurchaseBinding, key: OperationKey, intent: PurchaseIntent) -> bool {
        self.current_custody_mut(binding).is_some_and(|custody|
            custody.reservation_binding == Some(binding)
                && custody.ledger.begin_entry(key, intent, binding.producer_scope))
    }

    /// Original queued operation custody may be canceled while offline or while
    /// another actor is selected, only with definite pre-entry unsent evidence.
    pub fn cancel_definitely_unsent(&mut self, key: OperationKey, intent: PurchaseIntent) -> bool {
        let Some(custody) = self.actors.get_mut(&key.actor) else { return false; };
        if !custody.ledger.cancel_definitely_unsent(key, intent) { return false; }
        custody.reservation_binding = None;
        true
    }

    /// Local feedback from a former sender can preserve, never release, custody.
    pub fn mark_unknown(&mut self, key: OperationKey) -> bool {
        self.actors.get_mut(&key.actor).is_some_and(|custody| custody.ledger.mark_unknown(key))
    }

    pub fn recovery_query(&self, binding: PurchaseBinding) -> Option<RecoveryQuery> {
        if self.current != Some(binding) || self.connection != Some(binding.connection()) { return None; }
        self.ledger(binding.actor)?.recovery_query()
    }

    /// Stale selected-actor frames are ignored, not used to select their actor.
    pub fn observe_receipt(&mut self, binding: PurchaseBinding, receipt: PurchaseReceipt) -> Observation {
        self.current_custody_mut(binding).map_or(Observation::Ignored, |custody| {
            let result = custody.ledger.observe_receipt(receipt);
            if matches!(result, Observation::Settled(_)) { custody.reservation_binding = None; }
            result
        })
    }

    /// Only after the same authoritative economy bundle is completely applied.
    /// Decode, enqueue, inventory-only updates and movement are not witnesses.
    pub fn observe_applied_snapshot(&mut self, binding: PurchaseBinding, witness: SnapshotWitness) -> Observation {
        self.current_custody_mut(binding).map_or(Observation::Ignored, |custody| {
            let result = custody.ledger.observe_snapshot(witness);
            if matches!(result, Observation::Settled(_)) { custody.reservation_binding = None; }
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc_purchase_receipt::{EconomicResult, PurchaseCurrency, PurchasePhase,
        PurchaseSource, ServiceCatalogProof};

    // Pure-state fixtures only; these bytes do not authenticate a production peer.
    fn actor(value: u8) -> ActorKey { ActorKey::from_server_bytes([value; 32]).unwrap() }
    fn scope(value: u8) -> OwnerScope { OwnerScope::from_server_bytes([value; 32]).unwrap() }
    fn intent() -> PurchaseIntent {
        PurchaseIntent { item_index: 0, requested_count: 3, panel: 0,
            currency: PurchaseCurrency::Pearls, source: PurchaseSource::Used,
            service_catalog_proof: ServiceCatalogProof::from_server_bytes([7; 32]).unwrap() }
    }
    fn bind(host: &mut NpcPurchaseReceiptHost, connection: ConnectionToken, a: u8, p: u8) -> PurchaseBinding {
        let ticket = host.request_begin(connection).unwrap();
        host.bind(ticket, actor(a), scope(p)).unwrap()
    }
    fn witness(binding: PurchaseBinding, revision: u64) -> SnapshotWitness {
        SnapshotWitness { actor: binding.actor(), producer_scope: binding.producer_scope(),
            server_revision: revision, complete: true }
    }
    fn receipt(binding: PurchaseBinding, key: OperationKey, revision: u64) -> PurchaseReceipt {
        PurchaseReceipt { actor: key.actor, producer_scope: binding.producer_scope(), request_id: key.request_id,
            intent: intent(), result: EconomicResult::Committed { server_revision: revision,
                currency: intent().currency, source: intent().source, charged: 9, admitted_count: 2, incoming_uid: 0 } }
    }
    fn ready() -> (NpcPurchaseReceiptHost, ConnectionToken, PurchaseBinding) {
        let mut host = NpcPurchaseReceiptHost::new().unwrap();
        let connection = host.open_connection().unwrap();
        let binding = bind(&mut host, connection, 1, 2);
        assert_eq!(host.observe_applied_snapshot(binding, witness(binding, 10)), Observation::Pending);
        (host, connection, binding)
    }
    fn enter(host: &mut NpcPurchaseReceiptHost, binding: PurchaseBinding) -> OperationKey {
        let key = host.reserve(binding, intent()).unwrap();
        assert!(host.begin_entry(binding, key, intent()));
        key
    }
    fn settled(observation: Observation, key: OperationKey) {
        assert!(matches!(observation, Observation::Settled(value) if value.key == key && value.intent == intent()));
    }

    #[test]
    fn no_authority_or_admission_before_accepted_begin_and_complete_baseline() {
        let mut host = NpcPurchaseReceiptHost::new().unwrap();
        let connection = host.open_connection().unwrap();
        let ticket = host.request_begin(connection).unwrap();
        assert_ne!(ticket.sequence(), 0);
        assert_eq!(host.current_binding(), None);
        assert_eq!(host.retained_actor_count(), 0);
        let binding = host.bind(ticket, actor(1), scope(2)).unwrap();
        assert_eq!(host.reserve(binding, intent()), None);
        let partial = SnapshotWitness { complete: false, ..witness(binding, 10) };
        assert_eq!(host.observe_applied_snapshot(binding, partial), Observation::Ignored);
        assert_eq!(host.reserve(binding, intent()), None);
        host.observe_applied_snapshot(binding, witness(binding, 10));
        assert_eq!(host.reserve(binding, intent()).unwrap().request_id.sequence(), 1);
    }

    #[test]
    fn settlement_requires_exact_receipt_and_complete_applied_bundle_in_both_orders() {
        for snapshot_first in [false, true] {
            let (mut host, _, binding) = ready();
            let key = enter(&mut host, binding);
            let receipt = receipt(binding, key, 11);
            let result = if snapshot_first {
                assert_eq!(host.observe_applied_snapshot(binding, witness(binding, 11)), Observation::Pending);
                host.observe_receipt(binding, receipt)
            } else {
                assert_eq!(host.observe_receipt(binding, receipt), Observation::Pending);
                host.observe_applied_snapshot(binding, witness(binding, 11))
            };
            settled(result, key);
            assert_eq!(host.pending_current(), None);
            assert_eq!(host.observe_receipt(binding, receipt), Observation::Ignored);
        }
    }

    #[test]
    fn disposable_ui_facades_cannot_release_entered_custody_or_allocator() {
        let (mut host, connection, binding) = ready();
        let key = enter(&mut host, binding);
        // Disposable UI state holds a copy of the binding, never the host.
        for _ in 0..3 {
            let facade = host.current_binding().unwrap();
            assert_eq!(facade, binding);
            assert_eq!(host.pending_current().unwrap().key, key);
            assert_eq!(host.reserve(facade, intent()), None);
        }
        assert!(host.withdraw_current(connection));
        assert_eq!(host.current_binding(), None);
        assert!(host.mark_unknown(key));
        assert!(!host.cancel_definitely_unsent(key, intent()));
        assert_eq!(host.pending(actor(1)).unwrap().phase, PurchasePhase::Unknown);
        assert_eq!(host.ledger(actor(1)).unwrap().last_sequence(), 1);
    }

    #[test]
    fn reconnect_queries_original_operation_without_purchase_retry_or_new_id() {
        let (mut host, old_connection, old_binding) = ready();
        let key = enter(&mut host, old_binding);
        host.mark_unknown(key);
        let old_receipt = receipt(old_binding, key, 11);
        assert!(host.withdraw_connection(old_connection));
        let connection = host.open_connection().unwrap();
        let binding = bind(&mut host, connection, 1, 3);
        assert_eq!(host.ledger(actor(1)).unwrap().applied_baseline(), None);
        for _ in 0..3 {
            assert_eq!(host.recovery_query(binding), Some(RecoveryQuery {
                key, intent: intent(), producer_scope: scope(3) }));
            assert_eq!(host.reserve(binding, intent()), None);
            assert!(!host.begin_entry(binding, key, intent()));
        }
        assert_eq!(host.observe_receipt(old_binding, old_receipt), Observation::Ignored);
        assert_eq!(host.observe_applied_snapshot(old_binding, witness(old_binding, 11)), Observation::Ignored);
        host.observe_receipt(binding, PurchaseReceipt { producer_scope: scope(3), ..old_receipt });
        settled(host.observe_applied_snapshot(binding, witness(binding, 11)), key);
        let next = host.reserve(binding, intent()).unwrap();
        assert_eq!(next.request_id.sequence(), 2);
        assert_eq!(next.request_id.scope(), scope(3));
    }

    #[test]
    fn foreign_actor_selection_preserves_original_flight_and_rejects_old_selected_frames() {
        let (mut host, connection, original) = ready();
        let original_key = enter(&mut host, original);
        let foreign = bind(&mut host, connection, 9, 8);
        host.observe_applied_snapshot(foreign, witness(foreign, 10));
        let foreign_key = enter(&mut host, foreign);
        assert_eq!(host.pending(actor(1)).unwrap().key, original_key);
        assert_eq!(host.observe_receipt(original, receipt(original, original_key, 11)), Observation::Ignored);
        assert_eq!(host.observe_receipt(foreign, receipt(original, original_key, 11)), Observation::Ignored);
        assert_eq!(host.observe_applied_snapshot(foreign, witness(original, 11)), Observation::Ignored);
        assert_eq!(host.current_binding(), Some(foreign));
        assert_eq!(host.pending_current().unwrap().key, foreign_key);
        let resumed = bind(&mut host, connection, 1, 3);
        assert_eq!(host.recovery_query(resumed).unwrap().key, original_key);
        assert_eq!(host.pending(actor(9)).unwrap().key, foreign_key);
        assert_eq!(host.retained_actor_count(), 2);
    }

    #[test]
    fn delayed_begin_same_socket_and_old_connection_cannot_select_actor() {
        let (mut host, connection, _) = ready();
        let stale = host.request_begin(connection).unwrap();
        let latest = host.request_begin(connection).unwrap();
        assert!(latest.sequence() > stale.sequence());
        assert_eq!(host.bind(stale, actor(9), scope(8)), Err(BindError::StaleTicket));
        assert_eq!(host.current_binding(), None);
        let selected = host.bind(latest, actor(9), scope(8)).unwrap();
        assert_eq!(host.bind(latest, actor(1), scope(3)), Err(BindError::StaleTicket));
        let abandoned = host.request_begin(connection).unwrap();
        let new_connection = host.open_connection().unwrap();
        assert!(!host.withdraw_connection(connection));
        assert_eq!(host.bind(abandoned, actor(1), scope(3)), Err(BindError::StaleTicket));
        assert_eq!(host.observe_applied_snapshot(selected, witness(selected, 11)), Observation::Ignored);
        assert_eq!(host.connection(), Some(new_connection));
        assert_eq!(host.current_binding(), None);
    }

    #[test]
    fn same_producer_new_transport_requires_fresh_receipt_and_applied_witness() {
        for snapshot_first in [false, true] {
            let (mut host, _, binding) = ready();
            let key = enter(&mut host, binding);
            let old_receipt = receipt(binding, key, 11);
            host.observe_applied_snapshot(binding, witness(binding, 11));
            let connection = host.open_connection().unwrap();
            let resumed = bind(&mut host, connection, 1, 2);
            assert_eq!(host.pending_current().unwrap().key, key);
            assert_eq!(host.pending_current().unwrap().phase, PurchasePhase::Unknown);
            assert_eq!(host.ledger(actor(1)).unwrap().applied_baseline(), None);
            assert_eq!(host.reserve(resumed, intent()), None);
            assert!(!host.begin_entry(resumed, key, intent()));
            assert_eq!(host.recovery_query(resumed).unwrap().key, key);
            assert_eq!(host.observe_receipt(binding, old_receipt), Observation::Ignored);
            let result = if snapshot_first {
                assert_eq!(host.observe_applied_snapshot(resumed, witness(resumed, 11)), Observation::Pending);
                host.observe_receipt(resumed, old_receipt)
            } else {
                assert_eq!(host.observe_receipt(resumed, old_receipt), Observation::Pending);
                host.observe_applied_snapshot(resumed, witness(resumed, 11))
            };
            settled(result, key);
            assert_eq!(host.reserve(resumed, intent()).unwrap().request_id.sequence(), 2);
        }
    }

    #[test]
    fn same_socket_begin_withdraws_old_evidence_and_retired_producer_aba_cannot_return() {
        let (mut host, _, binding) = ready();
        let key = enter(&mut host, binding);
        host.observe_receipt(binding, receipt(binding, key, 11));
        let connection = host.open_connection().unwrap();
        let ticket = host.request_begin(connection).unwrap();
        assert_eq!(host.current_binding(), None);
        assert_eq!(host.pending(actor(1)).unwrap().key, key);
        let reused = host.bind(ticket, actor(1), scope(2)).unwrap();
        assert_eq!(host.pending_current().unwrap().phase, PurchasePhase::Unknown);
        assert_eq!(host.observe_applied_snapshot(reused, witness(reused, 11)), Observation::Pending);
        let next_ticket = host.request_begin(connection).unwrap();
        assert_eq!(host.reserve(reused, intent()), None);
        assert_eq!(host.recovery_query(reused), None);
        assert_eq!(host.ledger(actor(1)).unwrap().applied_baseline(), None);
        let resumed = host.bind(next_ticket, actor(1), scope(3)).unwrap();
        let aba = host.request_begin(connection).unwrap();
        assert_eq!(host.bind(aba, actor(1), scope(2)), Err(BindError::RetiredProducer));
        assert_eq!(host.current_binding(), None);
        assert_eq!(host.ledger(actor(1)).unwrap().producer_scope(), scope(3));
        assert_eq!(host.pending(actor(1)).unwrap().key, key);
        assert_eq!(host.recovery_query(resumed), None);
    }

    #[test]
    fn queued_old_producer_only_allows_exact_definitely_unsent_cancellation() {
        let (mut host, connection, binding) = ready();
        let key = host.reserve(binding, intent()).unwrap();
        let resumed = bind(&mut host, connection, 1, 3);
        host.observe_applied_snapshot(resumed, witness(resumed, 10));
        assert!(!host.begin_entry(binding, key, intent()));
        assert!(!host.begin_entry(resumed, key, intent()));
        assert_eq!(host.recovery_query(resumed), None);
        assert!(!host.cancel_definitely_unsent(key, PurchaseIntent { panel: 1, ..intent() }));
        assert!(host.cancel_definitely_unsent(key, intent()));
        assert_eq!(host.reserve(resumed, intent()).unwrap().request_id.sequence(), 2);
    }

    #[test]
    fn queued_same_producer_on_new_transport_cannot_enter_or_query() {
        let (mut host, _, original) = ready();
        let key = host.reserve(original, intent()).unwrap();
        let connection = host.open_connection().unwrap();
        let resumed = bind(&mut host, connection, 1, 2);
        host.observe_applied_snapshot(resumed, witness(resumed, 10));
        assert!(!host.begin_entry(original, key, intent()));
        assert!(!host.begin_entry(resumed, key, intent()));
        assert_eq!(host.recovery_query(resumed), None);
        assert_eq!(host.pending_current().unwrap().phase, PurchasePhase::Queued);
        assert!(host.cancel_definitely_unsent(key, intent()));
        let next = host.reserve(resumed, intent()).unwrap();
        assert_eq!(next.request_id.sequence(), 2);
        assert!(host.begin_entry(resumed, next, intent()));
    }

    #[test]
    fn default_actor_cap_preserves_all_entered_flights_without_eviction() {
        let mut host = NpcPurchaseReceiptHost::new().unwrap();
        let connection = host.open_connection().unwrap();
        assert_eq!(host.capacity(), MAX_RETAINED_ACTORS);
        for value in 1..=MAX_RETAINED_ACTORS as u8 {
            let binding = bind(&mut host, connection, value, value);
            host.observe_applied_snapshot(binding, witness(binding, 10));
            enter(&mut host, binding);
        }
        let ticket = host.request_begin(connection).unwrap();
        assert_eq!(host.bind(ticket, actor(65), scope(65)), Err(BindError::ActorCapacity));
        assert_eq!(host.retained_actor_count(), MAX_RETAINED_ACTORS);
        assert!(host.ledger(actor(65)).is_none());
        for value in 1..=MAX_RETAINED_ACTORS as u8 {
            let pending = host.pending(actor(value)).unwrap();
            assert_eq!(pending.key.actor, actor(value));
            assert_eq!(pending.key.request_id.sequence(), 1);
            assert_eq!(pending.key.request_id.scope(), scope(value));
            assert_eq!(pending.phase, PurchasePhase::Unknown);
            assert_eq!(host.ledger(actor(value)).unwrap().last_sequence(), 1);
        }
    }

    #[test]
    fn actor_capacity_refusal_is_atomic_and_never_evicts_even_settled_history() {
        let mut host = NpcPurchaseReceiptHost::with_capacity(2).unwrap();
        let connection = host.open_connection().unwrap();
        let original = bind(&mut host, connection, 1, 2);
        host.observe_applied_snapshot(original, witness(original, 10));
        let key = enter(&mut host, original);
        host.observe_receipt(original, receipt(original, key, 11));
        settled(host.observe_applied_snapshot(original, witness(original, 11)), key);
        let selected = bind(&mut host, connection, 9, 8);
        host.observe_applied_snapshot(selected, witness(selected, 10));
        let pending = enter(&mut host, selected);
        let ticket = host.request_begin(connection).unwrap();
        assert_eq!(host.bind(ticket, actor(6), scope(6)), Err(BindError::ActorCapacity));
        assert_eq!(host.current_binding(), None);
        assert_eq!(host.retained_actor_count(), 2);
        assert!(host.ledger(actor(6)).is_none());
        assert_eq!(host.ledger(actor(1)).unwrap().last_sequence(), 1);
        assert_eq!(host.pending(actor(9)).unwrap().key, pending);
        let resumed = host.bind(ticket, actor(1), scope(3)).unwrap();
        host.observe_applied_snapshot(resumed, witness(resumed, 11));
        assert_eq!(host.reserve(resumed, intent()).unwrap().request_id.sequence(), 2);
        assert_eq!(host.pending(actor(9)).unwrap().key, pending);
    }

    #[test]
    fn tokens_from_another_host_and_checked_control_exhaustion_preserve_state() {
        let (mut host, connection, binding) = ready();
        let (mut other, other_connection, _) = ready();
        assert_eq!(host.request_begin(other_connection), None);
        let other_ticket = other.request_begin(other_connection).unwrap();
        assert_eq!(host.bind(other_ticket, actor(9), scope(8)), Err(BindError::StaleTicket));
        host.last_begin = u64::MAX;
        assert_eq!(host.request_begin(connection), None);
        host.last_connection = u64::MAX;
        assert_eq!(host.open_connection(), None);
        assert_eq!(host.current_binding(), Some(binding));
        assert_eq!(host.connection(), Some(connection));
        assert!(NpcPurchaseReceiptHost::with_capacity(0).is_none());
        assert!(NpcPurchaseReceiptHost::with_capacity(MAX_RETAINED_ACTORS + 1).is_none());
    }
}

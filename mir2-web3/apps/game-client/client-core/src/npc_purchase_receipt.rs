//! Pure presentation custody for correlated NPC economic results.
//!
//! One ledger belongs to one server-authorized account/character ActorKey and
//! admits one economic flight across every currency/source. It is independent
//! of npc_gold_buy_attempt and never prices, charges, delivers, retries or sends.
//! Hosts must retain this non-Clone ledger and its allocator across UI remounts,
//! connection changes and timeout/withdrawal. Durable journal, protocol, ABI and
//! GUI integration are not implemented here.
//!
//! All producer binding, receipts and complete applied-snapshot witnesses are
//! trusted-host inputs. Nonzero opaque bytes do not authenticate a network peer:
//! the host must verify the server-issued ActorKey and current producer scope.
//! Server economic revisions must be monotonic for that stable actor, including
//! durable terminal rejections. A complete applied checkpoint at revision R must
//! contain all of that actor's economic results through R. Local snapshot clocks
//! and inventory membership cannot establish this checkpoint guarantee.

use std::collections::BTreeSet;

macro_rules! opaque_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);
        impl $name {
            /// The host supplies authenticated server-issued bytes, never a
            /// local account name, transport counter or client-generated ID.
            pub fn from_server_bytes(bytes: [u8; 32]) -> Option<Self> {
                bytes.iter().any(|byte| *byte != 0).then_some(Self(bytes))
            }
            pub fn bytes(&self) -> &[u8; 32] { &self.0 }
        }
    };
}

opaque_identity!(
    /// Stable, non-reusable server identity for the account + character pair.
    ActorKey
);
opaque_identity!(
    /// Current trusted producer incarnation, not the operation's actor identity.
    OwnerScope
);
opaque_identity!(
    /// Actual server-owned service/catalog proof captured with the purchase intent.
    ServiceCatalogProof
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId { scope: OwnerScope, sequence: u64 }
impl RequestId {
    /// Decode an existing key; only the ledger allocates keys for new requests.
    pub fn from_parts(scope: OwnerScope, sequence: u64) -> Option<Self> {
        (sequence != 0).then_some(Self { scope, sequence })
    }
    pub fn scope(self) -> OwnerScope { self.scope }
    pub fn sequence(self) -> u64 { self.sequence }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationKey { pub actor: ActorKey, pub request_id: RequestId }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseCurrency { Gold, Pearls }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseSource { Trade, BuyBack, Used }

/// Raw item_index is the actual u64 BuyItem goods identity, including zero;
/// it must not be reconstructed from a template index or requester inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurchaseIntent {
    pub item_index: u64,
    pub requested_count: u16,
    pub panel: u8,
    pub currency: PurchaseCurrency,
    pub source: PurchaseSource,
    pub service_catalog_proof: ServiceCatalogProof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EconomicResult {
    Committed { server_revision: u64, currency: PurchaseCurrency, source: PurchaseSource,
        charged: u32, admitted_count: u16, incoming_uid: u64 },
    Rejected { server_revision: u64 },
    Unknown,
}
impl EconomicResult {
    fn terminal_revision(self) -> Option<u64> {
        match self {
            Self::Committed { server_revision, .. } | Self::Rejected { server_revision } => Some(server_revision),
            Self::Unknown => None,
        }
    }
    fn valid_for(self, intent: PurchaseIntent, minimum_revision: u64) -> bool {
        match self {
            Self::Committed { server_revision, currency, source, admitted_count, .. } =>
                server_revision >= minimum_revision && server_revision != 0 && server_revision != u64::MAX && currency == intent.currency
                    && source == intent.source && admitted_count > 0 && admitted_count <= intent.requested_count,
            Self::Rejected { server_revision } => server_revision >= minimum_revision && server_revision != 0 && server_revision != u64::MAX,
            Self::Unknown => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurchaseReceipt {
    pub actor: ActorKey,
    pub producer_scope: OwnerScope,
    pub request_id: RequestId,
    pub intent: PurchaseIntent,
    pub result: EconomicResult,
}

/// A complete, already-applied authoritative snapshot for this actor. This is
/// not local UI readiness, worldSnapshotVersion, or presence of incoming_uid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotWitness {
    pub actor: ActorKey,
    pub producer_scope: OwnerScope,
    pub server_revision: u64,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchasePhase { Queued, Entered, Unknown, AwaitingSnapshot }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingPurchase {
    pub key: OperationKey,
    pub intent: PurchaseIntent,
    pub minimum_revision: u64,
    pub phase: PurchasePhase,
}

/// Read-only query: use the original operation and full tuple on the current
/// trusted producer. It allocates no replacement ID and authorizes no purchase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryQuery {
    pub key: OperationKey,
    pub intent: PurchaseIntent,
    pub producer_scope: OwnerScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurchaseSettlement { pub key: OperationKey, pub intent: PurchaseIntent, pub result: EconomicResult }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation { Ignored, Pending, Settled(PurchaseSettlement) }

#[derive(Debug)]
struct Flight {
    key: OperationKey,
    intent: PurchaseIntent,
    minimum_revision: u64,
    entered: bool,
    unknown: bool,
    terminal: Option<EconomicResult>,
    witness_revision: Option<u64>,
}
impl Flight {
    fn phase(&self) -> PurchasePhase {
        if !self.entered { PurchasePhase::Queued }
        else if self.terminal.is_some() { PurchasePhase::AwaitingSnapshot }
        else if self.unknown { PurchasePhase::Unknown }
        else { PurchasePhase::Entered }
    }
}

/// Synchronous, single-writer economic presentation barrier. It intentionally
/// has no reset, timeout, Clone, UI visibility, inventory or transport APIs.
#[derive(Debug)]
pub struct NpcPurchaseReceiptLedger {
    actor: ActorKey,
    producer_scope: OwnerScope,
    retired_producers: BTreeSet<OwnerScope>,
    last_sequence: u64,
    highest_revision: u64,
    applied_baseline: Option<u64>,
    flight: Option<Flight>,
}

impl NpcPurchaseReceiptLedger {
    /// No initial admission is possible until a complete trusted snapshot has
    /// established this producer's known server economic revision (including a
    /// legitimate initial revision zero).
    pub fn new(actor: ActorKey, producer_scope: OwnerScope) -> Self {
        Self { actor, producer_scope, retired_producers: BTreeSet::new(), last_sequence: 0,
            highest_revision: 0, applied_baseline: None, flight: None }
    }

    pub fn actor(&self) -> ActorKey { self.actor }
    pub fn producer_scope(&self) -> OwnerScope { self.producer_scope }
    pub fn last_sequence(&self) -> u64 { self.last_sequence }
    pub fn applied_baseline(&self) -> Option<u64> { self.applied_baseline }
    pub fn pending(&self) -> Option<PendingPurchase> {
        self.flight.as_ref().map(|flight| PendingPurchase { key: flight.key, intent: flight.intent,
            minimum_revision: flight.minimum_revision, phase: flight.phase() })
    }

    /// A foreign actor cannot take over this ledger. Hosts retain unresolved
    /// ledgers per actor. Retired incarnation reuse (A -> B -> A) is refused.
    /// Same-actor rebind preserves the original key/tuple and every entered
    /// barrier, but requires fresh receipt and snapshot evidence for recovery.
    pub fn rebind(&mut self, actor: ActorKey, producer_scope: OwnerScope) -> bool {
        if actor != self.actor || self.retired_producers.contains(&producer_scope) { return false; }
        if producer_scope == self.producer_scope { return true; }
        self.retired_producers.insert(self.producer_scope);
        self.producer_scope = producer_scope;
        self.applied_baseline = None;
        if let Some(flight) = &mut self.flight {
            flight.terminal = None;
            flight.witness_revision = None;
            if flight.entered { flight.unknown = true; }
        }
        true
    }

    /// Withdraw trusted producer evidence when its physical host binding ends.
    /// The original operation and allocator survive, including a same-producer
    /// reconnect. Only fresh receipt and complete application can settle it.
    pub fn withdraw_authority(&mut self) {
        self.applied_baseline = None;
        if let Some(flight) = &mut self.flight {
            flight.terminal = None;
            flight.witness_revision = None;
            if flight.entered { flight.unknown = true; }
        }
    }

    fn next_minimum_revision(&self) -> Option<u64> {
        let baseline = self.applied_baseline?;
        baseline.max(self.highest_revision).checked_add(1).filter(|revision| *revision != u64::MAX)
    }

    pub fn reserve(&mut self, intent: PurchaseIntent) -> Option<OperationKey> {
        if self.flight.is_some() || intent.requested_count == 0 { return None; }
        let minimum_revision = self.next_minimum_revision()?;
        let sequence = self.last_sequence.checked_add(1)?;
        let request_id = RequestId { scope: self.producer_scope, sequence };
        let key = OperationKey { actor: self.actor, request_id };
        self.last_sequence = sequence;
        self.flight = Some(Flight { key, intent, minimum_revision, entered: false, unknown: false,
            terminal: None, witness_revision: None });
        Some(key)
    }

    /// Single consumption at trusted host entry. This is a data transition;
    /// it does not send, own a physical gesture, or replace the transport slot.
    pub fn begin_entry(&mut self, key: OperationKey, intent: PurchaseIntent, producer_scope: OwnerScope) -> bool {
        if producer_scope != self.producer_scope || key.request_id.scope != self.producer_scope { return false; }
        let Some(minimum_revision) = self.next_minimum_revision() else { return false; };
        let Some(flight) = &mut self.flight else { return false; };
        if flight.key != key || flight.intent != intent || flight.entered { return false; }
        // A newer complete baseline between queue and entry raises the floor;
        // baseline evidence itself never becomes this flight's witness.
        flight.minimum_revision = flight.minimum_revision.max(minimum_revision);
        flight.entered = true;
        flight.witness_revision = None;
        true
    }

    pub fn cancel_definitely_unsent(&mut self, key: OperationKey, intent: PurchaseIntent) -> bool {
        if !self.flight.as_ref().is_some_and(|flight| flight.key == key && flight.intent == intent && !flight.entered) { return false; }
        self.flight = None;
        true
    }

    /// Local unknown transport feedback preserves economic custody. It cannot
    /// downgrade an already accepted terminal receipt or allocate a retry.
    pub fn mark_unknown(&mut self, key: OperationKey) -> bool {
        let Some(flight) = &mut self.flight else { return false; };
        if flight.key != key || !flight.entered { return false; }
        flight.unknown = true;
        true
    }

    pub fn recovery_query(&self) -> Option<RecoveryQuery> {
        let flight = self.flight.as_ref()?;
        flight.entered.then_some(RecoveryQuery { key: flight.key, intent: flight.intent,
            producer_scope: self.producer_scope })
    }

    pub fn observe_receipt(&mut self, receipt: PurchaseReceipt) -> Observation {
        if receipt.actor != self.actor || receipt.producer_scope != self.producer_scope { return Observation::Ignored; }
        let Some(flight) = &mut self.flight else { return Observation::Ignored; };
        if !flight.entered || receipt.request_id != flight.key.request_id || receipt.intent != flight.intent
            || !receipt.result.valid_for(flight.intent, flight.minimum_revision) { return Observation::Ignored; }
        if receipt.result == EconomicResult::Unknown {
            if flight.terminal.is_some() || flight.unknown { return Observation::Ignored; }
            flight.unknown = true;
            return Observation::Pending;
        }
        // Duplicates and contradictory later terminal observations cannot
        // replace an accepted result or affect the successor operation.
        if flight.terminal.is_some() { return Observation::Ignored; }
        flight.terminal = Some(receipt.result);
        self.highest_revision = self.highest_revision.max(receipt.result.terminal_revision().expect("terminal result"));
        self.settle_if_witnessed()
    }

    pub fn observe_snapshot(&mut self, witness: SnapshotWitness) -> Observation {
        if witness.actor != self.actor || witness.producer_scope != self.producer_scope || !witness.complete
            || witness.server_revision == u64::MAX || witness.server_revision < self.highest_revision { return Observation::Ignored; }
        self.highest_revision = witness.server_revision;
        self.applied_baseline = Some(witness.server_revision);
        let Some(flight) = &mut self.flight else { return Observation::Pending; };
        if !flight.entered || witness.server_revision < flight.minimum_revision { return Observation::Pending; }
        flight.witness_revision = Some(witness.server_revision);
        self.settle_if_witnessed()
    }

    fn settle_if_witnessed(&mut self) -> Observation {
        let Some(flight) = &self.flight else { return Observation::Ignored; };
        let Some(result) = flight.terminal else { return Observation::Pending; };
        let Some(witness_revision) = flight.witness_revision else { return Observation::Pending; };
        if witness_revision < result.terminal_revision().expect("stored terminal result") { return Observation::Pending; }
        let settlement = PurchaseSettlement { key: flight.key, intent: flight.intent, result };
        self.flight = None;
        Observation::Settled(settlement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(value: u8) -> ActorKey { ActorKey::from_server_bytes([value; 32]).unwrap() }
    fn scope(value: u8) -> OwnerScope { OwnerScope::from_server_bytes([value; 32]).unwrap() }
    fn intent(currency: PurchaseCurrency, source: PurchaseSource) -> PurchaseIntent {
        PurchaseIntent { item_index: 0, requested_count: 3, panel: 0, currency, source,
            service_catalog_proof: ServiceCatalogProof::from_server_bytes([7; 32]).unwrap() }
    }
    fn snapshot(ledger: &NpcPurchaseReceiptLedger, revision: u64) -> SnapshotWitness {
        SnapshotWitness { actor: ledger.actor(), producer_scope: ledger.producer_scope(), server_revision: revision, complete: true }
    }
    fn ready() -> NpcPurchaseReceiptLedger {
        let mut ledger = NpcPurchaseReceiptLedger::new(actor(1), scope(2));
        assert_eq!(ledger.observe_snapshot(snapshot(&ledger, 10)), Observation::Pending);
        ledger
    }
    fn enter(ledger: &mut NpcPurchaseReceiptLedger, intent: PurchaseIntent) -> OperationKey {
        let key = ledger.reserve(intent).unwrap();
        assert!(ledger.begin_entry(key, intent, ledger.producer_scope()));
        key
    }
    fn committed(ledger: &NpcPurchaseReceiptLedger, key: OperationKey, intent: PurchaseIntent, revision: u64) -> PurchaseReceipt {
        PurchaseReceipt { actor: key.actor, producer_scope: ledger.producer_scope(), request_id: key.request_id, intent,
            result: EconomicResult::Committed { server_revision: revision, currency: intent.currency,
                source: intent.source, charged: 9, admitted_count: 2, incoming_uid: 0 } }
    }
    fn settled(result: Observation, key: OperationKey) {
        let Observation::Settled(settlement) = result else { panic!("expected exact settlement: {result:?}"); };
        assert_eq!(settlement.key, key);
    }

    #[test]
    fn opaque_actor_producer_service_and_request_sequence_reject_zero() {
        assert_eq!(ActorKey::from_server_bytes([0; 32]), None);
        assert_eq!(OwnerScope::from_server_bytes([0; 32]), None);
        assert_eq!(ServiceCatalogProof::from_server_bytes([0; 32]), None);
        assert_eq!(RequestId::from_parts(scope(2), 0), None);
        let mut bytes = [0; 32]; bytes[31] = 1;
        assert_eq!(ActorKey::from_server_bytes(bytes).unwrap().bytes(), &bytes);
    }

    #[test]
    fn initial_admission_requires_complete_current_actor_baseline_and_nonzero_count() {
        let mut ledger = NpcPurchaseReceiptLedger::new(actor(1), scope(2));
        let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        assert_eq!(ledger.reserve(purchase), None);
        for bad in [SnapshotWitness { complete: false, ..snapshot(&ledger, 10) },
            SnapshotWitness { actor: actor(9), ..snapshot(&ledger, 10) },
            SnapshotWitness { producer_scope: scope(9), ..snapshot(&ledger, 10) }] {
            assert_eq!(ledger.observe_snapshot(bad), Observation::Ignored);
            assert_eq!(ledger.reserve(purchase), None);
        }
        ledger.observe_snapshot(snapshot(&ledger, 10));
        assert_eq!(ledger.reserve(PurchaseIntent { requested_count: 0, ..purchase }), None);
        assert_eq!(ledger.reserve(purchase).unwrap().request_id.sequence(), 1);
    }

    #[test]
    fn complete_initial_revision_zero_admits_only_positive_terminal_revision() {
        let mut ledger = NpcPurchaseReceiptLedger::new(actor(1), scope(2));
        assert_eq!(ledger.observe_snapshot(snapshot(&ledger, 0)), Observation::Pending);
        let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = enter(&mut ledger, purchase);
        assert_eq!(ledger.pending().unwrap().minimum_revision, 1);
        assert_eq!(ledger.observe_receipt(committed(&ledger, key, purchase, 0)), Observation::Ignored);
        assert_eq!(ledger.observe_receipt(committed(&ledger, key, purchase, 1)), Observation::Pending);
        settled(ledger.observe_snapshot(snapshot(&ledger, 1)), key);
    }

    #[test]
    fn supported_currency_source_combinations_settle_in_both_arrival_orders() {
        for (currency, source) in [(PurchaseCurrency::Gold, PurchaseSource::Trade),
            (PurchaseCurrency::Pearls, PurchaseSource::Trade), (PurchaseCurrency::Gold, PurchaseSource::BuyBack),
            (PurchaseCurrency::Gold, PurchaseSource::Used),
            (PurchaseCurrency::Pearls, PurchaseSource::Used)] {
            for snapshot_first in [false, true] {
                let mut ledger = ready(); let purchase = intent(currency, source); let key = enter(&mut ledger, purchase);
                let receipt = committed(&ledger, key, purchase, 11); let witness = snapshot(&ledger, 11);
                let last = if snapshot_first {
                    assert_eq!(ledger.observe_snapshot(witness), Observation::Pending);
                    ledger.observe_receipt(receipt)
                } else {
                    assert_eq!(ledger.observe_receipt(receipt), Observation::Pending);
                    ledger.observe_snapshot(witness)
                };
                settled(last, key); assert_eq!(ledger.pending(), None);
                assert_eq!(ledger.observe_receipt(receipt), Observation::Ignored);
            }
        }
    }

    #[test]
    fn rejected_journal_revision_requires_complete_witness_in_both_orders() {
        for snapshot_first in [false, true] {
            let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Used);
            let key = enter(&mut ledger, purchase); let mut receipt = committed(&ledger, key, purchase, 11);
            receipt.result = EconomicResult::Rejected { server_revision: 11 };
            let witness = snapshot(&ledger, 11);
            let result = if snapshot_first { ledger.observe_snapshot(witness); ledger.observe_receipt(receipt) }
                else { ledger.observe_receipt(receipt); ledger.observe_snapshot(witness) };
            settled(result, key);
        }
    }

    #[test]
    fn partial_foreign_actor_old_producer_and_older_revision_never_unlock() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Pearls, PurchaseSource::Used);
        let key = enter(&mut ledger, purchase); let receipt = committed(&ledger, key, purchase, 12);
        assert_eq!(ledger.observe_receipt(receipt), Observation::Pending);
        for bad in [SnapshotWitness { complete: false, ..snapshot(&ledger, 12) },
            SnapshotWitness { actor: actor(9), ..snapshot(&ledger, 12) },
            SnapshotWitness { producer_scope: scope(9), ..snapshot(&ledger, 12) }, snapshot(&ledger, 11)] {
            assert_eq!(ledger.observe_snapshot(bad), Observation::Ignored);
            assert_eq!(ledger.pending().unwrap().key, key);
        }
        settled(ledger.observe_snapshot(snapshot(&ledger, 12)), key);
    }

    #[test]
    fn same_uid_zero_fully_merged_delivery_requires_no_inventory_membership() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::BuyBack);
        let key = enter(&mut ledger, purchase); let mut receipt = committed(&ledger, key, purchase, 11);
        receipt.result = EconomicResult::Committed { server_revision: 11, currency: purchase.currency,
            source: purchase.source, charged: 0, admitted_count: purchase.requested_count, incoming_uid: 0 };
        assert_eq!(receipt.intent.item_index, 0);
        assert!(matches!(receipt.result, EconomicResult::Committed { incoming_uid: 0, .. }));
        ledger.observe_snapshot(snapshot(&ledger, 11)); settled(ledger.observe_receipt(receipt), key);
        // The API has no inventory argument: a merged incoming UID may be absent.
    }

    #[test]
    fn reconnect_recovery_keeps_original_key_tuple_and_allocator_without_retry() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Pearls, PurchaseSource::Trade);
        let key = enter(&mut ledger, purchase); ledger.mark_unknown(key);
        let old_receipt = committed(&ledger, key, purchase, 11);
        assert!(ledger.rebind(actor(1), scope(3)));
        assert_eq!(ledger.applied_baseline(), None);
        assert_eq!(ledger.pending().unwrap().phase, PurchasePhase::Unknown);
        for _ in 0..3 {
            assert_eq!(ledger.recovery_query(), Some(RecoveryQuery { key, intent: purchase, producer_scope: scope(3) }));
            assert_eq!(ledger.reserve(purchase), None); assert_eq!(ledger.last_sequence(), 1);
        }
        assert_eq!(ledger.observe_receipt(old_receipt), Observation::Ignored);
        let recovered = PurchaseReceipt { producer_scope: scope(3), ..old_receipt };
        ledger.observe_receipt(recovered); settled(ledger.observe_snapshot(snapshot(&ledger, 11)), key);
        let successor = ledger.reserve(purchase).unwrap();
        assert_eq!(successor.request_id.sequence(), 2); assert_eq!(successor.request_id.scope(), scope(3));
    }

    #[test]
    fn foreign_actor_rebind_and_low_revision_do_not_take_over_original_flight() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = enter(&mut ledger, purchase);
        assert!(!ledger.rebind(actor(9), scope(3))); assert_eq!(ledger.producer_scope(), scope(2));
        let receipt = committed(&ledger, key, purchase, 11);
        assert_eq!(ledger.observe_receipt(PurchaseReceipt { actor: actor(9), ..receipt }), Observation::Ignored);
        assert_eq!(ledger.observe_snapshot(SnapshotWitness { actor: actor(9), ..snapshot(&ledger, 11) }), Observation::Ignored);
        assert_eq!(ledger.pending().unwrap().key, key);
        assert!(ledger.rebind(actor(1), scope(3)));
        assert_eq!(ledger.observe_snapshot(snapshot(&ledger, 9)), Observation::Ignored);
        assert_eq!(ledger.applied_baseline(), None);
    }

    #[test]
    fn retired_producer_aba_cannot_restore_old_snapshot_or_receipt_evidence() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = enter(&mut ledger, purchase); let old = committed(&ledger, key, purchase, 11);
        ledger.observe_receipt(old);
        assert!(ledger.rebind(actor(1), scope(3))); assert!(!ledger.rebind(actor(1), scope(2)));
        assert_eq!(ledger.observe_snapshot(SnapshotWitness { producer_scope: scope(2), ..snapshot(&ledger, 11) }), Observation::Ignored);
        assert_eq!(ledger.observe_receipt(old), Observation::Ignored);
        assert_eq!(ledger.pending().unwrap().phase, PurchasePhase::Unknown);
        assert_eq!(ledger.observe_snapshot(snapshot(&ledger, 11)), Observation::Pending);
        assert!(ledger.pending().is_some());
        settled(ledger.observe_receipt(PurchaseReceipt { producer_scope: scope(3), ..old }), key);
    }

    #[test]
    fn producer_rebind_clears_previously_applied_flight_witness() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Pearls, PurchaseSource::BuyBack);
        let key = enter(&mut ledger, purchase);
        let old = committed(&ledger, key, purchase, 11);
        assert_eq!(ledger.observe_snapshot(snapshot(&ledger, 11)), Observation::Pending);
        assert!(ledger.rebind(actor(1), scope(3)));
        assert_eq!(ledger.observe_receipt(PurchaseReceipt { producer_scope: scope(3), ..old }), Observation::Pending);
        assert_eq!(ledger.pending().unwrap().phase, PurchasePhase::AwaitingSnapshot);
        assert_eq!(ledger.observe_snapshot(SnapshotWitness { producer_scope: scope(2), ..snapshot(&ledger, 11) }), Observation::Ignored);
        assert!(ledger.pending().is_some());
        settled(ledger.observe_snapshot(snapshot(&ledger, 11)), key);
    }

    #[test]
    fn unknown_barrier_blocks_every_currency_and_source_substitution() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = enter(&mut ledger, purchase); let mut receipt = committed(&ledger, key, purchase, 11);
        receipt.result = EconomicResult::Unknown;
        assert_eq!(ledger.observe_receipt(receipt), Observation::Pending);
        for currency in [PurchaseCurrency::Gold, PurchaseCurrency::Pearls] {
            for source in [PurchaseSource::Trade, PurchaseSource::BuyBack, PurchaseSource::Used] {
                assert_eq!(ledger.reserve(intent(currency, source)), None);
            }
        }
        ledger.observe_snapshot(snapshot(&ledger, 99)); assert_eq!(ledger.pending().unwrap().key, key);
        assert!(!ledger.cancel_definitely_unsent(key, purchase)); assert_eq!(ledger.last_sequence(), 1);
    }

    #[test]
    fn full_tuple_currency_source_revision_and_admission_mismatches_reject_receipts() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = enter(&mut ledger, purchase); let good = committed(&ledger, key, purchase, 11);
        for altered in [PurchaseIntent { item_index: 1, ..purchase }, PurchaseIntent { requested_count: 2, ..purchase },
            PurchaseIntent { panel: 1, ..purchase }, PurchaseIntent { currency: PurchaseCurrency::Pearls, ..purchase },
            PurchaseIntent { source: PurchaseSource::Used, ..purchase }, PurchaseIntent {
                service_catalog_proof: ServiceCatalogProof::from_server_bytes([8; 32]).unwrap(), ..purchase }] {
            assert_eq!(ledger.observe_receipt(PurchaseReceipt { intent: altered, ..good }), Observation::Ignored);
        }
        for result in [EconomicResult::Rejected { server_revision: 0 }, EconomicResult::Rejected { server_revision: 10 },
            EconomicResult::Committed { server_revision: 11, currency: PurchaseCurrency::Pearls, source: purchase.source, charged: 0, admitted_count: 1, incoming_uid: 0 },
            EconomicResult::Committed { server_revision: 11, currency: purchase.currency, source: PurchaseSource::Used, charged: 0, admitted_count: 1, incoming_uid: 0 },
            EconomicResult::Committed { server_revision: 11, currency: purchase.currency, source: purchase.source, charged: 0, admitted_count: 0, incoming_uid: 0 },
            EconomicResult::Committed { server_revision: 11, currency: purchase.currency, source: purchase.source, charged: 0, admitted_count: 4, incoming_uid: 0 }] {
            assert_eq!(ledger.observe_receipt(PurchaseReceipt { result, ..good }), Observation::Ignored);
        }
        assert_eq!(ledger.pending().unwrap().phase, PurchasePhase::Entered);
        assert_eq!(ledger.observe_receipt(good), Observation::Pending);
        settled(ledger.observe_snapshot(snapshot(&ledger, 11)), key);
        assert_eq!(ledger.pending(), None);
    }

    #[test]
    fn exact_entry_is_one_use_and_receipts_require_entered_original_request_and_producer() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = ledger.reserve(purchase).unwrap();
        let good = committed(&ledger, key, purchase, 11);
        assert_eq!(ledger.observe_receipt(good), Observation::Ignored);
        assert_eq!(ledger.reserve(purchase), None);
        assert!(!ledger.begin_entry(key, PurchaseIntent { panel: 1, ..purchase }, scope(2)));
        assert!(!ledger.begin_entry(key, purchase, scope(9)));
        assert_eq!(ledger.pending().unwrap().phase, PurchasePhase::Queued);
        assert!(ledger.begin_entry(key, purchase, scope(2)));
        assert!(!ledger.begin_entry(key, purchase, scope(2)));
        for bad in [PurchaseReceipt { request_id: RequestId::from_parts(scope(2), 2).unwrap(), ..good },
            PurchaseReceipt { request_id: RequestId::from_parts(scope(9), 1).unwrap(), ..good },
            PurchaseReceipt { producer_scope: scope(9), ..good }] {
            assert_eq!(ledger.observe_receipt(bad), Observation::Ignored);
            assert_eq!(ledger.pending().unwrap().key, key);
        }
        assert_eq!(ledger.last_sequence(), 1);
        ledger.observe_receipt(good); settled(ledger.observe_snapshot(snapshot(&ledger, 11)), key);
    }

    #[test]
    fn complete_baseline_before_entry_is_not_reusable_witness_and_raises_floor() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = ledger.reserve(purchase).unwrap(); ledger.observe_snapshot(snapshot(&ledger, 20));
        assert!(ledger.begin_entry(key, purchase, scope(2)));
        assert_eq!(ledger.pending().unwrap().minimum_revision, 21);
        assert_eq!(ledger.observe_receipt(committed(&ledger, key, purchase, 20)), Observation::Ignored);
        assert_eq!(ledger.observe_receipt(committed(&ledger, key, purchase, 21)), Observation::Pending);
        assert!(ledger.pending().is_some()); settled(ledger.observe_snapshot(snapshot(&ledger, 21)), key);
    }

    #[test]
    fn duplicate_mature_results_and_old_snapshots_never_settle_successor() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let old_key = enter(&mut ledger, purchase); let old = committed(&ledger, old_key, purchase, 11);
        ledger.observe_receipt(old); settled(ledger.observe_snapshot(snapshot(&ledger, 11)), old_key);
        let key = enter(&mut ledger, purchase);
        assert_eq!(key.request_id.sequence(), 2); assert_eq!(ledger.pending().unwrap().minimum_revision, 12);
        assert_eq!(ledger.observe_receipt(old), Observation::Ignored);
        assert_eq!(ledger.observe_snapshot(snapshot(&ledger, 11)), Observation::Pending);
        let receipt = committed(&ledger, key, purchase, 12);
        assert_eq!(ledger.observe_receipt(receipt), Observation::Pending);
        assert_eq!(ledger.observe_receipt(receipt), Observation::Ignored);
        settled(ledger.observe_snapshot(snapshot(&ledger, 12)), key);
        assert_eq!(ledger.observe_receipt(receipt), Observation::Ignored);
    }

    #[test]
    fn authoritative_terminal_is_not_downgraded_by_unknown_or_conflicting_result() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Pearls, PurchaseSource::Used);
        let key = enter(&mut ledger, purchase); let receipt = committed(&ledger, key, purchase, 11);
        ledger.observe_receipt(receipt);
        for result in [EconomicResult::Unknown, EconomicResult::Rejected { server_revision: 12 }] {
            assert_eq!(ledger.observe_receipt(PurchaseReceipt { result, ..receipt }), Observation::Ignored);
        }
        assert!(ledger.mark_unknown(key)); assert_eq!(ledger.pending().unwrap().phase, PurchasePhase::AwaitingSnapshot);
        let Observation::Settled(value) = ledger.observe_snapshot(snapshot(&ledger, 11)) else { panic!("expected commit"); };
        assert_eq!(value.result, receipt.result);
    }

    #[test]
    fn definitely_unsent_cancellation_is_exact_preentry_only_and_ids_stay_monotonic() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = ledger.reserve(purchase).unwrap();
        assert!(!ledger.cancel_definitely_unsent(key, PurchaseIntent { panel: 1, ..purchase }));
        assert!(ledger.cancel_definitely_unsent(key, purchase)); assert!(!ledger.cancel_definitely_unsent(key, purchase));
        assert!(ledger.rebind(actor(1), scope(3))); assert_eq!(ledger.reserve(purchase), None);
        ledger.observe_snapshot(snapshot(&ledger, 10)); let next = enter(&mut ledger, purchase);
        assert_eq!(next.request_id.sequence(), 2); assert!(!ledger.begin_entry(next, purchase, scope(3)));
        assert!(!ledger.cancel_definitely_unsent(next, purchase)); assert!(!ledger.cancel_definitely_unsent(key, purchase));
    }

    #[test]
    fn queued_old_producer_cannot_enter_after_rebind_and_can_only_cancel_definitely_unsent() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::BuyBack);
        let key = ledger.reserve(purchase).unwrap(); assert!(ledger.rebind(actor(1), scope(3)));
        ledger.observe_snapshot(snapshot(&ledger, 10));
        assert!(!ledger.begin_entry(key, purchase, scope(2))); assert!(!ledger.begin_entry(key, purchase, scope(3)));
        assert_eq!(ledger.recovery_query(), None); assert!(ledger.cancel_definitely_unsent(key, purchase));
        assert_eq!(ledger.reserve(purchase).unwrap().request_id.sequence(), 2);
    }

    #[test]
    fn same_producer_authority_withdrawal_keeps_original_operation_until_fresh_evidence() {
        let mut ledger = ready();
        let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Trade);
        let key = enter(&mut ledger, purchase);
        let receipt = committed(&ledger, key, purchase, 11);
        assert_eq!(ledger.observe_receipt(receipt), Observation::Pending);
        ledger.withdraw_authority();
        assert_eq!(ledger.applied_baseline(), None);
        assert_eq!(ledger.pending().unwrap().key, key);
        assert_eq!(ledger.pending().unwrap().phase, PurchasePhase::Unknown);
        assert_eq!(ledger.last_sequence(), 1);
        assert_eq!(ledger.reserve(purchase), None);
        assert!(ledger.rebind(actor(1), scope(2)));
        assert_eq!(ledger.recovery_query().unwrap().key, key);
        assert_eq!(ledger.observe_snapshot(snapshot(&ledger, 11)), Observation::Pending);
        assert!(ledger.pending().is_some());
        settled(ledger.observe_receipt(receipt), key);
        assert_eq!(ledger.reserve(purchase).unwrap().request_id.sequence(), 2);
    }

    #[test]
    fn checked_sequence_exhaustion_keeps_entered_recovery_and_never_wraps() {
        let mut ledger = ready(); ledger.last_sequence = u64::MAX - 1;
        let purchase = intent(PurchaseCurrency::Pearls, PurchaseSource::Trade); let key = enter(&mut ledger, purchase);
        assert_eq!(key.request_id.sequence(), u64::MAX);
        ledger.mark_unknown(key); assert_eq!(ledger.reserve(purchase), None);
        assert!(ledger.rebind(actor(1), scope(3))); assert_eq!(ledger.recovery_query().unwrap().key, key);
        let receipt = committed(&ledger, key, purchase, 11); ledger.observe_receipt(receipt);
        settled(ledger.observe_snapshot(snapshot(&ledger, 11)), key);
        assert_eq!(ledger.reserve(purchase), None); assert_eq!(ledger.last_sequence(), u64::MAX);
    }

    #[test]
    fn checked_revision_exhaustion_never_fabricates_a_next_baseline_or_clears_flight() {
        let mut ledger = ready(); let purchase = intent(PurchaseCurrency::Gold, PurchaseSource::Used);
        assert_eq!(ledger.observe_snapshot(snapshot(&ledger, u64::MAX)), Observation::Ignored);
        assert_eq!(ledger.applied_baseline(), Some(10));
        ledger.observe_snapshot(snapshot(&ledger, u64::MAX - 1));
        assert_eq!(ledger.reserve(purchase), None);
        assert_eq!(ledger.last_sequence(), 0);
        let mut active = ready(); let key = enter(&mut active, purchase);
        assert_eq!(active.observe_snapshot(snapshot(&active, u64::MAX)), Observation::Ignored);
        assert_eq!(active.applied_baseline(), Some(10));
        for bad in [committed(&active, key, purchase, u64::MAX),
            PurchaseReceipt { result: EconomicResult::Rejected { server_revision: u64::MAX },
                ..committed(&active, key, purchase, 11) }] {
            assert_eq!(active.observe_receipt(bad), Observation::Ignored);
            assert_eq!(active.pending().unwrap().key, key);
        }
        active.mark_unknown(key);
        assert_eq!(active.recovery_query().unwrap().key, key);
        active.observe_receipt(committed(&active, key, purchase, 11));
        settled(active.observe_snapshot(snapshot(&active, 11)), key);
        assert!(active.reserve(purchase).is_some());
    }
}

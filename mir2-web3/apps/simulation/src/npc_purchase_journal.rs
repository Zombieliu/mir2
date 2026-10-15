//! Strict, actor-bound records of terminal NPC purchase processing.
//!
//! This module does not execute a purchase, authenticate a client, generate an
//! actor, persist a checkpoint, or make an in-memory outcome durable. The host
//! must authenticate the exact account/character and stable, server-issued
//! actor before using a journal. Nonzero opaque bytes are validation, not proof
//! of authentication. Request scopes are producer incarnations, not actors.
//!
//! Call can_append before economic execution. Never evict an old operation to
//! make room: the host must refuse new purchases when this journal is full.
//! Persist entries together with a complete authoritative economic checkpoint;
//! its actor revision must include every retained result through that revision.
//! Protected save/merge and atomic persistence are host responsibilities.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::runtime::{
    NpcPurchaseCurrency, NpcPurchaseProcessingOutcome, NpcPurchaseRequest, NpcPurchaseSource,
};

pub const NPC_PURCHASE_JOURNAL_PROTOCOL_VERSION: u8 = 1;
pub const NPC_PURCHASE_JOURNAL_ENTRY_CAP: usize = 4096;

// Source: runtime/crystal_compat.rs CRYSTAL_PANEL_BUY. That implementation
// constant is private to runtime; this journal checks its actual value, zero.
const PURCHASE_PANEL: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NpcPurchaseIntent {
    pub request: NpcPurchaseRequest,
    pub currency: NpcPurchaseCurrency,
    pub source: NpcPurchaseSource,
    pub service_catalog_proof: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NpcPurchaseOperation {
    pub actor: [u8; 32],
    pub request_scope: [u8; 32],
    pub sequence: u64,
    pub intent: NpcPurchaseIntent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NpcPurchaseJournalEntry {
    pub operation: NpcPurchaseOperation,
    pub server_revision: u64,
    pub outcome: NpcPurchaseProcessingOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NpcPurchaseJournal {
    pub protocol_version: u8,
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub actor: [u8; 32],
    pub entries: Vec<NpcPurchaseJournalEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcPurchaseJournalError {
    InvalidIdentity,
    BindingMismatch,
    UnsupportedVersion,
    InvalidOperation,
    ForeignActor,
    DuplicateKey,
    IntentConflict,
    InvalidRevision,
    InvalidOutcome,
    CapacityReached,
    DuplicateRecord,
    RecordConflict,
}

impl fmt::Display for NpcPurchaseJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidIdentity => "NPC purchase journal requires a canonical account and valid character identity",
            Self::BindingMismatch => "NPC purchase journal account/character binding does not match",
            Self::UnsupportedVersion => "unsupported NPC purchase journal protocol version",
            Self::InvalidOperation => "invalid NPC purchase operation identity or request tuple",
            Self::ForeignActor => "NPC purchase operation belongs to another actor",
            Self::DuplicateKey => "NPC purchase journal contains a duplicate request scope/sequence",
            Self::IntentConflict => "NPC purchase request key was reused with a different intent",
            Self::InvalidRevision => "NPC purchase revision is invalid, unordered or beyond the complete checkpoint",
            Self::InvalidOutcome => "NPC purchase terminal outcome does not match the exact request intent",
            Self::CapacityReached => "NPC purchase journal capacity reached; old operations cannot be evicted",
            Self::DuplicateRecord => "NPC purchase terminal result is already recorded",
            Self::RecordConflict => "NPC purchase request key conflicts with its recorded terminal result",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for NpcPurchaseJournalError {}

type JournalResult<T> = Result<T, NpcPurchaseJournalError>;

fn nonzero(bytes: &[u8; 32]) -> bool {
    bytes.iter().any(|byte| *byte != 0)
}

fn validate_identity(account: &str, index: i32, name: &str, actor: &[u8; 32]) -> JournalResult<()> {
    if account.is_empty() || account != account.trim() || index < 0
        || account.chars().any(char::is_control) || name.trim().is_empty()
        || name.chars().any(char::is_control) || !nonzero(actor)
    {
        return Err(NpcPurchaseJournalError::InvalidIdentity);
    }
    Ok(())
}

fn validate_operation(operation: &NpcPurchaseOperation, actor: &[u8; 32]) -> JournalResult<()> {
    if !nonzero(&operation.actor) || !nonzero(&operation.request_scope)
        || !nonzero(&operation.intent.service_catalog_proof) || operation.sequence == 0
        || operation.intent.request.count == 0 || operation.intent.request.panel_type != PURCHASE_PANEL
    {
        return Err(NpcPurchaseJournalError::InvalidOperation);
    }
    if operation.actor != *actor {
        return Err(NpcPurchaseJournalError::ForeignActor);
    }
    // item_index and incoming_unique_id are raw u64 identities. Zero and the
    // full u64 range are valid; neither is a template ID or inventory lookup.
    Ok(())
}

fn validate_entry(entry: &NpcPurchaseJournalEntry, actor: &[u8; 32]) -> JournalResult<()> {
    validate_operation(&entry.operation, actor)?;
    if entry.server_revision == 0 || entry.server_revision == u64::MAX {
        return Err(NpcPurchaseJournalError::InvalidRevision);
    }
    let intent = entry.operation.intent;
    match &entry.outcome {
        NpcPurchaseProcessingOutcome::Committed {
            request, currency, source, admitted_count, ..
        } => {
            if *request != intent.request || *currency != intent.currency || *source != intent.source
                || *admitted_count == 0 || *admitted_count > intent.request.count
            {
                return Err(NpcPurchaseJournalError::InvalidOutcome);
            }
        }
        NpcPurchaseProcessingOutcome::Rejected { request, .. } => {
            if *request != intent.request {
                return Err(NpcPurchaseJournalError::InvalidOutcome);
            }
        }
    }
    // ProcessingOutcome deliberately has no Unknown case. An uncertain result
    // remains unresolved host custody and must never be forged into this journal.
    Ok(())
}

impl NpcPurchaseJournalEntry {
    /// Validate the terminal domain result without constructing or exporting a journal.
    pub fn validate_terminal(&self) -> Result<(), NpcPurchaseJournalError> {
        validate_entry(self, &self.operation.actor)
    }
}

impl NpcPurchaseJournal {
    pub fn new(
        account_id: impl Into<String>,
        character_index: i32,
        character_name: impl Into<String>,
        actor: [u8; 32],
    ) -> JournalResult<Self> {
        let account_id = account_id.into();
        let character_name = character_name.into();
        validate_identity(&account_id, character_index, &character_name, &actor)?;
        Ok(Self {
            protocol_version: NPC_PURCHASE_JOURNAL_PROTOCOL_VERSION,
            account_id,
            character_index,
            character_name,
            actor,
            entries: Vec::new(),
        })
    }

    /// Validate an exact authenticated binding and the complete checkpoint
    /// that contains this journal. This does not authenticate the supplied host.
    pub fn validate_for(
        &self,
        account: &str,
        index: i32,
        name: &str,
        complete_checkpoint_revision: u64,
    ) -> JournalResult<()> {
        validate_identity(account, index, name, &self.actor)?;
        if complete_checkpoint_revision == u64::MAX {
            return Err(NpcPurchaseJournalError::InvalidRevision);
        }
        if self.account_id != account || self.character_index != index || self.character_name != name {
            return Err(NpcPurchaseJournalError::BindingMismatch);
        }
        self.validate_integrity(complete_checkpoint_revision)
    }

    fn validate_integrity(&self, complete_checkpoint_revision: u64) -> JournalResult<()> {
        validate_identity(&self.account_id, self.character_index, &self.character_name, &self.actor)?;
        if self.protocol_version != NPC_PURCHASE_JOURNAL_PROTOCOL_VERSION {
            return Err(NpcPurchaseJournalError::UnsupportedVersion);
        }
        if self.entries.len() > NPC_PURCHASE_JOURNAL_ENTRY_CAP {
            return Err(NpcPurchaseJournalError::CapacityReached);
        }
        let mut keys = BTreeSet::new();
        let mut previous_revision = 0;
        for entry in &self.entries {
            validate_entry(entry, &self.actor)?;
            if !keys.insert((entry.operation.request_scope, entry.operation.sequence)) {
                return Err(NpcPurchaseJournalError::DuplicateKey);
            }
            if entry.server_revision <= previous_revision || entry.server_revision > complete_checkpoint_revision {
                return Err(NpcPurchaseJournalError::InvalidRevision);
            }
            previous_revision = entry.server_revision;
        }
        Ok(())
    }

    /// Look up the original operation only. A matching request key with a
    /// changed full tuple is an error, never a cache miss admitting a retry.
    pub fn lookup(&self, operation: &NpcPurchaseOperation) -> JournalResult<Option<&NpcPurchaseJournalEntry>> {
        self.validate_integrity(u64::MAX - 1)?;
        validate_operation(operation, &self.actor)?;
        let Some(entry) = self.entries.iter().find(|entry| {
            entry.operation.request_scope == operation.request_scope && entry.operation.sequence == operation.sequence
        }) else {
            return Ok(None);
        };
        if entry.operation != *operation {
            return Err(NpcPurchaseJournalError::IntentConflict);
        }
        Ok(Some(entry))
    }

    /// Capacity reservation is a host precondition before executing a purchase;
    /// this read-only check also refuses an exhausted retained revision. The
    /// host must separately check the complete actor checkpoint's revision.
    /// It does not execute, authorize or persist anything.
    pub fn can_append(&self) -> JournalResult<()> {
        self.validate_integrity(u64::MAX - 1)?;
        if self.entries.len() == NPC_PURCHASE_JOURNAL_ENTRY_CAP {
            return Err(NpcPurchaseJournalError::CapacityReached);
        }
        if self.entries.last().is_some_and(|last| last.server_revision == u64::MAX - 1) {
            return Err(NpcPurchaseJournalError::InvalidRevision);
        }
        Ok(())
    }

    /// All validation precedes push. Exact duplicates and conflicts both leave
    /// the journal untouched. The host must atomically save the accepted record
    /// and matching complete economic state before issuing a durable receipt.
    pub fn record(
        &mut self,
        operation: NpcPurchaseOperation,
        server_revision: u64,
        outcome: NpcPurchaseProcessingOutcome,
    ) -> JournalResult<()> {
        self.validate_integrity(u64::MAX - 1)?;
        let entry = NpcPurchaseJournalEntry { operation, server_revision, outcome };
        validate_entry(&entry, &self.actor)?;
        if let Some(old) = self.entries.iter().find(|old| {
            old.operation.request_scope == operation.request_scope && old.operation.sequence == operation.sequence
        }) {
            return Err(if *old == entry {
                NpcPurchaseJournalError::DuplicateRecord
            } else {
                NpcPurchaseJournalError::RecordConflict
            });
        }
        self.can_append()?;
        if self.entries.last().is_some_and(|last| server_revision <= last.server_revision) {
            return Err(NpcPurchaseJournalError::InvalidRevision);
        }
        self.entries.push(entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::NpcPurchaseRejection;

    fn journal() -> NpcPurchaseJournal {
        NpcPurchaseJournal::new("account", 0, "ActualPlayer", [1; 32]).unwrap()
    }

    fn operation(sequence: u64, currency: NpcPurchaseCurrency, source: NpcPurchaseSource) -> NpcPurchaseOperation {
        NpcPurchaseOperation {
            actor: [1; 32], request_scope: [2; 32], sequence,
            intent: NpcPurchaseIntent {
                request: NpcPurchaseRequest { item_index: 0, count: 3, panel_type: 0 },
                currency, source, service_catalog_proof: [3; 32],
            },
        }
    }

    fn gold(sequence: u64) -> NpcPurchaseOperation {
        operation(sequence, NpcPurchaseCurrency::Gold, NpcPurchaseSource::Trade)
    }

    fn committed(operation: NpcPurchaseOperation) -> NpcPurchaseProcessingOutcome {
        NpcPurchaseProcessingOutcome::Committed {
            request: operation.intent.request, currency: operation.intent.currency,
            source: operation.intent.source, charged: 0, admitted_count: 3, incoming_unique_id: 0,
        }
    }

    fn rejected(operation: NpcPurchaseOperation) -> NpcPurchaseProcessingOutcome {
        NpcPurchaseProcessingOutcome::Rejected {
            request: operation.intent.request, reason: NpcPurchaseRejection::InsufficientCurrency,
        }
    }

    fn entry(operation: NpcPurchaseOperation, revision: u64) -> NpcPurchaseJournalEntry {
        NpcPurchaseJournalEntry { operation, server_revision: revision, outcome: committed(operation) }
    }

    #[test]
    fn canonical_account_exact_character_and_nonzero_actor_are_required() {
        for account in ["", " ", " account", "account ", "acc\0ount", "acc\nount"] {
            assert!(NpcPurchaseJournal::new(account, 0, "ActualPlayer", [1; 32]).is_err());
        }
        assert!(NpcPurchaseJournal::new("account", -1, "ActualPlayer", [1; 32]).is_err());
        assert!(NpcPurchaseJournal::new("account", 0, " ", [1; 32]).is_err());
        for name in ["Actual\0Player", "Actual\nPlayer", "Actual\u{7f}Player"] {
            assert!(NpcPurchaseJournal::new("account", 0, name, [1; 32]).is_err());
        }
        assert!(NpcPurchaseJournal::new("account", 0, "ActualPlayer", [0; 32]).is_err());
        let journal = journal();
        for (account, index, name) in [("Account", 0, "ActualPlayer"), ("other", 0, "ActualPlayer"),
            ("account", 1, "ActualPlayer"), ("account", 0, "actualplayer")] {
            assert_eq!(journal.validate_for(account, index, name, 0), Err(NpcPurchaseJournalError::BindingMismatch));
        }
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", 0), Ok(()));
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", u64::MAX), Err(NpcPurchaseJournalError::InvalidRevision));
    }

    #[test]
    fn all_five_supported_combinations_preserve_actual_terminal_results() {
        let mut journal = journal();
        for (index, (currency, source)) in [(NpcPurchaseCurrency::Gold, NpcPurchaseSource::Trade),
            (NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Trade), (NpcPurchaseCurrency::Gold, NpcPurchaseSource::BuyBack),
            (NpcPurchaseCurrency::Gold, NpcPurchaseSource::Used), (NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Used)]
            .into_iter().enumerate()
        {
            let sequence = index as u64 + 1;
            let operation = operation(sequence, currency, source);
            let outcome = committed(operation);
            assert_eq!(journal.record(operation, sequence, outcome.clone()), Ok(()));
            let recorded = journal.lookup(&operation).unwrap().unwrap();
            assert_eq!(recorded.outcome, outcome);
            assert_eq!(recorded.operation.intent.request.item_index, 0);
        }
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", 5), Ok(()));
    }

    #[test]
    fn whole_merge_zero_uid_and_free_charge_need_no_inventory_presence() {
        let mut journal = journal(); let operation = gold(1);
        let outcome = committed(operation);
        assert_eq!(journal.record(operation, 1, outcome.clone()), Ok(()));
        assert_eq!(journal.lookup(&operation).unwrap().unwrap().outcome, outcome);
        assert!(matches!(outcome, NpcPurchaseProcessingOutcome::Committed {
            charged: 0, admitted_count: 3, incoming_unique_id: 0, ..
        }));
        // There is no inventory input: an incoming delta UID may be fully merged.
    }

    #[test]
    fn terminal_rejection_is_retained_and_checkpoint_revision_is_required() {
        let mut journal = journal(); let operation = gold(1);
        let outcome = rejected(operation);
        assert_eq!(journal.record(operation, 9, outcome.clone()), Ok(()));
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", 8), Err(NpcPurchaseJournalError::InvalidRevision));
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", 9), Ok(()));
        assert_eq!(journal.lookup(&operation).unwrap().unwrap().outcome, outcome);
    }

    #[test]
    fn lookup_conflicts_for_every_changed_intent_and_never_returns_retry_miss() {
        let mut journal = journal(); let original = gold(1);
        journal.record(original, 1, committed(original)).unwrap();
        for intent in [NpcPurchaseIntent { request: NpcPurchaseRequest { item_index: 1, ..original.intent.request }, ..original.intent },
            NpcPurchaseIntent { request: NpcPurchaseRequest { count: 2, ..original.intent.request }, ..original.intent },
            NpcPurchaseIntent { currency: NpcPurchaseCurrency::Pearls, ..original.intent },
            NpcPurchaseIntent { source: NpcPurchaseSource::Used, ..original.intent },
            NpcPurchaseIntent { service_catalog_proof: [4; 32], ..original.intent }] {
            assert_eq!(journal.lookup(&NpcPurchaseOperation { intent, ..original }), Err(NpcPurchaseJournalError::IntentConflict));
        }
        assert_eq!(journal.lookup(&NpcPurchaseOperation { actor: [9; 32], ..original }), Err(NpcPurchaseJournalError::ForeignActor));
        assert_eq!(journal.lookup(&gold(2)), Ok(None));
        assert_eq!(journal.lookup(&NpcPurchaseOperation { request_scope: [4; 32], ..original }), Ok(None));
        assert_eq!(journal.entries.len(), 1);
    }

    #[test]
    fn exact_duplicate_and_changed_tuple_outcome_or_revision_never_append() {
        let mut journal = journal(); let original = gold(1); let result = committed(original);
        journal.record(original, 1, result.clone()).unwrap(); let before = journal.clone();
        assert_eq!(journal.record(original, 1, result.clone()), Err(NpcPurchaseJournalError::DuplicateRecord));
        assert_eq!(journal.record(original, 2, result), Err(NpcPurchaseJournalError::RecordConflict));
        assert_eq!(journal.record(original, 1, rejected(original)), Err(NpcPurchaseJournalError::RecordConflict));
        let changed = NpcPurchaseOperation { intent: NpcPurchaseIntent { service_catalog_proof: [4; 32], ..original.intent }, ..original };
        assert_eq!(journal.record(changed, 2, committed(changed)), Err(NpcPurchaseJournalError::RecordConflict));
        assert_eq!(journal, before);
    }

    #[test]
    fn malformed_outcomes_leave_room_for_the_following_actual_result() {
        let mut journal = journal(); let operation = gold(1); let request = operation.intent.request;
        let bad_request = NpcPurchaseRequest { item_index: 1, ..request };
        for outcome in [NpcPurchaseProcessingOutcome::Committed { request: bad_request, currency: operation.intent.currency,
                source: operation.intent.source, charged: 0, admitted_count: 1, incoming_unique_id: 0 },
            NpcPurchaseProcessingOutcome::Committed { request, currency: NpcPurchaseCurrency::Pearls,
                source: operation.intent.source, charged: 0, admitted_count: 1, incoming_unique_id: 0 },
            NpcPurchaseProcessingOutcome::Committed { request, currency: operation.intent.currency,
                source: NpcPurchaseSource::Used, charged: 0, admitted_count: 1, incoming_unique_id: 0 },
            NpcPurchaseProcessingOutcome::Committed { request, currency: operation.intent.currency,
                source: operation.intent.source, charged: 0, admitted_count: 0, incoming_unique_id: 0 },
            NpcPurchaseProcessingOutcome::Committed { request, currency: operation.intent.currency,
                source: operation.intent.source, charged: 0, admitted_count: 4, incoming_unique_id: 0 },
            NpcPurchaseProcessingOutcome::Rejected { request: bad_request, reason: NpcPurchaseRejection::UnknownGood }] {
            assert_eq!(journal.record(operation, 1, outcome), Err(NpcPurchaseJournalError::InvalidOutcome));
            assert!(journal.entries.is_empty());
        }
        assert_eq!(journal.record(operation, 1, committed(operation)), Ok(()));
        assert!(journal.lookup(&operation).unwrap().is_some());
    }

    #[test]
    fn zero_opaque_sequence_quantity_and_wrong_panel_are_rejected_without_mutation() {
        let mut journal = journal(); let good = gold(1);
        for bad in [NpcPurchaseOperation { actor: [0; 32], ..good },
            NpcPurchaseOperation { request_scope: [0; 32], ..good }, NpcPurchaseOperation { sequence: 0, ..good },
            NpcPurchaseOperation { intent: NpcPurchaseIntent { service_catalog_proof: [0; 32], ..good.intent }, ..good },
            NpcPurchaseOperation { intent: NpcPurchaseIntent { request: NpcPurchaseRequest { count: 0, ..good.intent.request }, ..good.intent }, ..good },
            NpcPurchaseOperation { intent: NpcPurchaseIntent { request: NpcPurchaseRequest { panel_type: 1, ..good.intent.request }, ..good.intent }, ..good }] {
            assert_eq!(journal.record(bad, 1, committed(bad)), Err(NpcPurchaseJournalError::InvalidOperation));
            assert!(journal.entries.is_empty());
        }
        assert_eq!(journal.record(NpcPurchaseOperation { actor: [9; 32], ..good }, 1, committed(good)), Err(NpcPurchaseJournalError::ForeignActor));
        assert_eq!(journal.record(good, 1, committed(good)), Ok(()));
    }

    #[test]
    fn revision_zero_max_repetition_and_reordering_are_refused() {
        let mut journal = journal(); let first = gold(1);
        for revision in [0, u64::MAX] {
            assert_eq!(journal.record(first, revision, committed(first)), Err(NpcPurchaseJournalError::InvalidRevision));
            assert!(journal.entries.is_empty());
        }
        journal.record(first, 5, committed(first)).unwrap();
        for revision in [1, 4, 5] {
            let next = gold(2);
            assert_eq!(journal.record(next, revision, committed(next)), Err(NpcPurchaseJournalError::InvalidRevision));
            assert_eq!(journal.entries.len(), 1);
        }
        let next = gold(2); journal.record(next, 6, committed(next)).unwrap();
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", 6), Ok(()));
    }

    #[test]
    fn final_sequence_and_largest_usable_revision_preserve_full_u64_item_identity() {
        let mut journal = journal(); let mut operation = gold(u64::MAX);
        operation.intent.request.item_index = u64::MAX;
        let outcome = NpcPurchaseProcessingOutcome::Committed {
            request: operation.intent.request, currency: operation.intent.currency, source: operation.intent.source,
            charged: u32::MAX, admitted_count: 1, incoming_unique_id: u64::MAX,
        };
        assert_eq!(journal.record(operation, u64::MAX - 1, outcome.clone()), Ok(()));
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", u64::MAX - 1), Ok(()));
        assert_eq!(journal.lookup(&operation).unwrap().unwrap().outcome, outcome);
        assert_eq!(journal.can_append(), Err(NpcPurchaseJournalError::InvalidRevision));
        assert_eq!(journal.record(gold(1), u64::MAX, committed(gold(1))), Err(NpcPurchaseJournalError::InvalidRevision));
        assert_eq!(journal.entries.len(), 1);
    }

    #[test]
    fn duplicate_keys_are_invalid_even_with_increasing_revisions_or_new_intents() {
        for change_intent in [false, true] {
            let mut journal = journal(); let original = gold(1);
            let mut later = original;
            if change_intent { later.intent.service_catalog_proof = [4; 32]; }
            journal.entries = vec![entry(original, 1), entry(later, 2)];
            assert_eq!(journal.validate_for("account", 0, "ActualPlayer", 2), Err(NpcPurchaseJournalError::DuplicateKey));
            assert_eq!(journal.lookup(&original), Err(NpcPurchaseJournalError::DuplicateKey));
            assert_eq!(journal.can_append(), Err(NpcPurchaseJournalError::DuplicateKey));
        }
    }

    #[test]
    fn request_scopes_are_part_of_the_key_and_sequence_does_not_sort_revisions() {
        let mut journal = journal(); let first = gold(20);
        journal.record(first, 1, committed(first)).unwrap();
        let second = NpcPurchaseOperation { request_scope: [4; 32], ..gold(20) };
        journal.record(second, 2, committed(second)).unwrap();
        let third = gold(1); journal.record(third, 3, committed(third)).unwrap();
        assert_eq!(journal.lookup(&first).unwrap().unwrap().server_revision, 1);
        assert_eq!(journal.lookup(&second).unwrap().unwrap().server_revision, 2);
        assert_eq!(journal.lookup(&third).unwrap().unwrap().server_revision, 3);
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", 3), Ok(()));
    }

    #[test]
    fn full_capacity_refuses_new_purchase_record_and_never_evicts_old_keys() {
        let mut journal = journal();
        journal.entries = (1..=NPC_PURCHASE_JOURNAL_ENTRY_CAP as u64).map(|sequence| entry(gold(sequence), sequence)).collect();
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", NPC_PURCHASE_JOURNAL_ENTRY_CAP as u64), Ok(()));
        assert_eq!(journal.can_append(), Err(NpcPurchaseJournalError::CapacityReached));
        let original = journal.entries[0].clone(); let before = journal.clone();
        let next = gold(NPC_PURCHASE_JOURNAL_ENTRY_CAP as u64 + 1);
        assert_eq!(journal.record(next, next.sequence, committed(next)), Err(NpcPurchaseJournalError::CapacityReached));
        assert_eq!(journal, before);
        assert_eq!(journal.lookup(&original.operation).unwrap(), Some(&original));
        journal.entries.push(entry(next, next.sequence));
        assert_eq!(journal.validate_for("account", 0, "ActualPlayer", next.sequence), Err(NpcPurchaseJournalError::CapacityReached));
    }

    #[test]
    fn corrupted_existing_state_is_never_extended_or_returned_as_a_lookup_hit() {
        let mut valid = journal(); valid.entries = vec![entry(gold(1), 1), entry(gold(2), 2)];
        let mut corruptions = Vec::new();
        let mut bad = valid.clone(); bad.protocol_version = 2; corruptions.push(bad);
        let mut bad = valid.clone(); bad.actor = [0; 32]; corruptions.push(bad);
        let mut bad = valid.clone(); bad.account_id = " account".into(); corruptions.push(bad);
        let mut bad = valid.clone(); bad.entries[0].operation.actor = [9; 32]; corruptions.push(bad);
        let mut bad = valid.clone(); bad.entries[0].operation.intent.service_catalog_proof = [0; 32]; corruptions.push(bad);
        let mut bad = valid.clone(); bad.entries[0].server_revision = 0; corruptions.push(bad);
        let mut bad = valid.clone(); bad.entries[1].server_revision = 1; corruptions.push(bad);
        let mut bad = valid.clone(); bad.entries.swap(0, 1); corruptions.push(bad);
        let mut bad = valid.clone(); bad.entries[0].outcome = NpcPurchaseProcessingOutcome::Rejected {
            request: NpcPurchaseRequest { item_index: 9, ..gold(1).intent.request },
            reason: NpcPurchaseRejection::InsufficientCurrency,
        }; corruptions.push(bad);
        for mut bad in corruptions {
            let before = bad.clone();
            assert!(bad.validate_for("account", 0, "ActualPlayer", 2).is_err());
            assert!(bad.lookup(&gold(1)).is_err()); assert!(bad.can_append().is_err());
            assert!(bad.record(gold(3), 3, committed(gold(3))).is_err()); assert_eq!(bad, before);
        }
    }

    #[test]
    fn strict_camel_case_schema_roundtrips_and_unknown_fields_are_refused() {
        let mut journal = journal(); let operation = gold(1);
        journal.record(operation, 1, committed(operation)).unwrap();
        let encoded = serde_json::to_value(&journal).unwrap();
        assert!(encoded.get("protocolVersion").is_some()); assert!(encoded.get("protocol_version").is_none());
        assert!(encoded["entries"][0]["operation"].get("requestScope").is_some());
        assert!(encoded["entries"][0]["operation"]["intent"].get("serviceCatalogProof").is_some());
        assert!(encoded["entries"][0].get("serverRevision").is_some());
        let result = &encoded["entries"][0]["outcome"]["committed"];
        assert!(result.get("admittedCount").is_some()); assert!(result.get("admitted_count").is_none());
        assert!(result.get("incomingUniqueId").is_some()); assert!(result.get("incoming_unique_id").is_none());
        assert!(result["request"].get("itemIndex").is_some()); assert!(result["request"].get("panelType").is_some());
        assert_eq!(serde_json::from_value::<NpcPurchaseJournal>(encoded.clone()).unwrap(), journal);
        for path in [vec![], vec!["entries", "0"], vec!["entries", "0", "operation"],
            vec!["entries", "0", "operation", "intent"], vec!["entries", "0", "operation", "intent", "request"],
            vec!["entries", "0", "outcome", "committed"], vec!["entries", "0", "outcome", "committed", "request"]] {
            let mut malformed = encoded.clone(); let mut target = &mut malformed;
            for part in path {
                target = if part == "0" { &mut target[0] } else { &mut target[part] };
            }
            target.as_object_mut().unwrap().insert("unexpected".into(), serde_json::json!(true));
            assert!(serde_json::from_value::<NpcPurchaseJournal>(malformed).is_err());
        }
    }

    #[test]
    fn schema_refuses_unknown_outcomes_and_missing_binding_fields() {
        let mut journal = journal(); let operation = gold(1);
        journal.record(operation, 1, rejected(operation)).unwrap();
        let encoded = serde_json::to_value(&journal).unwrap();
        let mut unknown = encoded.clone(); unknown["entries"][0]["outcome"] = serde_json::json!({"unknown": {"detail": "timeout"}});
        assert!(serde_json::from_value::<NpcPurchaseJournal>(unknown).is_err());
        for field in ["protocolVersion", "accountId", "characterIndex", "characterName", "actor", "entries"] {
            let mut missing = encoded.clone(); missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<NpcPurchaseJournal>(missing).is_err());
        }
        let mut wrong_version = encoded; wrong_version["protocolVersion"] = serde_json::json!(2);
        let decoded = serde_json::from_value::<NpcPurchaseJournal>(wrong_version).unwrap();
        assert_eq!(decoded.validate_for("account", 0, "ActualPlayer", 1), Err(NpcPurchaseJournalError::UnsupportedVersion));
    }

    #[test]
    fn serde_refuses_raw_duplicate_and_escaped_duplicate_fields() {
        let encoded = serde_json::to_string(&journal()).unwrap();
        assert!(encoded.contains("\"protocolVersion\":1"));
        for replacement in [r#""protocolVersion":1,"protocolVersion":1"#,
            r#""protocolVersion":1,"protocol\u0056ersion":1"#] {
            let duplicate = encoded.replacen("\"protocolVersion\":1", replacement, 1);
            assert!(serde_json::from_str::<NpcPurchaseJournal>(&duplicate).is_err());
        }
    }

    #[test]
    fn serde_refuses_wrong_opaque_width_and_numeric_types_without_coercion() {
        let mut journal = journal(); let operation = gold(1);
        journal.record(operation, 1, committed(operation)).unwrap();
        let encoded = serde_json::to_value(&journal).unwrap();
        let mut bad_actor = encoded.clone(); bad_actor["actor"] = serde_json::to_value([1u8; 31]).unwrap();
        assert!(serde_json::from_value::<NpcPurchaseJournal>(bad_actor).is_err());
        for wrong_sequence in [serde_json::json!("1"), serde_json::json!(-1), serde_json::json!(1.5)] {
            let mut malformed = encoded.clone(); malformed["entries"][0]["operation"]["sequence"] = wrong_sequence;
            assert!(serde_json::from_value::<NpcPurchaseJournal>(malformed).is_err());
        }
        let mut malformed = encoded; malformed["entries"][0]["serverRevision"] = serde_json::json!("1");
        assert!(serde_json::from_value::<NpcPurchaseJournal>(malformed).is_err());
    }
}

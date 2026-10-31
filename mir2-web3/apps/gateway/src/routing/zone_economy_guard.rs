//! Contain an unresolved debit before the pipeline publishes candidate drops
//! or an item-funded cast. Durable prepared world-effect recovery is separate.
use super::*;

pub(super) const UNKNOWN: &str =
    "ZONE_ECONOMY_COMMIT_OUTCOME_UNKNOWN: unresolved source/world effect";

#[derive(Clone)]
pub(super) struct UnresolvedEconomyCommand {
    pub(super) idempotency_key: String,
    pub(super) context: Option<SharedAccountInventoryExecutionContext>,
    pub(super) envelope: SharedAccountInventoryCommandEnvelope,
}

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn ensure_resolved_economy_command(&self) -> Result<(), String> {
        if self.unresolved_economy_command.is_some()
            || self.pending_player_drop_binding.is_some()
            || self.last_player_drop_journal_result.is_some()
        {
            return Err(UNKNOWN.into());
        }
        Ok(())
    }

    pub(super) fn commit_account_inventory(
        &mut self,
        envelope: SharedAccountInventoryCommandEnvelope,
    ) -> Result<SharedAccountInventoryTransactionReceipt, String> {
        self.ensure_resolved_economy_command()?;
        let original = envelope.clone();
        let outcome = self.commit_account_inventory_outcome(envelope);
        let unresolved = match &outcome {
            SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                idempotency_key,
                execution_context,
                ..
            } => Some((idempotency_key.clone(), Some(execution_context.clone()))),
            SharedAccountInventoryCommitOutcome::LocalSourceOutcomeUnknown {
                idempotency_key,
                ..
            } => Some((idempotency_key.clone(), None)),
            _ => None,
        };
        if let Some((idempotency_key, context)) = unresolved {
            // Preserve the original command, never issue a fresh request ID or
            // roll back a candidate whose durable publication is unresolved.
            self.unresolved_economy_command = Some(UnresolvedEconomyCommand {
                idempotency_key,
                context,
                envelope: original,
            });
            if let Ok(permit) = self.inner.zone_economy_write_permit() {
                permit.commit_outcome_unknown();
            }
            if let Some(key) = self.current_presence_key() {
                if let Ok(mut state) = self.zone_state.lock() {
                    // This also revokes fast movement and live presentation.
                    let _ = state.begin_teardown_fence(&key);
                }
            }
            // Return before the shared pipeline's snapshot/SyncGroundDrops
            // tail. The private candidate cannot become a public world effect.
            return Err(UNKNOWN.into());
        }
        Ok(outcome.into_receipt())
    }
}

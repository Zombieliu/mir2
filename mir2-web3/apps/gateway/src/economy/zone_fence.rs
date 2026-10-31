//! Production economy publication uses the runtime's opaque same-source permit.
//! Legacy raw store APIs remain explicit admin/fixture boundaries.
use super::*;

pub(super) fn runtime_is_trade_participant(
    runtime: &InProcessWorldRuntime,
    first: &SharedTradeOffer,
    second: &SharedTradeOffer,
) -> bool {
    runtime.active_identity().is_some_and(|identity| {
        [first, second].iter().any(|offer| {
            offer.account_id == identity.account_id
                && offer.character_index == identity.character_index
                && offer.character_name == identity.character_name
        })
    })
}

pub(super) fn bound_transaction(transaction: &mut Transaction<'_>) -> Result<(), String> {
    transaction
        .batch_execute(
            "SET LOCAL lock_timeout='250ms'; SET LOCAL statement_timeout='2000ms'; \
         SET LOCAL idle_in_transaction_session_timeout='5000ms'",
        )
        .map_err(|_| "zone economy transaction limits unavailable".into())
}

impl PostgresEconomyStore {
    pub fn bootstrap_character_fenced(
        &self,
        permit: &ZoneEconomyWritePermit,
        context: &SharedAccountInventoryExecutionContext,
        identity: &ActiveSessionIdentity,
        snapshot: &WorldSnapshot,
    ) -> Result<EconomyBootstrapReceipt, String> {
        permit.validate_target_database(&self.database_url)?;
        if !context.external_commit_authorized {
            return Err("zone economy publication is not authorized".into());
        }
        self.bootstrap_character_inner(
            identity,
            snapshot,
            context.created_at_ms,
            Some((permit, context)),
        )
    }

    pub fn lookup_fenced(
        &self,
        permit: &ZoneEconomyWritePermit,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<Option<EconomyTransactionReceipt>, String> {
        permit.validate_target_database(&self.database_url)?;
        // No current lease/write-state admission and no schema provisioning.
        self.lookup_inner(envelope, true)
    }

    pub fn transact_fenced(
        &self,
        permit: &ZoneEconomyWritePermit,
        envelope: &EconomyTransactionEnvelope,
    ) -> Result<EconomyTransactionReceipt, String> {
        permit.validate_target_database(&self.database_url)?;
        self.transact_with_source(envelope, None, Some(permit))
            .map(|(receipt, _)| receipt)
    }

    pub(super) fn transact_prepared_kill_fenced(
        &self,
        permit: &ZoneEconomyWritePermit,
        envelope: &EconomyTransactionEnvelope,
        source: &dyn PreparedKillSource,
    ) -> Result<(EconomyTransactionReceipt, PreparedKillProjection), String> {
        permit.validate_target_database(&self.database_url)?;
        let (receipt, checkpoint) =
            self.transact_with_source(envelope, Some(source), Some(permit))?;
        Ok((
            receipt,
            checkpoint.ok_or("prepared kill checkpoint missing after commit")?,
        ))
    }
}

pub(super) fn read_bootstrap_receipt(
    transaction: &mut Transaction<'_>,
    identity: &ActiveSessionIdentity,
) -> Result<Option<EconomyBootstrapReceipt>, String> {
    let row = transaction.query_opt(
        "SELECT snapshot_digest,gold,experience,item_quantity,item_kind_count,bootstrapped_at_ms,details \
         FROM game_economy_bootstraps WHERE account_id=$1 AND character_index=$2",
        &[&identity.account_id, &identity.character_index],
    ).map_err(|error| format!("economy bootstrap lookup failed: {error}"))?;
    row.map(|row| {
        let opening: EconomyOpeningSnapshot = serde_json::from_value(row.get("details"))
            .map_err(|_| "economy bootstrap receipt details invalid")?;
        let receipt = EconomyBootstrapReceipt {
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
            snapshot_digest: row.get("snapshot_digest"),
            gold: row.get("gold"),
            experience: row.get("experience"),
            item_quantity: row.get("item_quantity"),
            item_kind_count: usize::try_from(row.get::<_, i32>("item_kind_count"))
                .map_err(|_| "economy bootstrap item count invalid")?,
            bootstrapped_at_ms: u64::try_from(row.get::<_, i64>("bootstrapped_at_ms"))
                .map_err(|_| "economy bootstrap timestamp invalid")?,
            duplicate: true,
        };
        if opening.account_id != identity.account_id
            || opening.character_index != identity.character_index
            || opening.digest()? != receipt.snapshot_digest
            || opening.gold != receipt.gold
            || opening.experience != receipt.experience
            || opening.total_item_quantity != receipt.item_quantity
            || opening.item_quantities.len() != receipt.item_kind_count
        {
            return Err("economy bootstrap receipt integrity conflict".into());
        }
        Ok(receipt)
    })
    .transpose()
}

impl PostgresEconomyAccountInventoryService {
    pub(super) fn runtime_permit(
        &self,
        runtime: &InProcessWorldRuntime,
        _context: &SharedAccountInventoryExecutionContext,
    ) -> Result<Option<ZoneEconomyWritePermit>, String> {
        if !self.store.requires_zone_write_permit() {
            return Ok(None);
        }
        let permit = runtime.zone_economy_write_permit()?;
        Ok(Some(permit))
    }

    pub(in crate::economy) fn mark_uncertain(&self, key: &str) {
        self.uncertain_transactions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key.to_string());
    }

    pub(in crate::economy) fn is_uncertain(&self, key: &str) -> bool {
        self.uncertain_transactions
            .lock()
            .map(|keys| keys.contains(key))
            .unwrap_or(true)
    }

    pub(super) fn bootstrap_runtime_fenced(
        &self,
        runtime: &InProcessWorldRuntime,
        context: &SharedAccountInventoryExecutionContext,
    ) -> Result<EconomyBootstrapReceipt, String> {
        let identity = runtime
            .active_identity()
            .ok_or("economy bootstrap requires active identity")?;
        let permit = self.runtime_permit(runtime, context)?;
        self.store.bootstrap_character_fenced(
            permit.as_ref(),
            context,
            &identity,
            &runtime.world_snapshot(),
        )
    }

    pub(super) fn commit_with_zone_permit(
        &self,
        runtime: &mut InProcessWorldRuntime,
        context: Option<&SharedAccountInventoryExecutionContext>,
        envelope: SharedAccountInventoryCommandEnvelope,
        reconciliation_only: bool,
    ) -> SharedAccountInventoryCommitOutcome {
        let failed = || Self::failed_receipt(&envelope.command);
        if self.store.requires_zone_write_permit()
            && matches!(
                envelope.command,
                SharedAccountInventoryCommand::GoldDrop { .. }
                    | SharedAccountInventoryCommand::InventoryItemDrop { .. }
            )
        {
            // A permit alone cannot replace the complete source transition and
            // durable world intent required by ordinary player drops.
            return SharedAccountInventoryCommitOutcome::Deferred { receipt: failed() };
        }
        let Some(context) = context else {
            return SharedAccountInventoryCommitOutcome::Deferred { receipt: failed() };
        };
        if runtime.active_identity().as_ref() != Some(&envelope.identity) {
            return SharedAccountInventoryCommitOutcome::Deferred { receipt: failed() };
        }
        let stable_key = envelope.stable_idempotency_key();
        if !context.external_commit_authorized {
            // Verified standby replay never contacts the production store.
            if self
                .projected_receipts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains_key(&stable_key)
            {
                return SharedAccountInventoryCommitOutcome::Confirmed(
                    Self::already_materialized_receipt(&envelope.command),
                );
            }
            let receipt = Self::apply_projection(runtime, &envelope);
            if receipt.committed {
                self.projected_receipts
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(stable_key, receipt.clone());
            }
            return SharedAccountInventoryCommitOutcome::Confirmed(receipt);
        }
        let permit = match self.runtime_permit(runtime, context) {
            Ok(permit) => permit,
            Err(_) => return SharedAccountInventoryCommitOutcome::Deferred { receipt: failed() },
        };
        if let (Some(store), SharedAccountInventoryCommand::MonsterKillAward(award)) =
            (self.store.prepared_kill_store(), &envelope.command)
        {
            let key = award
                .source_receipt_key
                .clone()
                .unwrap_or_else(|| format!("zone:{}:{stable_key}", context.zone_id));
            let mut award = award.clone();
            award.source_receipt_key = Some(key.clone());
            return match store.commit_runtime_kill(
                runtime,
                context,
                &envelope.identity,
                &key,
                &award,
                reconciliation_only || self.is_uncertain(&key),
            ) {
                Ok((_, packets)) => SharedAccountInventoryCommitOutcome::Confirmed(
                    SharedAccountInventoryTransactionReceipt {
                        kind: SharedAccountInventoryTransactionKind::MonsterKillAward,
                        committed: true,
                        packets,
                    },
                ),
                Err(mir2_simulation::PreparedKillPublicationFailure::Rejected(_)) => {
                    SharedAccountInventoryCommitOutcome::Deferred { receipt: failed() }
                }
                Err(mir2_simulation::PreparedKillPublicationFailure::OutcomeUnknown(_)) => {
                    self.mark_uncertain(&key);
                    SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                        idempotency_key: key,
                        execution_context: context.clone(),
                        receipt: failed(),
                    }
                }
            };
        }
        let experience_delta = match &envelope.command {
            SharedAccountInventoryCommand::MonsterKillAward(award) => {
                Some(runtime.shared_monster_kill_experience_balance_delta(award.experience))
            }
            _ => None,
        };
        let Some(transaction) =
            economy_transaction_for_command(context, &envelope, experience_delta)
        else {
            return SharedAccountInventoryCommitOutcome::Confirmed(match &envelope.command {
                SharedAccountInventoryCommand::MonsterKillAward(award) if award.experience == 0 => {
                    Self::apply_projection(runtime, &envelope)
                }
                _ => failed(),
            });
        };
        let unknown = || {
            self.mark_uncertain(&transaction.idempotency_key);
            SharedAccountInventoryCommitOutcome::OutcomeUnknown {
                idempotency_key: transaction.idempotency_key.clone(),
                execution_context: context.clone(),
                receipt: failed(),
            }
        };
        let receipt = match self.store.lookup_fenced(permit.as_ref(), &transaction) {
            Ok(Some(receipt)) => receipt,
            Ok(None) => {
                if reconciliation_only || self.is_uncertain(&transaction.idempotency_key) {
                    return unknown();
                }
                if !preflight_projection(runtime, &envelope.command) {
                    return SharedAccountInventoryCommitOutcome::Confirmed(failed());
                }
                match self.store.bootstrap_character_fenced(
                    permit.as_ref(),
                    context,
                    &envelope.identity,
                    &runtime.world_snapshot(),
                ) {
                    Ok(_) => {}
                    Err(error) if error.contains("COMMIT_OUTCOME_UNKNOWN") => return unknown(),
                    Err(_) => {
                        return SharedAccountInventoryCommitOutcome::Deferred { receipt: failed() }
                    }
                }
                match self.store.transact_fenced(permit.as_ref(), &transaction) {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        // Once a commit may have happened, absence on a later
                        // read cannot authorize rejection or a second writer.
                        let uncertain = error.contains("COMMIT_OUTCOME_UNKNOWN");
                        if uncertain {
                            self.mark_uncertain(&transaction.idempotency_key);
                        }
                        match self.store.lookup_fenced(permit.as_ref(), &transaction) {
                            Ok(Some(receipt)) => receipt,
                            Ok(None) if !uncertain => {
                                return SharedAccountInventoryCommitOutcome::Confirmed(failed())
                            }
                            Ok(None) | Err(_) => return unknown(),
                        }
                    }
                }
            }
            Err(error) if error.contains("ZONE_ECONOMY_WRITE_PERMIT_REJECTED") => {
                return SharedAccountInventoryCommitOutcome::Deferred { receipt: failed() }
            }
            Err(_) => return unknown(),
        };
        let projection = self.materialize_external_receipt(runtime, &envelope, &receipt);
        if projection.committed {
            self.projected_receipts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(stable_key, projection.clone());
            return SharedAccountInventoryCommitOutcome::Confirmed(projection);
        }
        // Every command retains its original durable operation when source
        // projection cannot be confirmed, including debit/consumption paths.
        unknown()
    }

    pub(super) fn settle_trade_authorized(
        &self,
        permit: Option<&ZoneEconomyWritePermit>,
        context: &SharedAccountInventoryExecutionContext,
        first: &SharedTradeOffer,
        second: &SharedTradeOffer,
        reconciliation_only: bool,
    ) -> SharedTradeSettlementOutcome {
        let transaction = match economy_transaction_for_trade(context, first, second) {
            Ok(Some(transaction)) => transaction,
            Ok(None) => return SharedTradeSettlementOutcome::Committed,
            Err(_) => return SharedTradeSettlementOutcome::Rejected,
        };
        let unknown = || {
            self.mark_uncertain(&transaction.idempotency_key);
            SharedTradeSettlementOutcome::OutcomeUnknown {
                idempotency_key: transaction.idempotency_key.clone(),
                execution_context: context.clone(),
            }
        };
        let outcome = |receipt: EconomyTransactionReceipt| {
            if receipt.duplicate {
                SharedTradeSettlementOutcome::DurableDuplicate {
                    event_id: receipt.event_id,
                }
            } else {
                SharedTradeSettlementOutcome::DurableCommitted {
                    event_id: receipt.event_id,
                }
            }
        };
        match self.store.lookup_fenced(permit, &transaction) {
            Ok(Some(receipt)) => outcome(receipt),
            Ok(None) => {
                if reconciliation_only || self.is_uncertain(&transaction.idempotency_key) {
                    return unknown();
                }
                match self.store.transact_fenced(permit, &transaction) {
                    Ok(receipt) => outcome(receipt),
                    Err(error) => {
                        let uncertain = error.contains("COMMIT_OUTCOME_UNKNOWN");
                        if uncertain {
                            self.mark_uncertain(&transaction.idempotency_key);
                        }
                        match self.store.lookup_fenced(permit, &transaction) {
                            Ok(Some(receipt)) => outcome(receipt),
                            Ok(None) if !uncertain => SharedTradeSettlementOutcome::Rejected,
                            Ok(None) | Err(_) => unknown(),
                        }
                    }
                }
            }
            Err(error) if error.contains("ZONE_ECONOMY_WRITE_PERMIT_REJECTED") => {
                SharedTradeSettlementOutcome::Deferred
            }
            Err(_) => unknown(),
        }
    }
}

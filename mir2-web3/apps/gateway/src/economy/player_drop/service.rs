use super::*;
use crate::routing::SharedPlayerDropCommit;
use mir2_protocol::{Point, UserItem};
use mir2_simulation::{
    PlayerDropRequest, PreparedPlayerDropPublication, PreparedPlayerDropPublicationFailure, ZoneKey,
};

fn failure(error: String) -> PreparedPlayerDropPublicationFailure {
    if error.contains("COMMIT_OUTCOME_UNKNOWN") {
        PreparedPlayerDropPublicationFailure::OutcomeUnknown(error)
    } else {
        PreparedPlayerDropPublicationFailure::Rejected(error)
    }
}

fn count_item_nodes(item: &UserItem) -> Result<usize, String> {
    let mut count = 1usize;
    for child in item.slots.iter().flatten() {
        count = count
            .checked_add(count_item_nodes(child)?)
            .ok_or("player drop item tree overflow")?;
        if count > 1024 {
            return Err("player drop item tree exceeds UID reservation bound".into());
        }
    }
    Ok(count)
}

fn assign_item_nodes(
    item: &mut UserItem,
    ids: &mut std::slice::Iter<'_, u64>,
) -> Result<(), String> {
    item.unique_id = *ids
        .next()
        .ok_or("player drop UID reservation omitted an item node")?;
    for child in item.slots.iter_mut().flatten() {
        assign_item_nodes(child, ids)?;
    }
    Ok(())
}

impl PostgresEconomyAccountInventoryService {
    #[cfg(test)]
    pub(crate) fn fail_next_player_drop_admission_phase_for_test(&self, phase: u8) {
        assert!((1..=4).contains(&phase));
        self.test_player_drop_admission_phase
            .store(phase, std::sync::atomic::Ordering::Release);
    }

    #[cfg(test)]
    fn player_drop_admission_phase_failed_for_test(&self, phase: u8) -> bool {
        self.test_player_drop_admission_phase
            .compare_exchange(
                phase,
                0,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::economy) fn publish_player_drop(
        &self,
        runtime: &mut InProcessWorldRuntime,
        binding: PlayerDropOperationBinding,
        zone: &ZoneKey,
        position: Point,
        created_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        request: PlayerDropRequest,
    ) -> Result<SharedPlayerDropCommit, PreparedPlayerDropPublicationFailure> {
        let store = self.store.prepared_kill_store().ok_or_else(|| {
            "durable player drops require real PostgreSQL source custody".to_string()
        })?;
        let permit = runtime.zone_economy_write_permit()?;
        permit.validate_target_database(&store.database_url)?;
        let identity = runtime
            .active_identity()
            .ok_or_else(|| "player drop requires active identity".to_string())?;
        binding.validate_integrity()?;
        if binding.account_id() != identity.account_id
            || binding.character_index() != identity.character_index
            || binding.zone_id() != permit.fence().zone_id()
        {
            return Err("player drop runtime identity or Zone binding conflict"
                .to_string()
                .into());
        }
        let key = binding.operation_key();
        let unknown = |detail: String| {
            self.mark_uncertain(&key);
            PreparedPlayerDropPublicationFailure::OutcomeUnknown(format!(
                "{}: {detail}",
                permit.commit_outcome_unknown()
            ))
        };
        // Exact old outcomes precede all gameplay, placement, bootstrap and UID
        // issuance. A durable Prepared without a commit is unresolved custody.
        if let Some(record) = store
            .lookup_player_drop_operation(&binding)
            .map_err(|error| unknown(error))?
        {
            if record.status() == PlayerDropOperationStatus::Aborted {
                return Err(
                    "player drop original operation was durably aborted; it cannot be replayed"
                        .to_string()
                        .into(),
                );
            }
            if record.status() == PlayerDropOperationStatus::Prepared {
                return Err(unknown(
                    "player drop remains durably Prepared; query-only recovery required".into(),
                ));
            }
            if record.operation.intent().zone() != zone
                || record.operation.intent().position() != &position
                || record.operation.intent().created_at_unix_ms() != created_at_unix_ms
                || record.operation.intent().expires_at_unix_ms() != expires_at_unix_ms
            {
                return Err("player drop retry differs from its original frozen intent"
                    .to_string()
                    .into());
            }
            validate_retried_request(record.operation(), &request)?;
            if let Some(admission) = store
                .lookup_player_drop_admission(&binding)
                .map_err(|error| unknown(error))?
            {
                admission
                    .validate_operation(record.operation())
                    .map_err(|error| unknown(error))?;
            }
            runtime.reconcile_shared_player_drop_receipt(record.operation())?;
            return Ok(SharedPlayerDropCommit {
                operation: record.operation.clone(),
                receipt: record.commit_receipt()?,
                effect: record.committed_effect(&permit, &store.database_url)?,
                packets: vec![],
            });
        }
        if self.is_uncertain(&key) {
            return Err(unknown("player drop receipt remains absent".into()));
        }
        if let Some(admission) = store
            .lookup_player_drop_admission(&binding)
            .map_err(|error| unknown(error))?
        {
            let frozen = admission.frozen();
            if frozen.zone() != zone
                || frozen.position() != &position
                || frozen.created_at_unix_ms() != created_at_unix_ms
                || frozen.expires_at_unix_ms() != expires_at_unix_ms
                || frozen.request() != &request
            {
                return Err("player drop retry differs from its durable admission"
                    .to_string()
                    .into());
            }
            return match admission.status() {
                PlayerDropAdmissionStatus::Aborted => Err("player drop admission was durably resolved without publication".to_string().into()),
                _ => Err(unknown("player drop admission has no final source receipt; query-only recovery required".into())),
            };
        }
        permit.validate_context(binding.zone_id(), binding.owner_epoch())?;
        if binding.owner_id() != permit.fence().owner_id() {
            return Err("player drop original owner mismatch".to_string().into());
        }
        let preview = runtime.preview_shared_player_drop(&request)?;
        let frozen = FrozenPlayerDropAdmission::new(
            binding.clone(),
            zone.clone(),
            position.clone(),
            created_at_unix_ms,
            expires_at_unix_ms,
            request.clone(),
            preview.clone(),
        )?;
        let admission = match store.admit_player_drop(&permit, &frozen).map_err(|error| {
            if error.contains("COMMIT_OUTCOME_UNKNOWN") {
                unknown(error)
            } else {
                failure(error)
            }
        })? {
            PlayerDropAdmissionPublication::Fresh(capability) => capability,
            PlayerDropAdmissionPublication::Existing(_) => return Err(unknown(
                "player drop admission was already recorded; lookup cannot resume its side effects"
                    .into(),
            )),
        };
        let publication = (|| {
            admission.validate(&permit, &store.database_url, &frozen)?;
            #[cfg(test)]
            if self.player_drop_admission_phase_failed_for_test(1) {
                return Err(unknown(
                    "injected gap after acknowledged admission before bootstrap".into(),
                ));
            }
            let context = SharedAccountInventoryExecutionContext {
                zone_id: crate::ZoneId::new(binding.zone_id()),
                fencing_generation: binding.owner_epoch(),
                source_sequence: binding.source_sequence(),
                created_at_ms: created_at_unix_ms,
                external_commit_authorized: true,
            };
            store
                .bootstrap_character_fenced(&permit, &context, &identity, &runtime.world_snapshot())
                .map_err(|error| {
                    if error.contains("COMMIT_OUTCOME_UNKNOWN") {
                        unknown(error)
                    } else {
                        failure(error)
                    }
                })?;
            #[cfg(test)]
            if self.player_drop_admission_phase_failed_for_test(2) {
                return Err(unknown(
                    "injected gap after acknowledged bootstrap before UID reservation".into(),
                ));
            }
            admission.validate(&permit, &store.database_url, &frozen)?;
            let resolved = match preview {
                PlayerDropOutcome::Item {
                    loot:
                        GroundDropLootSnapshot::InventoryItem {
                            exact_item: Some(mut exact),
                            ..
                        },
                    ..
                } if !exact.uid_assigned => {
                    let count = count_item_nodes(&exact.item)?;
                    let ids = store
                        .reserve_player_drop_item_uids(&permit, &binding, count)
                        .map_err(|error| {
                            if error.contains("COMMIT_OUTCOME_UNKNOWN") {
                                unknown(error)
                            } else {
                                failure(error)
                            }
                        })?;
                    let mut ids = ids.iter();
                    assign_item_nodes(&mut exact.item, &mut ids)?;
                    if ids.next().is_some() {
                        return Err("player drop UID reservation contains extra item nodes"
                            .to_string()
                            .into());
                    }
                    exact.uid_assigned = true;
                    Some(exact)
                }
                _ => None,
            };
            #[cfg(test)]
            if self.player_drop_admission_phase_failed_for_test(3) {
                return Err(unknown(
                    "injected gap after acknowledged UID reservation before Prepared".into(),
                ));
            }
            let (record, packets) = runtime
                .commit_shared_player_drop_with_resolved_item_via_postgres(
                    binding.clone(),
                    zone,
                    position,
                    created_at_unix_ms,
                    expires_at_unix_ms,
                    request,
                    resolved,
                    |source| {
                        store
                            .prepare_admitted_player_drop_operation(&permit, &admission, source)
                            .map_err(failure)?;
                        #[cfg(test)]
                        if self.player_drop_admission_phase_failed_for_test(4) {
                            return Err(unknown(
                                "injected gap after Prepared admission link before source commit"
                                    .into(),
                            ));
                        }
                        let receipt = store
                            .commit_player_drop_operation(&permit, source)
                            .map_err(failure)?;
                        let record = store
                            .lookup_player_drop_operation(&binding)
                            .map_err(|error| unknown(error))?
                            .ok_or_else(|| {
                                unknown(
                                    "committed player drop has no canonical operation receipt"
                                        .into(),
                                )
                            })?;
                        if record.receipt().map(|stored| stored.event_id.as_str())
                            != Some(receipt.event_id.as_str())
                        {
                            return Err(unknown(
                                "player drop canonical receipt conflicts after commit".into(),
                            ));
                        }
                        Ok(if receipt.duplicate {
                            PreparedPlayerDropPublication::AlreadyCommitted(record)
                        } else {
                            PreparedPlayerDropPublication::Written(record)
                        })
                    },
                )
                .map_err(|error| match error {
                    PreparedPlayerDropPublicationFailure::OutcomeUnknown(detail) => unknown(detail),
                    other => other,
                })?;
            Ok(SharedPlayerDropCommit {
                operation: record.operation.clone(),
                receipt: record.commit_receipt().map_err(|error| unknown(error))?,
                effect: record
                    .committed_effect(&permit, &store.database_url)
                    .map_err(|error| unknown(error))?,
                packets,
            })
        })();
        match publication {
            Err(PreparedPlayerDropPublicationFailure::Rejected(detail)) => {
                // A known refusal before Prepared can be closed under this
                // exact acknowledged admission. Unknown or Linked operations
                // retain their identity and require successor reconciliation.
                match store.reject_admitted_player_drop(
                    &permit,
                    &admission,
                    "ordinary player drop rejected before source preparation",
                ) {
                    Ok(()) => Err(PreparedPlayerDropPublicationFailure::Rejected(detail)),
                    Err(error) => Err(unknown(format!(
                        "admitted request could not be closed: {error}"
                    ))),
                }
            }
            result => result,
        }
    }
}

/// Bind a retry to the original source instance, never merely the same item key.
fn validate_retried_request(
    operation: &PlayerDropPreparedOperation,
    request: &PlayerDropRequest,
) -> Result<(), String> {
    match (request, operation.intent().outcome()) {
        (PlayerDropRequest::Gold { amount }, PlayerDropOutcome::Gold { amount: expected })
            if amount == expected =>
        {
            Ok(())
        }
        (
            PlayerDropRequest::Item {
                unique_id,
                count,
                hero_inventory: false,
            },
            PlayerDropOutcome::Item { quantity, .. },
        )
        | (
            PlayerDropRequest::Item {
                unique_id,
                count,
                hero_inventory: false,
            },
            PlayerDropOutcome::DestroyedItem { quantity, .. },
        ) if u32::from(*count) == *quantity => {
            #[derive(Deserialize)]
            struct Item {
                unique_id: u64,
                quantity: u32,
            }
            let parse=|save:&mir2_simulation::CharacterSaveRecord|->Result<BTreeMap<u64,u32>,String>{
                save.inventory_items_json.iter().map(|value|{let item:Item=serde_json::from_str(value).map_err(|e|e.to_string())?;Ok((item.unique_id,item.quantity))}).collect()
            };
            let before = parse(operation.runtime_before())?;
            let after = parse(operation.checkpoint())?;
            if before
                .get(unique_id)
                .copied()
                .unwrap_or(0)
                .checked_sub(after.get(unique_id).copied().unwrap_or(0))
                == Some(*quantity)
            {
                Ok(())
            } else {
                Err("player drop retry source item differs from its frozen instance".into())
            }
        }
        _ => Err("player drop retry differs from frozen request".into()),
    }
}

//! One authoritative session may own one unresolved remote save operation.
//! The database owns the immutable snapshot and the final commit evidence.
use super::*;

const RECOVERY: &str = "zone_save_operation_requires_recovery";
const BUSY: &str = "zone_save_operation_pending";
const UNKNOWN: &str = "ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN: zone operation unresolved";

#[derive(Clone)]
pub(super) struct HostedSaveOperation {
    binding: ZoneSaveOperationBinding,
    prepared: PreparedZoneTeardown,
    prepare_unknown: bool,
    released: bool,
}

fn lease_for(binding: &ZoneSaveOperationBinding) -> ZoneOwnerLease {
    let fence = binding.fence();
    ZoneOwnerLease::new(
        ZoneId::new(fence.zone_id()),
        fence.owner_id(),
        fence.fencing_token(),
    )
}

fn validate_pair(
    prepared: &PreparedZoneTeardown,
    receipt: &ZoneSaveOperationReceipt,
) -> Result<(), String> {
    prepared.validate_identity_checkpoint()?;
    let op = receipt.operation();
    if prepared.owner_lease() != &lease_for(op.binding())
        || prepared.identity().account_id != op.account_id()
        || prepared.identity().character_index != op.character_index()
        || serde_json::to_value(prepared.checkpoint()).map_err(|_| RECOVERY)?
            != serde_json::to_value(op.frozen_checkpoint()).map_err(|_| RECOVERY)?
    {
        return Err(RECOVERY.into());
    }
    Ok(())
}

impl HostedZoneOwnerCommandClient {
    pub(super) fn reject_pending_remote_save(&self) -> Result<(), String> {
        if self
            .remote_save_operation
            .lock()
            .map_err(|_| RECOVERY)?
            .as_ref()
            .is_some_and(|p| !p.released)
        {
            return Err(BUSY.into());
        }
        Ok(())
    }

    pub fn prepare_remote_zone_teardown(
        &self,
        lease: &ZoneOwnerLease,
        operation_id: &str,
        rpc_session_id: &str,
    ) -> Result<Option<ZoneSaveOperationReceipt>, String> {
        let authority = self
            .owner_lease_authority
            .as_ref()
            .filter(|a| a.requires_transactional_zone_save_fence())
            .ok_or(RECOVERY)?;
        let binding = ZoneSaveOperationBinding::new(
            operation_id.into(),
            rpc_session_id.into(),
            mir2_simulation::ZoneSaveFence::new(
                lease.zone_id().as_str().into(),
                lease.owner_id().into(),
                lease.fencing_token(),
            )?,
        )?;
        let mut slot = self.runtime.lock().map_err(|_| RECOVERY)?;
        let runtime = slot.as_mut().ok_or(RECOVERY)?;
        bind_runtime_zone_save_fence(runtime, Some(authority), lease)?;
        let mut pending = self.remote_save_operation.lock().map_err(|_| RECOVERY)?;
        let existing = runtime.query_zone_save_operation(&binding)?;
        if let Some(receipt) = existing {
            if receipt.status() != ZoneSaveOperationStatus::Prepared {
                return Ok(Some(receipt));
            }
            let local = pending
                .as_ref()
                .filter(|p| p.binding == binding)
                .ok_or(RECOVERY)?;
            validate_pair(&local.prepared, &receipt)?;
            if runtime.active_identity().as_ref() != Some(local.prepared.identity()) {
                return Err(RECOVERY.into());
            }
            return Ok(Some(receipt));
        }
        if let Some(local) = pending.as_ref() {
            if local.binding != binding && !local.released {
                return Err(BUSY.into());
            }
            if local.prepare_unknown {
                return Err(UNKNOWN.into());
            }
        }
        if pending.as_ref().is_none_or(|p| p.released) {
            let Some(prepared) = prepare_zone_teardown_checkpoint(runtime, lease)? else {
                return Ok(None);
            };
            *pending = Some(HostedSaveOperation {
                binding: binding.clone(),
                prepared,
                prepare_unknown: false,
                released: false,
            });
        }
        let local = pending.as_mut().ok_or(RECOVERY)?;
        match runtime.prepare_zone_save_operation(binding, local.prepared.checkpoint()) {
            Ok(receipt) => {
                validate_pair(&local.prepared, &receipt)?;
                Ok(Some(receipt))
            }
            Err(error) => {
                local.prepare_unknown = error.contains("COMMIT_OUTCOME_UNKNOWN");
                Err(error)
            }
        }
    }

    pub fn query_remote_zone_teardown(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<Option<ZoneSaveOperationReceipt>, String> {
        let slot = self.runtime.lock().map_err(|_| RECOVERY)?;
        slot.as_ref()
            .ok_or(RECOVERY)?
            .query_zone_save_operation(binding)
    }

    pub fn persist_remote_zone_teardown(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<ZoneSaveOperationReceipt, String> {
        let mut slot = self.runtime.lock().map_err(|_| RECOVERY)?;
        let runtime = slot.as_mut().ok_or(RECOVERY)?;
        let receipt = runtime.query_zone_save_operation(binding)?.ok_or(UNKNOWN)?;
        if receipt.status() != ZoneSaveOperationStatus::Prepared {
            return Ok(receipt);
        }
        let authority = self
            .owner_lease_authority
            .as_ref()
            .filter(|a| a.requires_transactional_zone_save_fence())
            .ok_or(RECOVERY)?;
        let lease = lease_for(binding);
        bind_runtime_zone_save_fence(runtime, Some(authority), &lease)?;
        let pending = self.remote_save_operation.lock().map_err(|_| RECOVERY)?;
        let local = pending
            .as_ref()
            .filter(|p| &p.binding == binding)
            .ok_or(RECOVERY)?;
        validate_pair(&local.prepared, &receipt)?;
        let committed = runtime.persist_zone_save_operation(receipt.operation())?;
        validate_pair(&local.prepared, &committed)?;
        if committed.status() == ZoneSaveOperationStatus::Prepared {
            return Err(UNKNOWN.into());
        }
        Ok(committed)
    }

    pub fn release_remote_zone_teardown(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<ZoneSaveOperationReceipt, String> {
        let mut slot = self.runtime.lock().map_err(|_| RECOVERY)?;
        let runtime = slot.as_mut().ok_or(RECOVERY)?;
        let receipt = runtime.query_zone_save_operation(binding)?.ok_or(UNKNOWN)?;
        if receipt.status() == ZoneSaveOperationStatus::Prepared {
            return Err(UNKNOWN.into());
        }
        let mut pending = self.remote_save_operation.lock().map_err(|_| RECOVERY)?;
        let Some(local) = pending.as_ref().filter(|p| &p.binding == binding) else {
            // A durable Released row does not prove that this process released
            // its in-memory fence. A restart needs explicit reconciliation.
            return Err(RECOVERY.into());
        };
        validate_pair(&local.prepared, &receipt)?;
        // A committed receipt alone cannot promote unconfirmed live state or
        // clear a shared Unknown freeze. Only an already-published source image
        // can resume the same in-memory admission.
        let current = runtime.active_character_checkpoint().ok_or(RECOVERY)?;
        let saved = receipt.saved_checkpoint().ok_or(RECOVERY)?;
        if runtime.active_identity().as_ref() != Some(local.prepared.identity())
            || current.revision != saved.revision
        {
            return Err(RECOVERY.into());
        }
        let authority = self.owner_lease_authority.as_ref().ok_or(RECOVERY)?;
        authority.validate_owner_lease(&lease_for(binding))?;
        let released = runtime.release_zone_save_operation(binding)?;
        if released.status() != ZoneSaveOperationStatus::Released {
            return Err(UNKNOWN.into());
        }
        release_zone_teardown_fence(runtime)?;
        // Keep a bounded confirmation for lost replies. It grants no write
        // permit and is replaced only when the next operation is prepared.
        pending.as_mut().ok_or(RECOVERY)?.released = true;
        Ok(released)
    }
}

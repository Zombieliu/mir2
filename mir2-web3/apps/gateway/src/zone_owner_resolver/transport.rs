//! One session's immutable binding to a resolved authoritative Zone owner.
//!
//! A transport is never rebuilt when ownership changes. Its owner, epoch and
//! endpoint remain pinned until the caller performs an explicit session rebind.

use super::{ResolveError, ResolvedZoneOwner, StableZoneOwnerResolver};
use crate::npc_purchase_owner_route::NpcPurchaseOwnerRouteExecution;
use crate::routing::{
    PreparedZoneTeardown, SharedZoneLiveOutboundSender, SharedZoneOwnerRpcTransport,
    ZoneLiveOutboundRegistration, ZoneOwnerCommandRequest, ZoneOwnerLease, ZoneOwnerRpcTransport,
};
use crate::zone_rpc::{TcpZoneOwnerRpcTransport, ZoneRpcLimits};
use mir2_protocol::ServerPacket;
use mir2_simulation::{
    ActiveSessionIdentity, CharacterHandoffCheckpoint, CharacterHandoffCommitment,
    CharacterSaveRecord, NativeAttackError, NativeAttackExecution, NpcPurchaseDurableError,
    NpcPurchaseOwnerAction, WorldCommandExecution, WorldSnapshot, ZoneSaveFence,
    ZoneSaveOperationBinding, ZoneSaveOperationReceipt, ZoneSaveOperationStatus,
};
use rand::RngCore;
use std::sync::{Arc, Mutex};

const SAVE_RECOVERY: &str = "zone_save_operation_requires_recovery";
const SAVE_UNKNOWN: &str =
    "ZONE_SAVE_COMMIT_OUTCOME_UNKNOWN: pending owner save requires receipt reconciliation";

#[derive(Debug)]
struct PendingSaveOperation {
    binding: ZoneSaveOperationBinding,
    prepare_unknown: bool,
    persist_unknown: bool,
    release_unknown: bool,
}

#[derive(Debug)]
pub struct StableZoneOwnerRpcTransport {
    resolver: Arc<StableZoneOwnerResolver>,
    pin: ResolvedZoneOwner,
    inner: SharedZoneOwnerRpcTransport,
    session_id: String,
    pending_save: Mutex<Option<PendingSaveOperation>>,
}

impl StableZoneOwnerRpcTransport {
    /// Bind exactly the supplied lease and verify the actual Host's identity
    /// before creating any remote session or dispatching a player mutation.
    /// Construction performs a synchronous, read-only Health request.
    pub fn new(
        resolver: Arc<StableZoneOwnerResolver>,
        initial_lease: ZoneOwnerLease,
        session_id: String,
        token: Option<String>,
        limits: ZoneRpcLimits,
    ) -> Result<Self, String> {
        let pin = resolver
            .require_lease(&initial_lease)
            .map_err(|error| error.to_string())?;
        let guard_resolver = resolver.clone();
        let guard_pin = pin.clone();
        // Exactly one endpoint: no active-shard hashing, standby selection or
        // mutation replay is possible, even if an operation loses its reply.
        let inner = TcpZoneOwnerRpcTransport::with_explicit_options(
            pin.endpoint(),
            pin.lease().zone_id().clone(),
            session_id.clone(),
            token,
            limits,
        )
        .with_owner_route_guard(Arc::new(move || {
            guard_resolver
                .require_exact(&guard_pin)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }));
        let health_transport = inner.clone();
        Self::from_transport(resolver, pin, session_id, Arc::new(inner), move || {
            health_transport.health().map(|health| health.host_id)
        })
    }

    fn from_transport(
        resolver: Arc<StableZoneOwnerResolver>,
        pin: ResolvedZoneOwner,
        session_id: String,
        inner: SharedZoneOwnerRpcTransport,
        health_host_id: impl FnOnce() -> Result<String, String>,
    ) -> Result<Self, String> {
        ZoneSaveOperationBinding::new(
            "validate-session".into(),
            session_id.clone(),
            ZoneSaveFence::new(
                pin.lease().zone_id().as_str().into(),
                pin.lease().owner_id().into(),
                pin.lease().fencing_token(),
            )?,
        )?;
        let transport = Self {
            resolver,
            pin,
            inner,
            session_id,
            pending_save: Mutex::new(None),
        };
        transport.validate_pin()?;
        if health_host_id()? != transport.pin.lease().owner_id() {
            return Err("zone_owner_host_identity_mismatch".into());
        }
        // Observation or the Host probe may block. A successful old observation
        // cannot authorize a session if the epoch changed during that IO.
        transport.validate_pin()?;
        Ok(transport)
    }

    fn validate_pin(&self) -> Result<(), String> {
        self.resolver
            .require_exact(&self.pin)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn validate_request_lease(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        if lease != self.pin.lease() {
            return Err(ResolveError::RequiresRebind.to_string());
        }
        self.validate_pin()
    }

    fn read<T>(
        &self,
        operation: impl FnOnce(&dyn ZoneOwnerRpcTransport) -> Result<T, String>,
    ) -> Result<T, String> {
        self.validate_pin()?;
        let value = operation(self.inner.as_ref())?;
        self.validate_pin()?;
        Ok(value)
    }

    fn capability(&self, operation: impl FnOnce(&dyn ZoneOwnerRpcTransport) -> bool) -> bool {
        self.validate_pin().is_ok() && operation(self.inner.as_ref()) && self.validate_pin().is_ok()
    }

    fn validate_save_binding(&self, binding: &ZoneSaveOperationBinding) -> Result<(), String> {
        let lease = self.pin.lease();
        if binding.rpc_session_id() != self.session_id
            || binding.fence().zone_id() != lease.zone_id().as_str()
            || binding.fence().owner_id() != lease.owner_id()
            || binding.fence().fencing_token() != lease.fencing_token()
        {
            return Err(ResolveError::RequiresRebind.to_string());
        }
        Ok(())
    }

    fn require_save_capability(&self) -> Result<(), String> {
        self.validate_pin()?;
        if !self.inner.supports_zone_save_operations() {
            return Err("zone_save_operations_unsupported".into());
        }
        self.validate_pin()
    }

    /// Receipt reconciliation targets the original pinned Host and original
    /// session even after expiry/takeover. It never authorizes a new mutation.
    fn query_save_receipt(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<Option<ZoneSaveOperationReceipt>, String> {
        self.validate_save_binding(binding)?;
        if !self.inner.supports_zone_save_operations() {
            return Err("zone_save_operations_unsupported".into());
        }
        let receipt = self.inner.query_zone_save_operation(binding)?;
        if receipt
            .as_ref()
            .is_some_and(|r| r.operation().binding() != binding)
        {
            return Err(SAVE_UNKNOWN.into());
        }
        Ok(receipt)
    }

    fn new_save_operation_id() -> String {
        let mut bytes = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let suffix = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!("zone-save-{suffix}")
    }
}

impl ZoneOwnerRpcTransport for StableZoneOwnerRpcTransport {
    fn supports_zone_save_operations(&self) -> bool {
        self.capability(|inner| inner.supports_zone_save_operations())
    }

    fn prepare_zone_save_operation(
        &self,
        lease: &ZoneOwnerLease,
        operation_id: &str,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        let binding = ZoneSaveOperationBinding::new(
            operation_id.into(),
            self.session_id.clone(),
            ZoneSaveFence::new(
                lease.zone_id().as_str().into(),
                lease.owner_id().into(),
                lease.fencing_token(),
            )?,
        )?;
        self.validate_save_binding(&binding)?;
        let mut pending = self.pending_save.lock().map_err(|_| SAVE_RECOVERY)?;
        if let Some(local) = pending.as_ref() {
            if local.binding != binding {
                return Err("zone_save_operation_pending".into());
            }
        } else {
            self.validate_request_lease(lease)?;
            self.require_save_capability()?;
            // Publish the immutable ID before any Prepare can reach the Host.
            *pending = Some(PendingSaveOperation {
                binding: binding.clone(),
                prepare_unknown: false,
                persist_unknown: false,
                release_unknown: false,
            });
        }
        if let Some(receipt) = self.query_save_receipt(&binding)? {
            if receipt.status() == ZoneSaveOperationStatus::Prepared {
                self.validate_request_lease(lease)?;
                pending.as_mut().ok_or(SAVE_RECOVERY)?.prepare_unknown = false;
            }
            return PreparedZoneTeardown::from_remote_operation(&receipt).map(Some);
        }
        let local = pending.as_mut().ok_or(SAVE_RECOVERY)?;
        if local.prepare_unknown || local.persist_unknown || local.release_unknown {
            return Err(SAVE_UNKNOWN.into());
        }
        self.validate_request_lease(lease)?;
        self.require_save_capability()?;
        match self.inner.prepare_zone_save_operation(lease, operation_id) {
            Ok(Some(prepared)) => {
                if prepared.remote_operation_binding() != Some(&binding) {
                    local.prepare_unknown = true;
                    return Err(SAVE_UNKNOWN.into());
                }
                Ok(Some(prepared))
            }
            Ok(None) => {
                *pending = None;
                Ok(None)
            }
            Err(error) => {
                local.prepare_unknown = error.contains("COMMIT_OUTCOME_UNKNOWN");
                Err(error)
            }
        }
    }

    fn persist_zone_save_operation(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<ZoneSaveOperationReceipt, String> {
        self.validate_save_binding(binding)?;
        let mut pending = self.pending_save.lock().map_err(|_| SAVE_RECOVERY)?;
        if pending
            .as_ref()
            .is_some_and(|local| &local.binding != binding)
        {
            return Err(SAVE_RECOVERY.into());
        }
        let receipt = self.query_save_receipt(binding)?.ok_or(SAVE_UNKNOWN)?;
        if receipt.status() != ZoneSaveOperationStatus::Prepared {
            return Ok(receipt);
        }
        let local = pending.as_mut().ok_or(SAVE_RECOVERY)?;
        if local.persist_unknown {
            return Err(SAVE_UNKNOWN.into());
        }
        self.require_save_capability()?;
        match self.inner.persist_zone_save_operation(binding) {
            Ok(receipt) => {
                if receipt.operation().binding() != binding
                    || receipt.status() == ZoneSaveOperationStatus::Prepared
                {
                    local.persist_unknown = true;
                    return Err(SAVE_UNKNOWN.into());
                }
                Ok(receipt)
            }
            Err(error) => {
                local.persist_unknown = error.contains("COMMIT_OUTCOME_UNKNOWN");
                Err(error)
            }
        }
    }

    fn query_zone_save_operation(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<Option<ZoneSaveOperationReceipt>, String> {
        let pending = self.pending_save.lock().map_err(|_| SAVE_RECOVERY)?;
        if pending
            .as_ref()
            .is_some_and(|local| &local.binding != binding)
        {
            return Err(SAVE_RECOVERY.into());
        }
        self.query_save_receipt(binding)
    }

    fn release_zone_save_operation(
        &self,
        binding: &ZoneSaveOperationBinding,
    ) -> Result<ZoneSaveOperationReceipt, String> {
        self.validate_save_binding(binding)?;
        let mut pending = self.pending_save.lock().map_err(|_| SAVE_RECOVERY)?;
        if pending
            .as_ref()
            .is_some_and(|local| &local.binding != binding)
        {
            return Err(SAVE_RECOVERY.into());
        }
        let receipt = self.query_save_receipt(binding)?.ok_or(SAVE_UNKNOWN)?;
        if receipt.status() == ZoneSaveOperationStatus::Prepared {
            return Err(SAVE_UNKNOWN.into());
        }
        // Released is durable DB evidence, not proof that this Host also
        // released its in-memory shared teardown fence. Confirm on the same
        // live pinned Host before publishing success or forgetting this ID.
        if pending.as_ref().is_some_and(|local| local.release_unknown)
            && receipt.status() != ZoneSaveOperationStatus::Released
        {
            return Err(SAVE_UNKNOWN.into());
        }
        self.require_save_capability()?;
        if pending.is_none() {
            if receipt.status() != ZoneSaveOperationStatus::Released {
                return Err(SAVE_RECOVERY.into());
            }
            *pending = Some(PendingSaveOperation {
                binding: binding.clone(),
                prepare_unknown: false,
                persist_unknown: false,
                release_unknown: false,
            });
        }
        let local = pending.as_mut().ok_or(SAVE_RECOVERY)?;
        match self.inner.release_zone_save_operation(binding) {
            Ok(receipt) => {
                if receipt.operation().binding() != binding
                    || receipt.status() != ZoneSaveOperationStatus::Released
                {
                    local.release_unknown = true;
                    return Err(SAVE_UNKNOWN.into());
                }
                *pending = None;
                Ok(receipt)
            }
            Err(error) => {
                local.release_unknown = error.contains("COMMIT_OUTCOME_UNKNOWN");
                Err(error)
            }
        }
    }

    fn execute_production_attack_requiring_typed_outcome(
        &self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<NativeAttackExecution, NativeAttackError> {
        self.validate_request_lease(request.owner_lease())
            .map_err(NativeAttackError::BeforeExecution)?;
        // Preserve both a confirmed outcome and an unknown commit exactly. A
        // post-execution lease check must never turn either into safe rejection.
        self.inner
            .execute_production_attack_requiring_typed_outcome(request)
    }

    fn supports_durable_npc_purchase_owner(&self) -> bool {
        self.capability(|inner| inner.supports_durable_npc_purchase_owner())
    }

    fn execute_npc_purchase_owner(
        &self,
        lease: &ZoneOwnerLease,
        authenticated: bool,
        action: NpcPurchaseOwnerAction,
    ) -> Result<NpcPurchaseOwnerRouteExecution, NpcPurchaseDurableError> {
        self.validate_request_lease(lease)
            .map_err(crate::npc_purchase_owner_route::before)?;
        self.inner
            .execute_npc_purchase_owner(lease, authenticated, action)
    }

    fn on_connect(&self) -> Result<Vec<ServerPacket>, String> {
        self.validate_pin()?;
        self.inner.on_connect()
    }

    fn execute(&self, request: ZoneOwnerCommandRequest) -> Result<WorldCommandExecution, String> {
        self.validate_request_lease(request.owner_lease())?;
        self.inner.execute(request)
    }

    fn execute_requiring_typed_game_shop_purchase_outcome(
        &self,
        request: ZoneOwnerCommandRequest,
    ) -> Result<WorldCommandExecution, String> {
        self.validate_request_lease(request.owner_lease())?;
        self.inner
            .execute_requiring_typed_game_shop_purchase_outcome(request)
    }

    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        self.capability(|inner| inner.supports_typed_game_shop_purchase_outcome())
    }

    fn supports_native_game_shop_gifts(&self) -> bool {
        self.capability(|inner| inner.supports_native_game_shop_gifts())
    }

    fn world_snapshot(&self) -> Result<WorldSnapshot, String> {
        self.read(|inner| inner.world_snapshot())
    }

    fn active_identity(&self) -> Result<Option<ActiveSessionIdentity>, String> {
        self.read(|inner| inner.active_identity())
    }

    fn active_character_checkpoint(&self) -> Result<Option<CharacterSaveRecord>, String> {
        self.read(|inner| inner.active_character_checkpoint())
    }

    fn restore_active_character_checkpoint(
        &self,
        checkpoint: &CharacterSaveRecord,
    ) -> Result<(), String> {
        self.validate_pin()?;
        self.inner.restore_active_character_checkpoint(checkpoint)
    }

    fn prepare_character_handoff(
        &self,
        lease: &ZoneOwnerLease,
    ) -> Result<CharacterHandoffCheckpoint, String> {
        self.validate_request_lease(lease)?;
        self.inner.prepare_character_handoff(lease)
    }

    fn resume_character_handoff(
        &self,
        lease: &ZoneOwnerLease,
        checkpoint: &CharacterHandoffCheckpoint,
    ) -> Result<CharacterHandoffCommitment, String> {
        self.validate_request_lease(lease)?;
        self.inner.resume_character_handoff(lease, checkpoint)
    }

    fn prepare_teardown_checkpoint(
        &self,
        owner_lease: &ZoneOwnerLease,
    ) -> Result<Option<PreparedZoneTeardown>, String> {
        let operation_id = self
            .pending_save
            .lock()
            .map_err(|_| SAVE_RECOVERY)?
            .as_ref()
            .map(|local| local.binding.operation_id().to_owned())
            .unwrap_or_else(Self::new_save_operation_id);
        self.prepare_zone_save_operation(owner_lease, &operation_id)
    }

    fn persist_teardown_checkpoint(&self, prepared: &PreparedZoneTeardown) -> Result<(), String> {
        let binding = prepared.remote_operation_binding().ok_or(SAVE_RECOVERY)?;
        if prepared.owner_lease() != self.pin.lease() {
            return Err(ResolveError::RequiresRebind.to_string());
        }
        self.persist_zone_save_operation(binding).map(|_| ())
    }

    fn release_teardown_fence(&self) -> Result<(), String> {
        let binding = self
            .pending_save
            .lock()
            .map_err(|_| SAVE_RECOVERY)?
            .as_ref()
            .map(|local| local.binding.clone());
        if let Some(binding) = binding {
            self.release_zone_save_operation(&binding).map(|_| ())
        } else {
            self.require_save_capability()
        }
    }

    fn save_active_character(&self) -> Result<(), String> {
        if let Some(prepared) = self.prepare_teardown_checkpoint(self.pin.lease())? {
            self.persist_teardown_checkpoint(&prepared)?;
            self.release_teardown_fence()?;
        }
        Ok(())
    }

    fn refresh_active_external_mail(&self) -> Result<bool, String> {
        self.validate_pin()?;
        self.inner.refresh_active_external_mail()
    }

    fn close_session(&self, owner_lease: &ZoneOwnerLease) -> Result<(), String> {
        self.validate_request_lease(owner_lease)?;
        self.inner.close_session(owner_lease)
    }

    fn register_live_outbound(
        &self,
        sender: SharedZoneLiveOutboundSender,
    ) -> Result<Option<Box<dyn ZoneLiveOutboundRegistration>>, String> {
        self.validate_pin()?;
        // The concrete TCP transport carries this same pin guard through all
        // clones, including the independently running outbound poll worker.
        self.inner.register_live_outbound(sender)
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;

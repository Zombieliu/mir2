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
    NpcPurchaseOwnerAction, WorldCommandExecution, WorldSnapshot,
};
use std::sync::Arc;

#[derive(Debug)]
pub struct StableZoneOwnerRpcTransport {
    resolver: Arc<StableZoneOwnerResolver>,
    pin: ResolvedZoneOwner,
    inner: SharedZoneOwnerRpcTransport,
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
            session_id,
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
        Self::from_transport(resolver, pin, Arc::new(inner), move || {
            health_transport.health().map(|health| health.host_id)
        })
    }

    fn from_transport(
        resolver: Arc<StableZoneOwnerResolver>,
        pin: ResolvedZoneOwner,
        inner: SharedZoneOwnerRpcTransport,
        health_host_id: impl FnOnce() -> Result<String, String>,
    ) -> Result<Self, String> {
        let transport = Self {
            resolver,
            pin,
            inner,
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
}

impl ZoneOwnerRpcTransport for StableZoneOwnerRpcTransport {
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
        self.validate_request_lease(owner_lease)?;
        self.inner.prepare_teardown_checkpoint(owner_lease)
    }

    fn persist_teardown_checkpoint(&self, prepared: &PreparedZoneTeardown) -> Result<(), String> {
        self.validate_request_lease(prepared.owner_lease())?;
        self.inner.persist_teardown_checkpoint(prepared)
    }

    fn release_teardown_fence(&self) -> Result<(), String> {
        self.validate_pin()?;
        self.inner.release_teardown_fence()
    }

    fn save_active_character(&self) -> Result<(), String> {
        self.validate_pin()?;
        self.inner.save_active_character()
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

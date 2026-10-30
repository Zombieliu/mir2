//! Quiesced replica-only bridge from an opaque canonical PG read to a Session.
use super::*;
use mir2_simulation::PreparedZoneSourceRecovery;

impl HostedZoneOwnerCommandClient {
    pub(crate) fn recover_canonical_source(
        &self,
        source: &PreparedZoneSourceRecovery,
        authoritative: &GatewayConfig,
        context: &SharedAccountInventoryExecutionContext,
    ) -> Result<Vec<ServerPacket>, String> {
        let mut runtime = self.runtime.lock().map_err(|_| "hosted runtime poisoned")?;
        self.reject_pending_remote_save()?;
        let runtime = runtime
            .as_mut()
            .ok_or("hosted runtime already handed off")?;
        let shared = runtime
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
            .ok_or("canonical source recovery requires the shared Zone runtime")?;
        shared.ensure_resolved_economy_command()?;
        if !shared
            .ranking_replica_zones
            .lock()
            .map_err(|_| "replica registry poisoned")?
            .contains(&shared.inventory_zone_id)
            || context.zone_id != shared.inventory_zone_id
            || !context.external_commit_authorized
        {
            return Err("canonical source recovery requires a quiesced replica".into());
        }
        source.validate_source_binding(authoritative)?;
        if let Some(identity) = shared.inner.active_identity() {
            let canonical = source
                .checkpoint(&identity.account_id, identity.character_index)
                .ok_or("canonical source recovery is missing its active character")?;
            let state = shared
                .zone_state
                .lock()
                .map_err(|_| "Zone state poisoned")?;
            let presence = ZonePresenceKey::from_identity(&identity);
            if let Some(session_id) = state.zone_sessions.get(&presence) {
                let key = state
                    .zone_manager
                    .zone_key_for_session(session_id)
                    .ok_or("canonical source recovery presence has no logical Zone")?;
                if key.map_file_name != canonical.map_file_name {
                    return Err("canonical source map differs from the recovered world".into());
                }
            }
            drop(state);
            shared
                .inner
                .install_zone_source_recovery(source, authoritative)?;
        } else {
            shared.inner.rebind_account_store(authoritative);
            shared.inner.bind_zone_save_fence(source.fence().clone())?;
        }
        shared.last_ground_drop_projection_reconciliation_identity = None;
        shared.trade_projection_reconciliation_state = TradeProjectionReconciliationState::Unknown;
        shared.refresh_replica_zone_binding()?;
        Ok(Vec::new())
    }

    /// Run after every Session has installed the first canonical read. A
    /// delivery may advance the shared cache, so it cannot reinstall that read.
    pub(crate) fn recover_canonical_deliveries(
        &self,
        context: &SharedAccountInventoryExecutionContext,
    ) -> Result<Vec<ServerPacket>, String> {
        let mut runtime = self.runtime.lock().map_err(|_| "hosted runtime poisoned")?;
        self.reject_pending_remote_save()?;
        let runtime = runtime
            .as_mut()
            .ok_or("hosted runtime already handed off")?;
        let shared = runtime
            .as_any_mut()
            .downcast_mut::<SharedInProcessZoneSessionRuntime>()
            .ok_or("canonical delivery recovery requires shared runtime")?;
        shared.ensure_resolved_economy_command()?;
        if !shared
            .ranking_replica_zones
            .lock()
            .map_err(|_| "replica registry poisoned")?
            .contains(&shared.inventory_zone_id)
            || context.zone_id != shared.inventory_zone_id
            || !context.external_commit_authorized
        {
            return Err("canonical delivery recovery requires a quiesced replica".into());
        }
        shared
            .inner
            .zone_economy_write_permit()?
            .validate_context(context.zone_id.as_str(), context.fencing_generation)?;
        shared.sync_pending_zone_movement_transform()?;
        shared.settle_pending_zone_vitals_before_save();
        let service = Arc::clone(&shared.account_inventory_service);
        let mut packets =
            service.reconcile_ground_drop_projections_fenced(&mut shared.inner, Some(context));
        packets
            .extend(service.reconcile_trade_projections_fenced(&mut shared.inner, Some(context)));
        if service.has_pending_ground_drop_projection_fenced(&shared.inner, Some(context))
            || service.has_pending_trade_projection_fenced(&shared.inner, Some(context))
        {
            return Err("canonical source recovery has unresolved asset deliveries".into());
        }
        Ok(packets)
    }
}

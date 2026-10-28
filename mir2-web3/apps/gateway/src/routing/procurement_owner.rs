//! Procurement uses the actual private owner and the existing durable lease.
//! No shared world assets or movement are simulated in this module.
use super::*;
use mir2_protocol::procurement::ProcurementRequest;
use mir2_simulation::procurement::ProcurementExecution;

pub(super) fn execute_fenced(
    authority: Option<&SharedZoneOwnerLeaseAuthority>,
    runtime: &mut ZoneRuntimeHandle,
    lease: &ZoneOwnerLease,
    authenticated: bool,
    request: &ProcurementRequest,
    now_ms: u64,
) -> Result<ProcurementExecution, String> {
    if !authenticated {
        return Err("procurementAuthenticationRequired".into());
    }
    let authority = authority.ok_or("procurementOwnerLeaseUnavailable")?;
    if !authority.supports_durable_npc_purchase_lease() {
        return Err("procurementOwnerLeaseUnavailable".into());
    }
    authority.validate_owner_lease(lease)?;
    partition_binding::validate_runtime_execution_domain(runtime, lease)?;
    let mut result = None;
    let guard = crate::session::catch_gateway_panic("procurement fenced owner", || {
        authority.with_durable_npc_purchase_lease(lease, &mut || {
            result = Some(runtime.execute_procurement_owner(authenticated, request, now_ms));
        })
    });
    match guard {
        Ok(Ok(())) => result.ok_or("procurementOwnerRouteUnavailable")?,
        // A guard failure before invocation proves no mutation was attempted.
        Ok(Err(_)) if result.is_none() => Err("procurementOwnerLeaseUnavailable".into()),
        _ => Err("procurementOwnerPublicationOutcomeUnknown".into()),
    }
}

pub(super) fn execute_shared(
    shared: &mut SharedInProcessZoneSessionRuntime,
    authenticated: bool,
    request: &ProcurementRequest,
    now_ms: u64,
) -> Result<ProcurementExecution, String> {
    if !authenticated {
        return Err("procurementAuthenticationRequired".into());
    }
    shared.validate_durable_npc_actor()?;
    shared.force_inner_to_current_zone_transform();
    shared.force_inner_to_current_zone_vitals();
    let basis = shared.prepare_shared_personal_vital_command()?;
    let mut execution = shared
        .inner
        .execute_procurement_owner(authenticated, request, now_ms)?;
    // Everything after this point may follow a durable private asset commit.
    let publish = (|| -> Result<(), String> {
        shared.validate_durable_npc_actor()?;
        if let Some(basis) = basis {
            shared.commit_shared_personal_vital_command(basis, &mut execution.packets)?;
        }
        let identity = shared
            .inner
            .active_identity()
            .ok_or("owner identity retired")?;
        let key = ZonePresenceKey::from_identity(&identity);
        let snapshot = shared.inner.world_snapshot();
        let player = snapshot
            .entities
            .iter()
            .find(|e| e.kind == WorldEntityKind::SelfPlayer)
            .cloned()
            .ok_or("owner player missing")?;
        let mut state = shared
            .zone_state
            .lock()
            .map_err(|_| "zone mutex poisoned")?;
        if !state.players.contains_key(&key) || state.teardown_fenced(&key) {
            return Err("owner presence retired".into());
        }
        state.upsert_player_with_transform_policy(
            key,
            &identity.character_name,
            snapshot.map_file_name.ok_or("owner map missing")?,
            player,
            snapshot.free_bag_slots,
            snapshot.player_pk_points,
            true,
        );
        drop(state);
        shared.normalize_owner_state_packets(&mut execution.packets);
        shared.publish_ranking_inspect_projection();
        Ok(())
    })();
    if publish.is_err() {
        return Err("procurementOwnerPublicationOutcomeUnknown".into());
    }
    Ok(execution)
}

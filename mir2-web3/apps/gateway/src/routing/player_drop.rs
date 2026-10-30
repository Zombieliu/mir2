//! Live complete-source publication and frozen-result replica execution.
use super::*;
use mir2_simulation::{
    PlayerDropJournalResult, PlayerDropOperationBinding, PlayerDropOutcome,
    PlayerDropPlacementKind, PlayerDropRequest, PreparedPlayerDropPublicationFailure,
};
use rand::RngCore;

const UNKNOWN: &str =
    "PLAYER_DROP_COMMIT_OUTCOME_UNKNOWN: durable world custody requires reconciliation";
const LIFETIME_MS: u64 = 30 * 60 * 1000;

fn rejected_packets(request: &PlayerDropRequest) -> Vec<ServerPacket> {
    match request {
        PlayerDropRequest::Gold { .. } => vec![],
        PlayerDropRequest::Item {
            unique_id,
            count,
            hero_inventory,
        } => vec![ServerPacket::DropItem {
            unique_id: *unique_id,
            count: *count,
            hero_inventory: *hero_inventory,
            success: false,
        }],
    }
}

impl SharedInProcessZoneSessionRuntime {
    fn freeze_player_drop(&mut self, binding: PlayerDropOperationBinding) -> String {
        self.pending_player_drop_binding = Some(binding);
        if let Ok(permit) = self.inner.zone_economy_write_permit() {
            permit.commit_outcome_unknown();
        }
        if let Some(key) = self.current_presence_key() {
            if let Ok(mut state) = self.zone_state.lock() {
                let _ = state.begin_teardown_fence(&key);
            }
        }
        UNKNOWN.into()
    }

    fn dispatch_player_drop_result(
        &mut self,
        zone: &ZoneKey,
        applied: mir2_simulation::PlayerDropApplyResult,
    ) -> Result<Vec<ServerPacket>, String> {
        let owner = self.current_presence_key();
        let mut state = self.zone_state.lock().map_err(|_| UNKNOWN)?;
        if let Some(drop) = applied
            .object_id
            .and_then(|id| state.zone_manager.ground_drop_snapshot_for_key(zone, id))
        {
            state.sync_authoritative_zone_drops_for_source(zone, &[drop]);
        }
        let (packets, _, _, _, _, _, _) =
            state.dispatch_zone_outbounds(applied.outbounds, owner.as_ref());
        Ok(packets)
    }

    pub(super) fn try_publish_durable_player_drop(
        &mut self,
        request: PlayerDropRequest,
    ) -> Result<Option<Vec<ServerPacket>>, String> {
        if let Some(frozen) = self.replaying_player_drop_result.take() {
            frozen.validate_integrity()?;
            if frozen.request != request
                || frozen.operation.binding().zone_id() != self.inventory_zone_id.as_str()
            {
                return Err("replica player drop request/Zone mismatch".into());
            }
            if !self
                .ranking_replica_zones
                .lock()
                .map_err(|_| "replica marker poisoned")?
                .contains(&self.inventory_zone_id)
            {
                return Err("frozen player drop requires a replica Zone".into());
            }
            let zone = frozen.operation.intent().zone().clone();
            {
                let mut state = self
                    .zone_state
                    .lock()
                    .map_err(|_| "replica Zone state poisoned")?;
                state
                    .zone_manager
                    .set_player_drop_replica_mode_for_key(&zone, true);
                if let Some(reservation) = &frozen.reservation {
                    state
                        .zone_manager
                        .install_player_drop_replica_reservation_for_key(&zone, reservation)?;
                }
            }
            let mut packets = self
                .inner
                .apply_player_drop_replica_source(&frozen.operation, &frozen.receipt)?;
            let applied = self
                .zone_state
                .lock()
                .map_err(|_| "replica Zone state poisoned")?
                .zone_manager
                .apply_player_drop_replica_result_for_key(
                    &zone,
                    &frozen.operation,
                    &frozen.receipt,
                    frozen.operation.intent().created_at_unix_ms(),
                )?;
            packets.extend(self.dispatch_player_drop_result(&zone, applied)?);
            self.applied_player_drop_during_command = true;
            return Ok(Some(packets));
        }
        if !self
            .account_inventory_service
            .supports_durable_player_drops()
        {
            return Ok(None);
        }
        let context = self
            .economy_execution_context
            .clone()
            .ok_or("durable player drop requires ordered owner context")?;
        if !context.external_commit_authorized {
            // Historical journals retain their original private replay path.
            // New journals always carry the frozen result intercepted above.
            return Ok(None);
        }
        if !self.player_drop_journal_capture_configured {
            return Err("durable player drop requires an ordered Zone Host journal publisher".into());
        }
        let preview = match self.inner.preview_shared_player_drop(&request) {
            Ok(preview) => preview,
            Err(_) => return Ok(Some(rejected_packets(&request))),
        };
        let identity = self
            .inner
            .active_identity()
            .ok_or("durable player drop requires active identity")?;
        let session_id = self
            .current_zone_session_id()
            .ok_or("durable player drop requires source presence")?;
        let permit = self.inner.zone_economy_write_permit()?;
        permit.validate_context(context.zone_id.as_str(), context.fencing_generation)?;
        let mut nonce = [0u8; 32];
        rand::rngs::OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| "player drop operation nonce unavailable")?;
        let nonce: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let binding = PlayerDropOperationBinding::new(
            nonce,
            session_id.as_str().into(),
            identity.account_id,
            identity.character_index,
            context.zone_id.as_str().into(),
            permit.fence().owner_id().into(),
            context.fencing_generation,
            context.source_sequence,
        )?;
        let created = context.created_at_ms;
        let expires = created
            .checked_add(LIFETIME_MS)
            .ok_or("player drop expiry overflow")?;
        let (zone, position, reservation) = {
            let mut state = self
                .zone_state
                .lock()
                .map_err(|_| "player drop Zone state poisoned")?;
            let zone = state
                .zone_manager
                .zone_key_for_session(&session_id)
                .ok_or("player drop source Zone missing")?;
            let position = state
                .zone_manager
                .player_transform(&session_id)
                .ok_or("player drop source transform missing")?
                .0;
            let kind = match preview {
                PlayerDropOutcome::Gold { .. } => Some(PlayerDropPlacementKind::Gold),
                PlayerDropOutcome::Item { .. } => Some(PlayerDropPlacementKind::Item),
                PlayerDropOutcome::DestroyedItem { .. } => None,
            };
            let reservation = match kind {
                Some(kind) => match state.zone_manager.reserve_player_drop_placement_for_key(
                    &zone,
                    &session_id,
                    &binding.world_effect_id(),
                    kind,
                    created,
                ) {
                    Ok(reservation) => Some(reservation),
                    Err(_) => return Ok(Some(rejected_packets(&request))),
                },
                None => None,
            };
            let position = reservation
                .as_ref()
                .map_or(position, |value| value.position().clone());
            (zone, position, reservation)
        };
        self.pending_player_drop_binding = Some(binding.clone());
        let service = Arc::clone(&self.account_inventory_service);
        let committed = match service.publish_durable_player_drop(
            &mut self.inner,
            binding.clone(),
            &zone,
            position,
            created,
            expires,
            request.clone(),
        ) {
            Ok(committed) => committed,
            Err(PreparedPlayerDropPublicationFailure::Rejected(_)) => {
                if let Some(reservation) = &reservation {
                    if let Ok(mut state) = self.zone_state.lock() {
                        let _ = state
                            .zone_manager
                            .release_player_drop_placement_for_key(&zone, reservation);
                    }
                }
                // The original durable Prepared may remain. Resolve it with an
                // audited successor operation before admitting another writer.
                return Err(self.freeze_player_drop(binding));
            }
            Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(_)) => {
                return Err(self.freeze_player_drop(binding));
            }
        };
        let frozen = PlayerDropJournalResult {
            operation: committed.operation,
            receipt: committed.receipt,
            reservation,
            request,
        };
        if frozen.validate_integrity().is_err()
            || frozen.operation.binding() != &binding
            || committed.effect.intent() != frozen.operation.intent()
            || committed.effect.state() != frozen.receipt.state()
        {
            return Err(self.freeze_player_drop(binding));
        }
        let applied_result = self
            .zone_state
            .lock()
            .map_err(|_| UNKNOWN.to_string())
            .and_then(|mut state| {
                state.zone_manager.apply_committed_player_drop_for_key(
                    &zone,
                    &committed.effect,
                    created,
                )
            });
        let applied = match applied_result {
            Ok(applied) => applied,
            Err(_) => return Err(self.freeze_player_drop(binding)),
        };
        let mut packets = committed.packets;
        match self.dispatch_player_drop_result(&zone, applied) {
            Ok(world_packets) => packets.extend(world_packets),
            Err(_) => return Err(self.freeze_player_drop(binding)),
        }
        self.pending_player_drop_binding = None;
        self.applied_player_drop_during_command = true;
        self.last_player_drop_journal_result = Some(frozen);
        Ok(Some(packets))
    }

    pub(super) fn replay_player_drop_journal_result(
        &mut self,
        result: &PlayerDropJournalResult,
    ) -> Result<Vec<ServerPacket>, String> {
        result.validate_integrity()?;
        if self.replaying_player_drop_result.is_some()
            || self.last_player_drop_journal_result.is_some()
        {
            return Err("player drop journal result is already pending".into());
        }
        let packet = match &result.request {
            PlayerDropRequest::Gold { amount } => ClientPacket::DropGold { amount: *amount },
            PlayerDropRequest::Item {
                unique_id,
                count,
                hero_inventory,
            } => ClientPacket::DropItem {
                unique_id: *unique_id,
                count: *count,
                hero_inventory: *hero_inventory,
            },
        };
        let binding = result.operation.binding();
        let previous =
            self.economy_execution_context
                .replace(SharedAccountInventoryExecutionContext {
                    zone_id: ZoneId::new(binding.zone_id()),
                    fencing_generation: binding.owner_epoch(),
                    source_sequence: binding.source_sequence(),
                    created_at_ms: result.operation.intent().created_at_unix_ms(),
                    external_commit_authorized: false,
                });
        self.replaying_player_drop_result = Some(result.clone());
        let execution = self.execute_shared_pipeline(WorldCommand::ClientPacket(packet), None);
        self.economy_execution_context = previous;
        let was_consumed = self.replaying_player_drop_result.take().is_none();
        if !was_consumed {
            return Err("frozen player drop was not applied at its source boundary".into());
        }
        execution
    }
}

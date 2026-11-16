//! Quiesced factory-level world installation. Source/lease validation and
//! complete canonical enumeration belong to the Host recovery coordinator.
use super::*;
use mir2_simulation::{
    PlayerDropCommittedEffect, PlayerDropCustodyState, PlayerDropEffectState, PlayerDropOutcome,
};

impl SharedInProcessZoneRuntimeFactory {
    pub(crate) fn reconcile_player_drop_recovery(
        &self,
        zone_id: &ZoneId,
        proofs: &[PlayerDropCommittedEffect],
        now_ms: u64,
    ) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for proof in proofs {
            proof.intent().validate_integrity()?;
            if proof.intent().binding().zone_id() != zone_id.as_str()
                || !seen.insert(proof.intent().world_effect_id())
            {
                return Err("player-drop recovery owner group or effect identity mismatch".into());
            }
        }
        let resources = {
            let replicas = self
                .replica_zone_ids
                .lock()
                .map_err(|_| "shared Zone replica marker mutex poisoned")?;
            if !replicas.contains(zone_id) {
                return Err("player-drop recovery requires a standby replica".into());
            }
            let resources = self
                .zones
                .lock()
                .map_err(|_| "shared Zone factory mutex poisoned")?
                .get(zone_id)
                .cloned();
            drop(replicas);
            match resources {
                Some(resources) => resources,
                // No original Session is needed to create this inactive owner
                // resource. resources_for_zone observes the replica marker.
                None => self.resources_for_zone(zone_id),
            }
        };
        // Keep the established replicas -> state lock order. Recheck after
        // resource creation and retain the guard through staged installation.
        let replicas = self
            .replica_zone_ids
            .lock()
            .map_err(|_| "shared Zone replica marker mutex poisoned")?;
        if !replicas.contains(zone_id) {
            return Err("player-drop recovery requires a standby replica".into());
        }
        if resources.autonomous_ticks_enabled.load(Ordering::Acquire) {
            return Err("player-drop recovery requires disabled autonomous ticks".into());
        }
        let mut state = resources
            .zone_state
            .lock()
            .map_err(|_| "player-drop recovery Zone state poisoned")?;
        let mut keys: BTreeSet<_> = state.zone_manager.zone_keys().into_iter().collect();
        keys.extend(proofs.iter().map(|proof| proof.intent().zone().clone()));
        let old_objects: Vec<_> = keys
            .iter()
            .flat_map(|key| {
                state
                    .zone_manager
                    .player_drop_custody_states_for_key(key)
                    .into_iter()
                    .filter_map(move |(_, custody)| match custody {
                        PlayerDropCustodyState::Live { object_id, .. } => {
                            Some((key.clone(), object_id))
                        }
                        _ => None,
                    })
            })
            .collect();
        // Decode into a separate manager; failed reconciliation never publishes
        // even the earlier maps. Live outbound registrations remain in `state`.
        let mut staged = state.zone_manager.transaction_fork();
        let mut outbounds = Vec::new();
        let mut live_drops = Vec::new();
        for key in &keys {
            let selected: Vec<_> = proofs
                .iter()
                .filter(|proof| proof.intent().zone() == key)
                .cloned()
                .collect();
            staged.set_player_drop_replica_mode_for_key(key, true);
            outbounds
                .extend(staged.reconcile_committed_player_drops_for_key(key, &selected, now_ms)?);
            for proof in selected {
                let recovered = staged.query_player_drop_effect_for_key(
                    key,
                    proof.intent().world_effect_id(),
                    proof.intent().intent_digest(),
                )?;
                let matches_receipt = match (proof.state(), proof.intent().outcome(), &recovered) {
                    (
                        PlayerDropEffectState::Live,
                        PlayerDropOutcome::DestroyedItem { .. },
                        Some(PlayerDropCustodyState::Destroyed),
                    )
                    | (
                        PlayerDropEffectState::Consumed,
                        _,
                        Some(PlayerDropCustodyState::Consumed),
                    )
                    | (PlayerDropEffectState::Expired, _, Some(PlayerDropCustodyState::Expired)) => {
                        true
                    }
                    (
                        PlayerDropEffectState::Live,
                        outcome,
                        Some(PlayerDropCustodyState::Live { .. }),
                    ) => !matches!(outcome, PlayerDropOutcome::DestroyedItem { .. }),
                    _ => false,
                };
                if !matches_receipt {
                    return Err(
                        "player-drop recovered world disagrees with its canonical receipt".into(),
                    );
                }
                if let Some(PlayerDropCustodyState::Live { object_id, .. }) = recovered {
                    let drop = staged
                        .ground_drop_snapshot_for_key(key, object_id)
                        .ok_or("recovered player-drop live custody lacks its exact snapshot")?;
                    live_drops.push((key.clone(), drop));
                }
            }
        }
        let mut maps = state.maps.clone();
        for (key, object_id) in old_objects {
            if let Some(map) = maps.get_mut(&key.map_file_name) {
                map.ground_drops.remove(&object_id);
                map.drop_ownership_expires_at_ms.remove(&object_id);
                map.drop_expires_at_ms.remove(&object_id);
                map.original_monster_owner_drop_ids.remove(&object_id);
                map.removed_drop_ids.insert(object_id);
            }
        }
        for (key, drop) in &live_drops {
            let map = maps.entry(key.map_file_name.clone()).or_default();
            if map.ground_drops.contains_key(&drop.object_id) {
                return Err(
                    "recovered player-drop projection conflicts with an unrelated drop".into(),
                );
            }
            map.removed_drop_ids.remove(&drop.object_id);
            map.ground_drops.insert(drop.object_id, drop.clone());
        }
        // All fallible work has finished. Publish world custody and projection
        // together, then deliver the already prepared FIFO removals/spawns.
        state.zone_manager = staged;
        state.maps = maps;
        let _ = state.dispatch_zone_outbounds(outbounds, None);
        for (key, drop) in live_drops {
            state.sync_authoritative_zone_drops_for_source(&key, &[drop]);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "player_drop_recovery_tests.rs"]
mod tests;

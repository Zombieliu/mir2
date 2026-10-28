//! Ephemeral accepted owner-execution binding, not a Source map directory,
//! NPC/page authorization, capacity reservation or deterministic read transcript.
//! Nothing here is serde/checkpoint data or an installer exposed to a client.
use super::*;
use mir2_simulation::ZoneNpcPopulationReadSet;
use std::sync::Weak;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NpcOwnerExecutionRole {
    Active,
    VerifiedReplay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NpcOwnerExecutionBinding {
    owner_lease: ZoneOwnerLease,
    source_sequence: u64,
    role: NpcOwnerExecutionRole,
}

/// Preserve the accepted request's full lease; the existing owner entry point
/// remains responsible for lease validation. This is not a validation receipt.
pub(super) fn accepted_execution_binding(
    owner_lease: &ZoneOwnerLease,
    mode: ZoneOwnerCommandMode,
    source_sequence: Option<u64>,
    active_owner_execution: bool,
) -> Option<NpcOwnerExecutionBinding> {
    if !matches!(
        mode,
        ZoneOwnerCommandMode::ProductionPlayer {
            authenticated: true
        }
    ) {
        return None;
    }
    let source_sequence = source_sequence.filter(|sequence| *sequence > 0)?;
    Some(NpcOwnerExecutionBinding {
        owner_lease: owner_lease.clone(),
        source_sequence,
        role: if active_owner_execution {
            NpcOwnerExecutionRole::Active
        } else {
            NpcOwnerExecutionRole::VerifiedReplay
        },
    })
}

#[derive(Debug, Default)]
pub(super) struct NpcOwnerExecutionSlot {
    generation: u64,
    occupied: bool,
    binding: Option<NpcOwnerExecutionBinding>,
}

/// The slot belongs to one personal runtime, not shared world state. Keeping an
/// independent Drop guard clears it even when execution returns Err or unwinds.
pub(super) struct NpcOwnerExecutionScope {
    slot: Arc<Mutex<NpcOwnerExecutionSlot>>,
}
impl Drop for NpcOwnerExecutionScope {
    fn drop(&mut self) {
        // Removing authority from a poisoned slot is safe. A future enter/read
        // still rejects its poison; this does not restore or mint authority.
        let mut slot = self
            .slot
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        slot.binding = None;
        slot.occupied = false;
    }
}

#[derive(Debug, Clone)]
struct OwnerExecutionPopulationCapture {
    slot: Weak<Mutex<NpcOwnerExecutionSlot>>,
    scope_generation: u64,
    binding: NpcOwnerExecutionBinding,
    actor_presence: ZonePresenceKey,
    session_id: SessionId,
    presence_epoch: u64,
    population: ZoneNpcPopulationReadSet,
}

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn begin_npc_owner_execution_scope(
        &self,
        binding: Option<NpcOwnerExecutionBinding>,
    ) -> Result<NpcOwnerExecutionScope, String> {
        let mut slot = self
            .npc_owner_execution_slot
            .lock()
            .map_err(|_| "NPC owner execution slot is poisoned".to_string())?;
        if slot.occupied {
            return Err("NPC owner execution scope is already occupied".to_string());
        }
        let next = slot
            .generation
            .checked_add(1)
            .ok_or_else(|| "NPC owner execution generation is exhausted".to_string())?;
        slot.generation = next;
        slot.occupied = true;
        slot.binding = binding;
        Ok(NpcOwnerExecutionScope {
            slot: self.npc_owner_execution_slot.clone(),
        })
    }

    /// This is a point-in-time admitted-Player read under accepted execution.
    /// A future Source adapter must additionally bind the actual resolved
    /// script/page/body and complete successfully loaded Source map directory.
    fn capture_owner_execution_population(
        &self,
    ) -> Result<OwnerExecutionPopulationCapture, String> {
        let (generation, binding) = {
            let slot = self
                .npc_owner_execution_slot
                .lock()
                .map_err(|_| "NPC owner execution slot is poisoned".to_string())?;
            if !slot.occupied {
                return Err("NPC owner execution is absent".to_string());
            }
            let binding = slot.binding.clone().ok_or_else(|| {
                "NPC owner execution has no authenticated source sequence".to_string()
            })?;
            (slot.generation, binding)
        };
        if binding.role != NpcOwnerExecutionRole::Active {
            return Err("NPC replay requires its verified population transcript".to_string());
        }
        if binding.owner_lease.zone_id() != &self.inventory_zone_id {
            return Err("NPC owner execution belongs to another Zone".to_string());
        }
        if !Arc::ptr_eq(&self.zone_state, &self.movement_ingress.zone_state) {
            return Err("NPC movement ingress belongs to another Zone state".to_string());
        }
        let identity = self
            .inner
            .active_identity()
            .ok_or_else(|| "NPC population actor is not active".to_string())?;
        // Match authoritative movement's established lock order, never reverse
        // it or derive admission from current_presence_key's personal fallback.
        let state = self
            .zone_state
            .lock()
            .map_err(|_| "NPC shared Zone state is poisoned".to_string())?;
        let movement = self
            .movement_ingress
            .session_state
            .lock()
            .map_err(|_| "NPC movement presence is poisoned".to_string())?;
        let actor = movement
            .presence_key
            .as_ref()
            .ok_or_else(|| "NPC population actor has no admitted presence".to_string())?;
        if actor.account_id != identity.account_id
            || actor.character_index != identity.character_index
            || state.teardown_fenced(actor)
            || movement.presence_epoch == 0
            || movement.presence_epoch == u64::MAX
        {
            return Err("NPC population actor presence is unavailable".to_string());
        }
        let presence = state
            .players
            .get(actor)
            .ok_or_else(|| "NPC population actor has no real Player presence".to_string())?;
        let session = state
            .zone_sessions
            .get(actor)
            .ok_or_else(|| "NPC population actor has no admitted session".to_string())?;
        if state.zone_session_keys.get(session) != Some(actor) {
            return Err("NPC population actor reverse session is unavailable".to_string());
        }
        let key = state
            .zone_manager
            .zone_key_for_session(session)
            .ok_or_else(|| "NPC population actor has no Manager Zone".to_string())?;
        if normalize_gateway_map_file_name(&key.map_file_name)
            != normalize_gateway_map_file_name(&presence.map_file_name)
            || movement.cached_map_file_name.as_deref() != Some(presence.map_file_name.as_str())
        {
            return Err("NPC population actor map binding is unavailable".to_string());
        }
        let owner = state
            .zone_manager
            .online_owner_proof_for_session(session)
            .ok_or_else(|| "NPC population actor has no current online Node".to_string())?;
        let life = state
            .zone_manager
            .player_life_generation(session)
            .ok_or_else(|| "NPC population actor has no current life".to_string())?;
        // zone_object_id is the real global Node ID; owner-local SelfPlayer IDs
        // are intentionally not substituted for it or compared as equal.
        let population = state
            .zone_manager
            .capture_npc_population(session, &owner, &key, life, presence.zone_object_id)
            .ok_or_else(|| "NPC population Manager capture is unavailable".to_string())?;
        if !population.matches_actor(
            &identity.account_id,
            identity.character_index,
            presence.zone_object_id,
        ) {
            return Err("NPC population actor identity does not match its Node".to_string());
        }
        Ok(OwnerExecutionPopulationCapture {
            slot: Arc::downgrade(&self.npc_owner_execution_slot),
            scope_generation: generation,
            binding,
            actor_presence: actor.clone(),
            session_id: session.clone(),
            presence_epoch: movement.presence_epoch,
            population,
        })
    }

    /// This validates execution/actor lifetime, not other Players or count
    /// freshness. Every later CHECKHUM must capture a new read rather than cache.
    fn owner_execution_population_binding_matches(
        &self,
        captured: &OwnerExecutionPopulationCapture,
    ) -> bool {
        let Some(identity) = self.inner.active_identity() else {
            return false;
        };
        if identity.account_id != captured.actor_presence.account_id
            || identity.character_index != captured.actor_presence.character_index
            || captured.binding.owner_lease.zone_id() != &self.inventory_zone_id
            || !Arc::ptr_eq(&self.zone_state, &self.movement_ingress.zone_state)
        {
            return false;
        }
        let Some(slot) = captured.slot.upgrade() else {
            return false;
        };
        if !Arc::ptr_eq(&slot, &self.npc_owner_execution_slot) {
            return false;
        }
        let Ok(slot) = slot.lock() else {
            return false;
        };
        if !slot.occupied
            || slot.generation != captured.scope_generation
            || slot.binding.as_ref() != Some(&captured.binding)
        {
            return false;
        }
        drop(slot);
        let Ok(state) = self.zone_state.lock() else {
            return false;
        };
        let Ok(movement) = self.movement_ingress.session_state.lock() else {
            return false;
        };
        movement.presence_key.as_ref() == Some(&captured.actor_presence)
            && movement.presence_epoch == captured.presence_epoch
            && movement.presence_epoch != u64::MAX
            && !state.teardown_fenced(&captured.actor_presence)
            && state
                .players
                .get(&captured.actor_presence)
                .is_some_and(|presence| {
                    presence.zone_object_id == captured.population.actor_object_id()
                        && normalize_gateway_map_file_name(&presence.map_file_name)
                            == normalize_gateway_map_file_name(
                                &captured.population.actor_key().map_file_name,
                            )
                        && movement.cached_map_file_name.as_deref()
                            == Some(presence.map_file_name.as_str())
                })
            && state
                .zone_manager
                .npc_population_actor_binding_matches(&captured.population)
            && state.zone_sessions.get(&captured.actor_presence) == Some(&captured.session_id)
            && state.zone_session_keys.get(&captured.session_id) == Some(&captured.actor_presence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn runtime() -> SharedInProcessZoneSessionRuntime {
        let mut runtime = super::super::tests::shared_session_runtime(Arc::new(Mutex::new(
            SharedInProcessZoneState::new(),
        )));
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into(),
            }))
            .unwrap();
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: 0,
            }))
            .unwrap();
        runtime
    }
    fn binding(
        runtime: &SharedInProcessZoneSessionRuntime,
        replay: bool,
    ) -> NpcOwnerExecutionBinding {
        let lease = ZoneOwnerLease::new(runtime.inventory_zone_id.clone(), "isolated-owner", 7);
        let sequence = runtime
            .next_in_process_economy_execution_context(&lease)
            .unwrap()
            .source_sequence;
        accepted_execution_binding(
            &lease,
            ZoneOwnerCommandMode::ProductionPlayer {
                authenticated: true,
            },
            Some(sequence),
            !replay,
        )
        .unwrap()
    }

    #[test]
    fn full_lease_and_real_sequence_are_preserved_without_direct_or_unauthenticated_authority() {
        let lease = ZoneOwnerLease::new(ZoneId::new("isolated"), "full-owner", 23);
        let mode = ZoneOwnerCommandMode::ProductionPlayer {
            authenticated: true,
        };
        let accepted = accepted_execution_binding(&lease, mode, Some(42), true).unwrap();
        assert_eq!(accepted.owner_lease, lease);
        assert_eq!(accepted.source_sequence, 42);
        assert_eq!(accepted.role, NpcOwnerExecutionRole::Active);
        assert_eq!(
            accepted_execution_binding(&lease, mode, Some(42), false)
                .unwrap()
                .role,
            NpcOwnerExecutionRole::VerifiedReplay
        );
        for (mode, sequence) in [
            (ZoneOwnerCommandMode::Direct, Some(42)),
            (
                ZoneOwnerCommandMode::ProductionPlayer {
                    authenticated: false,
                },
                Some(42),
            ),
            (mode, None),
            (mode, Some(0)),
        ] {
            assert!(accepted_execution_binding(&lease, mode, sequence, true).is_none());
        }
    }

    #[test]
    fn real_admitted_node_and_life_zero_are_scoped_and_not_owner_local_ids() {
        let runtime = runtime();
        assert!(runtime.capture_owner_execution_population().is_err());
        let accepted = binding(&runtime, false);
        let scope = runtime
            .begin_npc_owner_execution_scope(Some(accepted.clone()))
            .unwrap();
        let capture = runtime.capture_owner_execution_population().unwrap();
        assert_eq!(capture.binding, accepted);
        assert_eq!(capture.population.actor_life(), 0);
        let state = runtime.zone_state.lock().unwrap();
        let presence = &state.players[&capture.actor_presence];
        assert_ne!(presence.zone_object_id, presence.owner_local_object_id);
        assert_eq!(
            capture.population.actor_object_id(),
            presence.zone_object_id
        );
        drop(state);
        assert_eq!(
            capture
                .population
                .count_for_key(capture.population.actor_key()),
            Some(1)
        );
        assert!(runtime.owner_execution_population_binding_matches(&capture));
        assert!(runtime.begin_npc_owner_execution_scope(None).is_err());
        drop(scope);
        assert!(!runtime.owner_execution_population_binding_matches(&capture));
        assert!(runtime.capture_owner_execution_population().is_err());
        let _new_scope = runtime
            .begin_npc_owner_execution_scope(Some(accepted))
            .unwrap();
        assert!(!runtime.owner_execution_population_binding_matches(&capture));
    }

    #[test]
    fn replay_missing_sequence_wrong_zone_and_exhausted_presence_do_not_mint_live_population() {
        let runtime = runtime();
        let replay = runtime
            .begin_npc_owner_execution_scope(Some(binding(&runtime, true)))
            .unwrap();
        assert!(runtime.capture_owner_execution_population().is_err());
        drop(replay);
        let missing = runtime.begin_npc_owner_execution_scope(None).unwrap();
        assert!(runtime.capture_owner_execution_population().is_err());
        drop(missing);
        let mut wrong = binding(&runtime, false);
        wrong.owner_lease = ZoneOwnerLease::new(ZoneId::new("another-zone"), "isolated-owner", 7);
        let wrong_scope = runtime
            .begin_npc_owner_execution_scope(Some(wrong))
            .unwrap();
        assert!(runtime.capture_owner_execution_population().is_err());
        drop(wrong_scope);
        let _scope = runtime
            .begin_npc_owner_execution_scope(Some(binding(&runtime, false)))
            .unwrap();
        runtime
            .movement_ingress
            .session_state
            .lock()
            .unwrap()
            .presence_epoch = u64::MAX;
        assert!(runtime.capture_owner_execution_population().is_err());
    }

    #[test]
    fn scope_drop_clears_on_error_and_unwind_and_refuses_generation_recycling() {
        let runtime = runtime();
        let accepted = binding(&runtime, false);
        let outcome: Result<(), String> = (|| {
            let _scope = runtime.begin_npc_owner_execution_scope(Some(accepted.clone()))?;
            assert!(runtime.capture_owner_execution_population().is_ok());
            Err("isolated failure".into())
        })();
        assert!(outcome.is_err());
        assert!(runtime.capture_owner_execution_population().is_err());
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _scope = runtime
                .begin_npc_owner_execution_scope(Some(accepted))
                .unwrap();
            panic!("isolated execution unwind");
        }));
        assert!(panic.is_err());
        assert!(!runtime.npc_owner_execution_slot.lock().unwrap().occupied);
        assert!(runtime.capture_owner_execution_population().is_err());
        runtime.npc_owner_execution_slot.lock().unwrap().generation = u64::MAX;
        assert!(runtime.begin_npc_owner_execution_scope(None).is_err());
    }

    #[test]
    fn missing_reverse_admission_and_teardown_fence_invalidate_real_capture() {
        let runtime = runtime();
        let _scope = runtime
            .begin_npc_owner_execution_scope(Some(binding(&runtime, false)))
            .unwrap();
        let captured = runtime.capture_owner_execution_population().unwrap();
        let mut state = runtime.zone_state.lock().unwrap();
        let session = state.zone_sessions[&captured.actor_presence].clone();
        let reverse = state.zone_session_keys.remove(&session).unwrap();
        drop(state);
        assert!(runtime.capture_owner_execution_population().is_err());
        assert!(!runtime.owner_execution_population_binding_matches(&captured));
        let mut state = runtime.zone_state.lock().unwrap();
        state.zone_session_keys.insert(session, reverse);
        state
            .teardown_fences
            .insert(captured.actor_presence.clone());
        drop(state);
        assert!(runtime.capture_owner_execution_population().is_err());
        assert!(!runtime.owner_execution_population_binding_matches(&captured));
    }

    #[test]
    fn replacing_both_gateway_session_indexes_cannot_reuse_the_old_manager_binding() {
        let runtime = runtime();
        let _scope = runtime
            .begin_npc_owner_execution_scope(Some(binding(&runtime, false)))
            .unwrap();
        let captured = runtime.capture_owner_execution_population().unwrap();
        let replacement = SessionId("isolated-different-session".into());
        let mut state = runtime.zone_state.lock().unwrap();
        assert!(state
            .zone_session_keys
            .remove(&captured.session_id)
            .is_some());
        state
            .zone_sessions
            .insert(captured.actor_presence.clone(), replacement.clone());
        state
            .zone_session_keys
            .insert(replacement.clone(), captured.actor_presence.clone());
        // The old real Manager Node remains to expose the otherwise valid
        // independent indexes. No command or replacement Node is fabricated.
        assert!(state
            .zone_manager
            .npc_population_actor_binding_matches(&captured.population));
        drop(state);
        assert!(runtime.capture_owner_execution_population().is_err());
        assert!(!runtime.owner_execution_population_binding_matches(&captured));
        let mut state = runtime.zone_state.lock().unwrap();
        state.zone_session_keys.remove(&replacement);
        state
            .zone_sessions
            .insert(captured.actor_presence.clone(), captured.session_id.clone());
        state
            .zone_session_keys
            .insert(captured.session_id.clone(), captured.actor_presence.clone());
        drop(state);
        assert!(runtime.owner_execution_population_binding_matches(&captured));
        assert!(runtime.capture_owner_execution_population().is_ok());
    }

    #[test]
    fn presence_epoch_and_personal_logout_invalidate_an_otherwise_retained_node() {
        let mut runtime = runtime();
        let _scope = runtime
            .begin_npc_owner_execution_scope(Some(binding(&runtime, false)))
            .unwrap();
        let captured = runtime.capture_owner_execution_population().unwrap();
        runtime
            .movement_ingress
            .session_state
            .lock()
            .unwrap()
            .presence_epoch += 1;
        assert!(!runtime.owner_execution_population_binding_matches(&captured));
        let current = runtime.capture_owner_execution_population().unwrap();
        assert!(runtime.owner_execution_population_binding_matches(&current));
        // Exercise personal logout while this prepared phase still has the
        // old shared Node. A fallback to the Book alone would authorize it.
        runtime
            .inner
            .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
            .unwrap();
        assert!(!runtime.owner_execution_population_binding_matches(&current));
        assert!(runtime.capture_owner_execution_population().is_err());
    }

    struct ExactLease(ZoneOwnerLease);
    impl ZoneOwnerLeaseAuthority for ExactLease {
        fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
            assert_eq!(zone_id, self.0.zone_id());
            self.0.clone()
        }
    }

    #[test]
    fn ordinary_in_process_owner_checks_full_lease_and_clears_scope_on_success_and_error() {
        let runtime = runtime();
        let slot = runtime.npc_owner_execution_slot.clone();
        let state = runtime.zone_state.clone();
        let lease = ZoneOwnerLease::new(runtime.inventory_zone_id.clone(), "accepted-owner", 17);
        let client = InProcessZoneOwnerCommandClient::with_owner_lease_authority(Arc::new(
            ExactLease(lease.clone()),
        ));
        let mut handle: ZoneRuntimeHandle = Box::new(runtime);
        let command = || {
            WorldCommand::ClientPacket(ClientPacket::Turn {
                direction: MirDirection::Down,
            })
        };
        let before = state.lock().unwrap().next_economy_source_sequence;
        client
            .execute(
                &mut handle,
                ZoneOwnerCommandRequest::production_player(lease.clone(), true, command()),
            )
            .unwrap();
        assert_eq!(
            state.lock().unwrap().next_economy_source_sequence,
            before + 1
        );
        assert_eq!(slot.lock().unwrap().generation, 1);
        assert!(!slot.lock().unwrap().occupied);
        assert!(slot.lock().unwrap().binding.is_none());
        // Same zone and fencing token with a different owner_id is rejected
        // before scope entry; the full lease is not reduced to its token.
        let wrong = ZoneOwnerLease::new(
            lease.zone_id().clone(),
            "different-owner",
            lease.fencing_token(),
        );
        assert!(client
            .execute(
                &mut handle,
                ZoneOwnerCommandRequest::production_player(wrong, true, command())
            )
            .is_err());
        assert_eq!(slot.lock().unwrap().generation, 1);
        assert!(client
            .execute(
                &mut handle,
                ZoneOwnerCommandRequest::production_player(
                    lease,
                    false,
                    WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 })
                )
            )
            .is_err());
        assert_eq!(slot.lock().unwrap().generation, 2);
        assert!(!slot.lock().unwrap().occupied);
        assert!(slot.lock().unwrap().binding.is_none());
    }

    #[test]
    fn ordinary_hosted_active_and_verified_replay_clear_their_ephemeral_scopes() {
        let runtime = runtime();
        let slot = runtime.npc_owner_execution_slot.clone();
        let lease = ZoneOwnerLease::new(runtime.inventory_zone_id.clone(), "hosted-owner", 19);
        let client = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            Box::new(runtime),
            Arc::new(ExactLease(lease.clone())),
        );
        let request = |sequence| {
            ZoneOwnerCommandRequest::production_player(
                lease.clone(),
                true,
                WorldCommand::ClientPacket(ClientPacket::Turn {
                    direction: MirDirection::Down,
                }),
            )
            .with_source_sequence(sequence)
        };
        client.execute_request(request(41)).unwrap();
        assert_eq!(slot.lock().unwrap().generation, 1);
        assert!(!slot.lock().unwrap().occupied);
        client.execute_replay_request(request(42)).unwrap();
        assert_eq!(slot.lock().unwrap().generation, 2);
        assert!(!slot.lock().unwrap().occupied);
        assert!(slot.lock().unwrap().binding.is_none());
    }
}

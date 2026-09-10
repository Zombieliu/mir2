use super::*;

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn sync_shared_intelligent_creature_actor(&mut self) -> Vec<ServerPacket> {
        let Some(session_id) = self.current_zone_session_id() else {
            return vec![];
        };
        let Some(key) = self.current_presence_key() else {
            return vec![];
        };
        let snapshot = self.inner.world_snapshot();
        let creature = snapshot
            .stage5_systems
            .active_intelligent_creature()
            .cloned();
        self.sync_current_shared_ground_drops_to_zone(&session_id);
        let map_allows = self.inner.shared_intelligent_creature_map_allowed();
        let candidates = self
            .zone_state
            .lock()
            .expect("shared zone presence mutex")
            .map_layer(snapshot.map_file_name.as_deref())
            .map(|layer| layer.ground_drops.into_values().collect::<Vec<_>>())
            .unwrap_or_default();
        let allowed = snapshot
            .ground_drops
            .iter()
            .chain(candidates.iter())
            .filter(|drop| {
                creature
                    .as_ref()
                    .is_some_and(|pet| intelligent_creature_allows_ground_drop(pet, drop))
                    && self.inner.can_commit_shared_ground_drop_pickup(drop)
            })
            .map(|drop| drop.object_id)
            .collect();
        let (mut packets, spawned) = {
            let mut state = self.zone_state.lock().expect("shared zone presence mutex");
            let outbounds = state.zone_manager.sync_intelligent_creature(
                &session_id,
                creature.clone(),
                allowed,
                snapshot.stage5_systems.group.members.clone(),
                map_allows,
                Self::zone_now_ms(),
            );
            let spawned = state
                .zone_manager
                .intelligent_creature_object_id(&session_id)
                .is_some();
            (
                state.dispatch_zone_outbounds(outbounds, Some(&key)).0,
                spawned,
            )
        };
        if creature.is_some() && !spawned {
            packets.extend(self.inner.dismiss_unspawned_shared_intelligent_creature());
        }
        packets
    }

    pub(super) fn request_shared_intelligent_creature_actor_pickup(
        &mut self,
        location: Point,
        mouse_mode: bool,
    ) -> Vec<ServerPacket> {
        let packets = self.sync_shared_intelligent_creature_actor();
        if let Some(session_id) = self.current_zone_session_id() {
            self.zone_state
                .lock()
                .expect("shared zone presence mutex")
                .zone_manager
                .request_intelligent_creature_pickup(&session_id, mouse_mode, location);
        }
        packets
    }

    pub(super) fn drain_shared_intelligent_creature_actor(&mut self) -> Vec<ServerPacket> {
        let mut packets = self.sync_shared_intelligent_creature_actor();
        let Some(session_id) = self.current_zone_session_id() else {
            return packets;
        };
        let Some(identity) = self.inner.active_identity() else {
            return packets;
        };
        let (operations, intents) = {
            let mut state = self.zone_state.lock().expect("shared zone presence mutex");
            (
                state
                    .zone_manager
                    .peek_intelligent_creature_operations_for_identity(
                        &identity.account_id,
                        identity.character_index,
                    ),
                state
                    .zone_manager
                    .drain_intelligent_creature_pickup_intents(&session_id),
            )
        };
        for operation in operations {
            if operation.owner.account_id != identity.account_id
                || operation.owner.character_index != identity.character_index
            {
                continue;
            }
            if let Ok(reward_packets) = self.inner.commit_shared_intelligent_creature_operation(
                operation.pet_type,
                &operation.operation_id,
            ) {
                self.zone_state
                    .lock()
                    .expect("shared zone presence mutex")
                    .zone_manager
                    .settle_intelligent_creature_operation(
                        &operation.owner.session_id,
                        &operation.operation_id,
                    );
                packets.extend(reward_packets);
            }
        }
        for intent in intents {
            let current = self
                .zone_state
                .lock()
                .expect("shared zone presence mutex")
                .zone_manager
                .intelligent_creature_intent_is_current(&intent);
            if !current
                || intent.owner.account_id != identity.account_id
                || intent.owner.character_index != identity.character_index
            {
                continue;
            }
            let snapshot = self.inner.world_snapshot();
            let Some(creature) = snapshot
                .stage5_systems
                .active_intelligent_creature()
                .filter(|pet| pet.pet_type == intent.pet_type)
                .cloned()
            else {
                continue;
            };
            let (mut claim_packets, claims, awards, damages, heals) = self
                .dispatch_zone_player_command_collecting_claims(
                    ZoneCommand::ClaimGroundDrop {
                        session_id: session_id.clone(),
                        object_id: Some(intent.target.object_id),
                        target: intent.target.position,
                        group_members: snapshot.stage5_systems.group.members.clone(),
                        now_ms: Self::zone_now_ms(),
                    },
                    false,
                );
            if claims.is_empty() {
                self.zone_state
                    .lock()
                    .expect("shared zone presence mutex")
                    .zone_manager
                    .settle_intelligent_creature_pickup(
                        &session_id,
                        intent.creature_object_id,
                        intent.target.object_id,
                    );
            }
            self.filter_stale_owner_vital_packets(&mut claim_packets, &damages);
            claim_packets.extend(self.apply_zone_player_damages(damages));
            self.apply_zone_player_heals(heals);
            claim_packets.extend(self.apply_zone_monster_kill_awards(awards));
            let (rewards, canceled) =
                self.apply_shared_intelligent_creature_drop_claims(&creature, claims);
            remove_object_remove_packets(&mut claim_packets, &canceled);
            claim_packets.extend(rewards);
            packets.extend(claim_packets);
        }
        packets
    }
}

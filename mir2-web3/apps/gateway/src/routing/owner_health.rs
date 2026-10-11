//! Private presentation capability retained through the bounded live channel.
use super::*;
use std::sync::Weak;

pub(crate) trait OwnerHealthValidator: fmt::Debug + Send + Sync {
    fn validate(&self, change: &mir2_simulation::ZoneOwnerHealthChange) -> Result<bool, String>;
}

#[derive(Debug, Clone)]
pub(crate) struct OwnerHealthDelivery {
    pub change: mir2_simulation::ZoneOwnerHealthChange,
    pub validator: Arc<dyn OwnerHealthValidator>,
    pub highest_sent_sequence: Arc<Mutex<u64>>,
}

impl OwnerHealthDelivery {
    pub(crate) fn claim(&self) -> Result<bool, String> {
        if !self.validator.validate(&self.change)? {
            return Ok(false);
        }
        let mut highest = self
            .highest_sent_sequence
            .lock()
            .map_err(|_| "owner health send sequence mutex poisoned".to_string())?;
        if self.change.cursor.health_sequence <= *highest {
            return Ok(false);
        }
        *highest = self.change.cursor.health_sequence;
        Ok(true)
    }
}

#[derive(Debug)]
pub(super) struct LocalOwnerHealthValidator {
    pub state: Weak<Mutex<SharedInProcessZoneState>>,
    pub key: ZonePresenceKey,
    pub registration_id: u64,
}

/// Runtime-only FIFO: health capabilities must not overtake earlier Death,
/// Revived or other normal viewport packets when the live channel is full.
#[derive(Debug)]
pub(super) enum PendingOwnerPresentation {
    Health(mir2_simulation::ZoneOwnerHealthChange),
    Packet(ServerPacket),
}

/// Trusted local preparation only, never a client or checkpoint payload.
/// Repository transactions must finish before this basis can publish a delta.
pub(super) struct SharedPersonalVitalBasis {
    key: ZonePresenceKey,
    cursor: mir2_simulation::ZoneOwnerHealthCursor,
    before: mir2_simulation::LocalPlayerVitalsSnapshot,
    acknowledged_private_death: bool,
    teardown_fence_generation: Option<u64>,
}

impl SharedInProcessZoneSessionRuntime {
    pub(super) fn prepare_shared_personal_vital_command(
        &mut self,
    ) -> Result<Option<SharedPersonalVitalBasis>, String> {
        self.prepare_personal_vital_command_for_fence(None)
    }

    pub(super) fn prepare_teardown_personal_vital_command(
        &mut self,
        key: &ZonePresenceKey,
    ) -> Result<Option<SharedPersonalVitalBasis>, String> {
        self.prepare_personal_vital_command_for_fence(Some(key))
    }

    // Only confirmed internal reward consumers use this during the deterministic
    // drain. Ordinary client/potion commands still reject a teardown fence.
    pub(super) fn prepare_personal_reward_vital_command(
        &mut self,
    ) -> Result<Option<SharedPersonalVitalBasis>, String> {
        let fenced_key = self.current_presence_key().filter(|key| {
            self.zone_state
                .lock()
                .is_ok_and(|state| state.teardown_fenced(key))
        });
        if let Some(key) = fenced_key {
            self.prepare_teardown_personal_vital_command(&key)
        } else {
            self.prepare_shared_personal_vital_command()
        }
    }

    fn prepare_personal_vital_command_for_fence(
        &mut self,
        teardown_key: Option<&ZonePresenceKey>,
    ) -> Result<Option<SharedPersonalVitalBasis>, String> {
        let (Some(session), Some(key)) =
            (self.current_zone_session_id(), self.current_presence_key())
        else {
            return Ok(None);
        };
        let (cursor, hp, max_hp, mp, teardown_fence_generation) = {
            let state = self
                .zone_state
                .lock()
                .map_err(|_| "shared Zone vitals mutex poisoned")?;
            let teardown_fence_generation = if let Some(teardown_key) = teardown_key {
                if teardown_key != &key || !state.teardown_fenced(&key) {
                    return Err("personal vital drain requires its current teardown fence".into());
                }
                Some(
                    *state
                        .teardown_vital_fence_generations
                        .get(&key)
                        .ok_or("personal vital drain requires a runtime fence capability")?,
                )
            } else {
                if state.teardown_fenced(&key) {
                    return Err("personal vital command is fenced for teardown".into());
                }
                None
            };
            let cursor = state
                .zone_manager
                .owner_health_cursor(&session)
                .ok_or("personal vital mutation requires a current shared admission")?;
            let (hp, max_hp, mp) = state
                .zone_manager
                .player_vitals(&session)
                .ok_or("personal vital mutation requires a current shared player")?;
            (cursor, hp, max_hp, mp, teardown_fence_generation)
        };
        let local = self.inner.local_player_vitals_snapshot();
        let acknowledged_private_death = local.player_dead == Some(true)
            && local
                .player_object_id
                .is_some_and(|id| self.owner_dead_entity_ids.contains(&id))
            && !cursor.dead;
        if !acknowledged_private_death {
            self.mirror_personal_owner_pools(hp, max_hp, mp, cursor.dead);
        }
        Ok(Some(SharedPersonalVitalBasis {
            key,
            cursor,
            before: self.inner.local_player_vitals_snapshot(),
            acknowledged_private_death,
            teardown_fence_generation,
        }))
    }

    pub(super) fn commit_shared_personal_vital_command(
        &mut self,
        basis: SharedPersonalVitalBasis,
        packets: &mut Vec<ServerPacket>,
    ) -> Result<(), String> {
        // The normal Source transaction has already either succeeded or rolled
        // back its inventory and pools. No AccountStore work is run while the
        // Zone writer is held here.
        let after = self.inner.local_player_vitals_snapshot();
        let explicit_revive = packets.iter().any(|packet| match packet {
            ServerPacket::Revived => true,
            ServerPacket::ObjectRevived { info } => {
                Some(info.object_id) == basis.before.player_object_id
            }
            _ => false,
        });
        packets.retain(|packet| !matches!(packet, ServerPacket::HealthChanged { .. }));
        if basis.acknowledged_private_death {
            return Ok(());
        }
        let final_pools = {
            let mut state = self
                .zone_state
                .lock()
                .map_err(|_| "shared Zone vitals mutex poisoned")?;
            let session = &basis.cursor.session_id;
            let Some(current) = state.zone_manager.owner_health_cursor(session) else {
                return Ok(());
            };
            let Some((hp, max_hp, mp)) = state.zone_manager.player_vitals(session) else {
                return Ok(());
            };
            let same_admission = state.zone_sessions.get(&basis.key) == Some(session)
                && match basis.teardown_fence_generation {
                    Some(generation) => {
                        state.teardown_fenced(&basis.key)
                            && state.teardown_vital_fence_generations.get(&basis.key)
                                == Some(&generation)
                    }
                    None => !state.teardown_fenced(&basis.key),
                }
                && current.online_owner == basis.cursor.online_owner
                && current.object_id == basis.cursor.object_id;
            let same_life = same_admission
                && current.life_generation == basis.cursor.life_generation
                && current.dead == basis.cursor.dead
                && current.health_sequence >= basis.cursor.health_sequence;
            if same_life {
                if let (
                    Some(before_hp),
                    Some(before_max_hp),
                    Some(before_mp),
                    Some(after_hp),
                    Some(after_max_hp),
                    Some(after_mp),
                ) = (
                    basis.before.player_hp,
                    basis.before.player_max_hp,
                    basis.before.player_mp,
                    after.player_hp,
                    after.player_max_hp,
                    after.player_mp,
                ) {
                    let next_max_hp = max_hp
                        .saturating_add(after_max_hp.saturating_sub(before_max_hp))
                        .max(1);
                    let next_hp = hp
                        .saturating_add(after_hp.saturating_sub(before_hp))
                        .clamp(0, next_max_hp);
                    // Crystal LevelUp refills a corpse without clearing Dead.
                    // Only an actual revive receipt may clear that life flag.
                    let keep_dead = current.dead && !explicit_revive;
                    let next_mp = mp
                        .saturating_add(after_mp.saturating_sub(before_mp))
                        .clamp(0, after.player_max_mp.unwrap_or(i32::MAX).max(0));
                    let out = state.zone_manager.commit_owner_vitals(
                        session,
                        next_hp,
                        next_max_hp,
                        next_mp,
                        keep_dead || after.player_dead.unwrap_or(next_hp <= 0) || next_hp <= 0,
                    );
                    packets.extend(state.dispatch_zone_outbounds(out, Some(&basis.key)).0);
                }
            } else if same_admission {
                // A committed stat refresh belongs to the character admission,
                // while its refill belongs to the prepared life. Retain a real
                // level's ceiling without replaying an old-life heal or revive.
                if let (Some(before_max), Some(after_max)) =
                    (basis.before.player_max_hp, after.player_max_hp)
                {
                    if before_max != after_max {
                        let next_max = max_hp
                            .saturating_add(after_max.saturating_sub(before_max))
                            .max(1);
                        let out = state.zone_manager.commit_owner_vitals(
                            session,
                            hp.clamp(0, next_max),
                            next_max,
                            mp.clamp(0, after.player_max_mp.unwrap_or(i32::MAX).max(0)),
                            current.dead,
                        );
                        packets.extend(state.dispatch_zone_outbounds(out, Some(&basis.key)).0);
                    }
                }
            }
            state
                .zone_manager
                .player_vitals(session)
                .map(|(hp, max_hp, mp)| {
                    (
                        hp,
                        max_hp,
                        mp,
                        state
                            .zone_manager
                            .player_is_dead(session)
                            .unwrap_or(hp <= 0),
                    )
                })
        };
        if let Some((hp, max_hp, mp, dead)) = final_pools {
            self.mirror_personal_owner_pools(hp, max_hp, mp, dead);
        }
        Ok(())
    }

    fn mirror_personal_owner_pools(&mut self, hp: i32, max_hp: i32, mp: i32, dead: bool) {
        let local = self.inner.local_player_vitals_snapshot();
        if local.player_hp != Some(hp)
            || local.player_max_hp != Some(max_hp)
            || local.player_mp != Some(mp)
            || local.player_dead != Some(dead)
        {
            self.inner.force_authoritative_player_vitals_with_max_hp(
                Some(hp),
                Some(max_hp),
                Some(mp),
            );
            self.inner.force_authoritative_player_life(dead);
        }
    }
}

impl OwnerHealthValidator for LocalOwnerHealthValidator {
    fn validate(&self, change: &mir2_simulation::ZoneOwnerHealthChange) -> Result<bool, String> {
        let Some(state) = self.state.upgrade() else {
            return Ok(false);
        };
        let state = state
            .lock()
            .map_err(|_| "owner health Zone mutex poisoned".to_string())?;
        Ok(state.owner_health_registration_is_current(&self.key, self.registration_id, change))
    }
}

#[cfg(test)]
impl SharedZoneLiveOutboundRegistration {
    /// Explicitly prepared pool mutations for the real Host overflow fixture.
    /// No packet/proof or overload flag is fabricated by this helper.
    pub(crate) fn prepare_owner_health_backpressure_for_test(&self) {
        let mut state = self.zone_state.lock().unwrap();
        let session = state.zone_sessions.get(&self.key).unwrap().clone();
        let (_, max_hp, mp) = state.zone_manager.player_vitals(&session).unwrap();
        for index in 0..MAX_PENDING_ZONE_PACKETS_PER_PLAYER + 3 {
            let out = state.zone_manager.commit_owner_vitals(
                &session,
                if index % 2 == 0 { 8 } else { 9 },
                max_hp,
                mp,
                false,
            );
            state.dispatch_zone_outbounds(out, Some(&self.key));
        }
    }
}

impl SharedInProcessZoneState {
    pub(super) fn owner_health_registration_is_current(
        &self,
        key: &ZonePresenceKey,
        registration_id: u64,
        change: &mir2_simulation::ZoneOwnerHealthChange,
    ) -> bool {
        !self.teardown_fenced(key)
            && self.zone_sessions.get(key) == Some(&change.cursor.session_id)
            && self.live_zone_outbounds.get(key).is_some_and(|record| {
                record.registration_id == registration_id
                    && !record.overloaded.load(Ordering::Acquire)
            })
            && self.zone_manager.owner_health_change_is_current(change)
    }

    pub(super) fn queue_owner_health_change(
        &mut self,
        key: ZonePresenceKey,
        change: mir2_simulation::ZoneOwnerHealthChange,
    ) {
        if self.teardown_fenced(&key)
            || !self.live_zone_outbounds.contains_key(&key)
            || self.zone_sessions.get(&key) != Some(&change.cursor.session_id)
            || !self.zone_manager.owner_health_change_is_current(&change)
        {
            return;
        }
        self.queue_owner_presentation(key, PendingOwnerPresentation::Health(change));
    }

    pub(super) fn queue_owner_presentation(
        &mut self,
        key: ZonePresenceKey,
        presentation: PendingOwnerPresentation,
    ) {
        if self.teardown_fenced(&key)
            || self
                .live_zone_outbounds
                .get(&key)
                .is_some_and(|record| record.overloaded.load(Ordering::Acquire))
        {
            return;
        }
        // Older legacy packets may have waited before health registration was
        // prepared. Adopt viewport presentation only; owner-location priority
        // and durable social receipts stay on their existing separate paths.
        if let Some(packets) = self.pending_zone_packets.remove(&key) {
            let (normal, retained): (Vec<_>, Vec<_>) =
                packets.into_iter().partition(is_owner_health_fifo_packet);
            self.pending_zone_owner_health
                .entry(key.clone())
                .or_default()
                .extend(normal.into_iter().map(PendingOwnerPresentation::Packet));
            if !retained.is_empty() {
                self.pending_zone_packets.insert(key.clone(), retained);
            }
        }
        let pending = self
            .pending_zone_owner_health
            .entry(key.clone())
            .or_default();
        if let PendingOwnerPresentation::Packet(packet) = &presentation {
            if let Some(object_id) = coalesced_zone_movement_object_id(packet) {
                // Keep the existing movement backpressure policy, but do not
                // coalesce across a health or actor lifecycle boundary.
                let boundary = pending.iter().rposition(|entry| match entry {
                    PendingOwnerPresentation::Health(_) => true,
                    PendingOwnerPresentation::Packet(ServerPacket::ObjectPlayer { info }) => {
                        info.object_id == object_id
                    }
                    PendingOwnerPresentation::Packet(ServerPacket::ObjectRemove {
                        object_id: id,
                    }) => *id == object_id,
                    PendingOwnerPresentation::Packet(ServerPacket::ObjectDied { info }) => {
                        info.object_id == object_id
                    }
                    PendingOwnerPresentation::Packet(ServerPacket::ObjectRevived { info }) => {
                        info.object_id == object_id
                    }
                    _ => false,
                });
                let mut index = 0;
                pending.retain(|entry| {
                    let keep = boundary.is_some_and(|boundary| index <= boundary)
                        || !matches!(entry, PendingOwnerPresentation::Packet(queued)
                            if coalesced_zone_movement_object_id(queued) == Some(object_id));
                    index += 1;
                    keep
                });
            }
        }
        if let PendingOwnerPresentation::Health(change) = &presentation {
            if pending.last().is_some_and(|prior| {
                matches!(prior,
                PendingOwnerPresentation::Health(prior) if prior.cursor == change.cursor)
            }) {
                return;
            }
        }
        pending.push(presentation);
        self.retry_pending_owner_health(&key);
        if !self.live_zone_outbounds.contains_key(&key) {
            // Closed may have unregistered the sender during retry. Expire only
            // display proofs; normal Source lifecycle controls must still drain.
            if let Some(ordered) = self.pending_zone_owner_health.remove(&key) {
                self.pending_zone_packets.entry(key).or_default().extend(
                    ordered.into_iter().filter_map(|item| match item {
                        PendingOwnerPresentation::Packet(packet) => Some(packet),
                        PendingOwnerPresentation::Health(_) => None,
                    }),
                );
            }
            return;
        }
        if self
            .pending_zone_owner_health
            .get(&key)
            .is_some_and(|p| p.len() > MAX_PENDING_ZONE_PACKETS_PER_PLAYER)
        {
            if let Some(record) = self.live_zone_outbounds.get(&key) {
                record
                    .sender
                    .cancel_registration(record.registration_id, record.overloaded.clone());
            }
            // A subscriber still exists; cancel and rebuild its viewport.
            self.pending_zone_owner_health.remove(&key);
        }
    }

    pub(super) fn retry_pending_owner_health(&mut self, key: &ZonePresenceKey) {
        let Some(changes) = self.pending_zone_owner_health.remove(key) else {
            return;
        };
        let mut pending = Vec::new();
        let mut full = false;
        let mut delivered_removes = Vec::new();
        for presentation in changes {
            if let PendingOwnerPresentation::Health(change) = &presentation {
                if !self.zone_manager.owner_health_change_is_current(change) {
                    continue;
                }
            }
            if full {
                pending.push(presentation);
                continue;
            }
            match presentation {
                PendingOwnerPresentation::Packet(packet) => {
                    let removed = match &packet {
                        ServerPacket::ObjectRemove { object_id } => Some(*object_id),
                        _ => None,
                    };
                    if let Err(packet) = self.try_push_live_zone_outbound(key, packet) {
                        pending.push(PendingOwnerPresentation::Packet(packet));
                        full = true;
                    } else {
                        delivered_removes.extend(removed);
                    }
                }
                PendingOwnerPresentation::Health(change) => {
                    let Some(record) = self.live_zone_outbounds.get(key) else {
                        pending.push(PendingOwnerPresentation::Health(change));
                        full = true;
                        continue;
                    };
                    let Some(validator) = record.owner_health_validator.as_ref() else {
                        pending.push(PendingOwnerPresentation::Health(change));
                        full = true;
                        continue;
                    };
                    if record.overloaded.load(Ordering::Acquire) {
                        continue;
                    }
                    let outbound = SharedZoneLiveOutbound {
                        registration_id: record.registration_id,
                        packet: ServerPacket::HealthChanged {
                            hp: change.hp,
                            mp: change.mp,
                        },
                        overloaded: Some(record.overloaded.clone()),
                        owner_health: Some(OwnerHealthDelivery {
                            change: change.clone(),
                            validator: validator.clone(),
                            highest_sent_sequence: record.owner_health_sent_sequence.clone(),
                        }),
                    };
                    match record.sender.try_send(outbound) {
                        Ok(()) => {}
                        Err(TokioTrySendError::Full(_)) => {
                            pending.push(PendingOwnerPresentation::Health(change));
                            full = true;
                        }
                        Err(TokioTrySendError::Closed(outbound)) => {
                            self.unregister_live_zone_outbound(key, outbound.registration_id());
                            pending.push(PendingOwnerPresentation::Health(change));
                            full = true;
                        }
                    }
                }
            }
        }
        self.forget_delivered_player_removals(key, delivered_removes);
        if !pending.is_empty() {
            self.pending_zone_owner_health.insert(key.clone(), pending);
        }
    }
}

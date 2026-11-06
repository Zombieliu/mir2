//! Ordinary player-drop custody. PostgreSQL owns economic finality; this module
//! projects only verified committed effects and retains immutable pickup identity.
use super::*;
use crate::player_drop::{
    PlayerDropCommittedEffect, PlayerDropEffectState, PlayerDropOutcome, PlayerDropWorldIntent,
};
use crate::runtime::zone::player_drop::PlayerDropEffectRecord;
use crate::runtime::zone::{
    PlayerDropApplyResult, PlayerDropCustodyState, PlayerDropEffectIdentity,
    PlayerDropPlacementKind, PlayerDropPlacementReservation,
};

pub(super) fn committed_player_drop_spawn_packet(drop: &GroundDropSnapshot) -> ServerPacket {
    let mut packet = ground_drop_spawn_packet(drop);
    if let (ServerPacket::ObjectItem { info }, GroundDropLootSnapshot::InventoryItem { key, .. }) =
        (&mut packet, &drop.loot)
    {
        info.grade = crate::runtime::drops::crystal_item_grade_for_key(key);
    }
    packet
}

impl ZoneRuntime {
    pub(crate) fn set_player_drop_replica_mode(&mut self, replica: bool) {
        self.player_drop_replica_mode = replica;
    }
    pub(crate) fn is_player_drop_replica_mode(&self) -> bool {
        self.player_drop_replica_mode
    }
    pub(super) fn player_drop_reserved_cell_count(&self, point: &Point) -> usize {
        self.player_drop_reservations.values().filter(|r| r.position == *point).count()
            + self.claimed_ground_drops.values().filter(|c| c.drop.x == point.x && c.drop.y == point.y).count()
            + self.player_drop_effects.values().filter(|r| {
                matches!(r.state, PlayerDropCustodyState::Live { object_id, .. }
                    if !self.ground_drops.contains_key(&object_id) && !self.claimed_ground_drops.contains_key(&object_id))
                    && r.intent.position() == point
            }).count()
    }

    pub fn reserve_player_drop_placement(
        &mut self,
        session: &SessionId,
        world_effect_id: &str,
        kind: PlayerDropPlacementKind,
        now_ms: u64,
    ) -> Result<PlayerDropPlacementReservation, String> {
        if now_ms == 0 || world_effect_id.is_empty() || world_effect_id.len() > 1024 {
            return Err("invalid player-drop effect identity".into());
        }
        let player = self
            .players
            .get(session)
            .ok_or("player-drop placement requires active Zone player")?;
        if let Some(old) = self.player_drop_reservations.get(world_effect_id) {
            return if old.session_id == *session
                && old.player_object_id == player.object_id
                && old.kind == kind
            {
                Ok(old.clone())
            } else {
                Err("conflicting player-drop placement reservation".into())
            };
        }
        if self.player_drop_effects.contains_key(world_effect_id) {
            return Err("player-drop effect already committed".into());
        }
        let origin = player.position.clone();
        let player_object_id = player.object_id;
        let mut best: Option<(Point, usize)> = None;
        'rings: for radius in 0..=kind.range() {
            for y in origin.y.saturating_sub(radius)..=origin.y.saturating_add(radius) {
                for x in origin.x.saturating_sub(radius)..=origin.x.saturating_add(radius) {
                    if radius > 0 && (x - origin.x).abs().max((y - origin.y).abs()) != radius {
                        continue;
                    }
                    let point = Point { x, y };
                    if self.collision.is_ground_drop_blocked(&point)
                        || !self.can_occupy(&point, None)
                    {
                        continue;
                    }
                    let count = self
                        .ground_drops
                        .values()
                        .filter(|d| d.drop.x == x && d.drop.y == y)
                        .count()
                        + self.player_drop_reserved_cell_count(&point);
                    if count >= crate::runtime::drops::CRYSTAL_DROP_STACK_SIZE {
                        continue;
                    }
                    if best.as_ref().is_none_or(|(_, n)| count < *n) {
                        best = Some((point, count));
                    }
                    if count == 0 {
                        break 'rings;
                    }
                }
            }
        }
        let position = best.ok_or("no legal player-drop placement")?.0;
        let reservation = PlayerDropPlacementReservation {
            zone: self.key.clone(),
            world_effect_id: world_effect_id.to_owned(),
            session_id: session.clone(),
            player_object_id,
            origin,
            position,
            kind,
            reserved_at_unix_ms: now_ms,
        };
        self.player_drop_reservations
            .insert(world_effect_id.to_owned(), reservation.clone());
        Ok(reservation)
    }

    pub(crate) fn install_frozen_player_drop_reservation(
        &mut self,
        slot: &PlayerDropPlacementReservation,
    ) -> Result<(), String> {
        if !self.player_drop_replica_mode {
            return Err("frozen placement replay requires replica Zone".into());
        }
        if slot.zone != self.key
            || slot.world_effect_id.is_empty()
            || slot.reserved_at_unix_ms == 0
            || zone_tile_distance(&slot.origin, &slot.position) > slot.kind.range()
            || self
                .players
                .get(&slot.session_id)
                .is_none_or(|p| p.object_id != slot.player_object_id || p.position != slot.origin)
        {
            return Err("frozen player-drop reservation identity mismatch".into());
        }
        if let Some(old) = self.player_drop_reservations.get(slot.world_effect_id()) {
            return if old == slot {
                Ok(())
            } else {
                Err("conflicting frozen player-drop reservation".into())
            };
        }
        if self
            .player_drop_effects
            .contains_key(slot.world_effect_id())
        {
            return Err("frozen player-drop effect already committed".into());
        }
        self.player_drop_reservations
            .insert(slot.world_effect_id().to_owned(), slot.clone());
        Ok(())
    }

    /// Release only after definite source rejection; Unknown retains its slot.
    pub fn release_player_drop_placement(
        &mut self,
        reservation: &PlayerDropPlacementReservation,
    ) -> bool {
        if self
            .player_drop_reservations
            .get(reservation.world_effect_id())
            != Some(reservation)
        {
            return false;
        }
        self.player_drop_reservations
            .remove(reservation.world_effect_id());
        true
    }

    pub fn player_drop_effect_state(
        &self,
        world_effect_id: &str,
    ) -> Option<PlayerDropCustodyState> {
        Some(self.player_drop_effects.get(world_effect_id)?.state.clone())
    }

    /// Read-only custody inventory for quiesced Host recovery. Includes claimed
    /// live objects whose presentation is already absent from the active set.
    pub fn player_drop_custody_states(&self) -> Vec<(String, PlayerDropCustodyState)> {
        self.player_drop_effects
            .iter()
            .map(|(id, record)| (id.clone(), record.state.clone()))
            .collect()
    }

    pub fn query_player_drop_effect(
        &self,
        world_effect_id: &str,
        intent_digest: &str,
    ) -> Result<Option<PlayerDropCustodyState>, String> {
        let Some(record) = self.player_drop_effects.get(world_effect_id) else {
            return Ok(None);
        };
        if record.intent.intent_digest() != intent_digest {
            return Err("player-drop query immutable intent mismatch".into());
        }
        Ok(Some(record.state.clone()))
    }

    pub fn apply_committed_player_drop(
        &mut self,
        proof: &PlayerDropCommittedEffect,
        now_ms: u64,
    ) -> Result<PlayerDropApplyResult, String> {
        let intent = proof.intent();
        intent.validate_integrity()?;
        if intent.zone() != &self.key {
            return Err("committed player-drop Zone mismatch".into());
        }
        if let Some(old) = self
            .player_drop_effects
            .get(intent.world_effect_id())
            .cloned()
        {
            if old.intent != *intent {
                return Err("committed player-drop immutable intent mismatch".into());
            }
            // Terminal state is monotonic. A delayed Live receipt cannot revive
            // an effect consumed by a later authoritative pickup.
            if !matches!(old.state, PlayerDropCustodyState::Live { .. }) {
                let terminal = match proof.state() {
                    PlayerDropEffectState::Consumed => PlayerDropCustodyState::Consumed,
                    PlayerDropEffectState::Expired
                        if old.state != PlayerDropCustodyState::Consumed =>
                    {
                        PlayerDropCustodyState::Expired
                    }
                    _ => old.state.clone(),
                };
                let replay = terminal == old.state;
                self.player_drop_effects
                    .get_mut(intent.world_effect_id())
                    .expect("retained effect")
                    .state = terminal;
                return Ok(PlayerDropApplyResult {
                    object_id: None,
                    replay,
                    outbounds: Vec::new(),
                });
            }
            if proof.state() == PlayerDropEffectState::Live && now_ms < intent.expires_at_unix_ms()
            {
                return Ok(PlayerDropApplyResult {
                    object_id: match old.state {
                        PlayerDropCustodyState::Live { object_id, .. } => Some(object_id),
                        _ => None,
                    },
                    replay: true,
                    outbounds: Vec::new(),
                });
            }
        }
        if let Some(slot) = self.player_drop_reservations.get(intent.world_effect_id()) {
            let kind = match intent.outcome() {
                PlayerDropOutcome::Gold { .. } => PlayerDropPlacementKind::Gold,
                _ => PlayerDropPlacementKind::Item,
            };
            if &slot.position != intent.position() || slot.kind != kind {
                return Err("committed player-drop frozen placement mismatch".into());
            }
        }
        let mut outbounds = self.remove_player_drop_presentation(intent.world_effect_id());
        self.player_drop_reservations
            .remove(intent.world_effect_id());
        let terminal = match proof.state() {
            PlayerDropEffectState::Consumed => Some(PlayerDropCustodyState::Consumed),
            PlayerDropEffectState::Expired => Some(PlayerDropCustodyState::Expired),
            PlayerDropEffectState::Live if now_ms >= intent.expires_at_unix_ms() => {
                Some(PlayerDropCustodyState::Expired)
            }
            PlayerDropEffectState::Live
                if matches!(intent.outcome(), PlayerDropOutcome::DestroyedItem { .. }) =>
            {
                Some(PlayerDropCustodyState::Destroyed)
            }
            _ => None,
        };
        if let Some(state) = terminal {
            self.player_drop_effects.insert(
                intent.world_effect_id().to_owned(),
                PlayerDropEffectRecord {
                    intent: intent.clone(),
                    state,
                },
            );
            return Ok(PlayerDropApplyResult {
                object_id: None,
                replay: false,
                outbounds,
            });
        }
        let object_id = self.fresh_player_drop_object_id()?;
        let drop = self.player_drop_snapshot_from_intent(intent, object_id)?;
        let mut stored = self
            .new_zone_ground_drop(drop.clone(), None)
            .ok_or("player-drop generation exhausted")?;
        stored.player_drop_effect = Some(PlayerDropEffectIdentity::from_intent(intent));
        let drop_generation = stored.drop_generation;
        self.ground_drops.insert(object_id, stored);
        self.removed_object_ids.remove(&object_id);
        self.apply_zone_object_packets(&[committed_player_drop_spawn_packet(&drop)], now_ms);
        self.objects
            .get_mut(&object_id)
            .ok_or("committed player-drop object missing")?
            .expires_at_ms = Some(intent.expires_at_unix_ms());
        self.player_drop_effects.insert(
            intent.world_effect_id().to_owned(),
            PlayerDropEffectRecord {
                intent: intent.clone(),
                state: PlayerDropCustodyState::Live {
                    object_id,
                    drop_generation,
                },
            },
        );
        outbounds.extend(self.diff_all_zone_object_visibility());
        Ok(PlayerDropApplyResult {
            object_id: Some(object_id),
            replay: false,
            outbounds,
        })
    }

    /// Quiesced recovery replaces all snapshot-derived ordinary drop authority.
    /// The caller supplies the complete canonical source read, including Applied
    /// live effects and terminal effects. Old claims and reservations are revoked.
    pub fn reconcile_committed_player_drops(
        &mut self,
        proofs: &[PlayerDropCommittedEffect],
        now_ms: u64,
    ) -> Result<Vec<ZoneOutbound>, String> {
        let mut seen = BTreeSet::new();
        for proof in proofs {
            proof.intent().validate_integrity()?;
            if proof.intent().zone() != &self.key || !seen.insert(proof.intent().world_effect_id())
            {
                return Err("invalid player-drop recovery set".into());
            }
        }
        let ids: Vec<_> = self.player_drop_effects.keys().cloned().collect();
        let mut outbounds = Vec::new();
        for id in ids {
            outbounds.extend(self.remove_player_drop_presentation(&id));
        }
        self.player_drop_effects.clear();
        self.player_drop_reservations.clear();
        for proof in proofs {
            outbounds.extend(self.apply_committed_player_drop(proof, now_ms)?.outbounds);
        }
        Ok(outbounds)
    }

    fn fresh_player_drop_object_id(&mut self) -> Result<u32, String> {
        loop {
            let id = self.next_object_id.max(1);
            self.next_object_id = id.checked_add(1).ok_or("player-drop object ID exhausted")?;
            if !self.object_id_in_use(id)
                && !self.removed_object_ids.contains(&id)
                && !self.detached_ground_drop_custody.contains_key(&id)
            {
                return Ok(id);
            }
        }
    }

    fn player_drop_snapshot_from_intent(
        &self,
        intent: &PlayerDropWorldIntent,
        object_id: u32,
    ) -> Result<GroundDropSnapshot, String> {
        let (name, name_colour_argb, icon, quantity, loot) = match intent.outcome() {
            PlayerDropOutcome::Gold { amount } => (
                "Gold".to_string(),
                -1,
                0,
                *amount,
                GroundDropLootSnapshot::Gold { amount: *amount },
            ),
            PlayerDropOutcome::Item {
                quantity,
                loot:
                    loot @ GroundDropLootSnapshot::InventoryItem {
                        key,
                        name,
                        added_attack,
                        added_defence,
                        added_stats,
                        socket_slots,
                        ..
                    },
            } => (
                name.clone(),
                crate::runtime::drops::crystal_item_name_colour_argb_for_drop(
                    key,
                    *added_attack,
                    *added_defence,
                    added_stats,
                    *socket_slots,
                ),
                crate::runtime::items::item_icon_for_key(key),
                *quantity,
                loot.clone(),
            ),
            _ => return Err("destroyed player-drop has no world presentation".into()),
        };
        Ok(GroundDropSnapshot {
            object_id,
            name,
            name_colour_argb,
            icon,
            x: intent.position().x,
            y: intent.position().y,
            quantity,
            source_monster: String::new(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot,
        })
    }

    fn remove_player_drop_presentation(&mut self, id: &str) -> Vec<ZoneOutbound> {
        let object_id = match self.player_drop_effects.get(id).map(|r| &r.state) {
            Some(PlayerDropCustodyState::Live { object_id, .. }) => *object_id,
            _ => return Vec::new(),
        };
        self.ground_drops.remove(&object_id);
        self.claimed_ground_drops.remove(&object_id);
        self.detached_ground_drop_custody.remove(&object_id);
        self.remove_ground_drop_object_for_claim(object_id)
    }

    pub(super) fn ground_drop_ticket_economic_key(&self, ticket: &GroundDropClaimTicket) -> String {
        ticket
            .player_drop_effect
            .as_ref()
            .map(|effect| effect.pickup_key())
            .unwrap_or_else(|| {
                canonical_ground_drop_claim_idempotency_key(
                    &self.key,
                    ticket.object_id,
                    ticket.drop_generation,
                    &ticket.payload_digest,
                )
            })
    }

    pub(super) fn player_drop_ticket_authority_is_current(
        &self,
        ticket: &GroundDropClaimTicket,
    ) -> bool {
        let Some(identity) = &ticket.player_drop_effect else {
            return ticket.player_drop_claimant.is_none();
        };
        let Some(claimant) = ticket
            .player_drop_claimant
            .as_ref()
            .and_then(|value| serde_json::from_str::<OnlineOwner>(value).ok())
        else {
            return false;
        };
        if claimant.session_id != ticket.session_id
            || claimant.namespace.len() != 32
            || claimant.epoch == 0
        {
            return false;
        }
        self.player_drop_effects
            .get(identity.world_effect_id())
            .is_some_and(|r| {
                identity == &PlayerDropEffectIdentity::from_intent(&r.intent)
                    && self
                        .player_drop_snapshot_from_intent(&r.intent, ticket.object_id)
                        .is_ok_and(|drop| drop == ticket.drop)
                    && r.state
                        == (PlayerDropCustodyState::Live {
                            object_id: ticket.object_id,
                            drop_generation: ticket.drop_generation,
                        })
            })
    }
    pub(super) fn mark_player_drop_consumed(&mut self, ticket: &GroundDropClaimTicket) {
        if let Some(effect) = &ticket.player_drop_effect {
            if let Some(record) = self.player_drop_effects.get_mut(effect.world_effect_id()) {
                record.state = PlayerDropCustodyState::Consumed;
            }
        }
    }
    pub(super) fn mark_player_drop_expired_object(&mut self, object_id: u32) {
        for record in self.player_drop_effects.values_mut() {
            if matches!(record.state, PlayerDropCustodyState::Live { object_id: id, .. } if id == object_id)
            {
                record.state = PlayerDropCustodyState::Expired;
            }
        }
    }
    pub(super) fn expire_player_drop_effects(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .player_drop_effects
            .iter()
            .filter(|(_, r)| {
                matches!(r.state, PlayerDropCustodyState::Live { .. })
                    && now_ms >= r.intent.expires_at_unix_ms()
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut outbounds = Vec::new();
        for id in ids {
            outbounds.extend(self.remove_player_drop_presentation(&id));
            if let Some(record) = self.player_drop_effects.get_mut(&id) {
                record.state = PlayerDropCustodyState::Expired;
            }
        }
        outbounds
    }

    pub(super) fn validate_player_drop_custody(&self) -> Result<(), String> {
        let mut active = BTreeSet::new();
        for (id, record) in &self.player_drop_effects {
            record.intent.validate_integrity()?;
            if record.intent.world_effect_id() != id || record.intent.zone() != &self.key {
                return Err("invalid player-drop custody identity".into());
            }
            if let PlayerDropCustodyState::Live {
                object_id,
                drop_generation,
            } = record.state
            {
                if !active.insert(object_id) {
                    return Err("duplicate player-drop presentation identity".into());
                }
                let identity = PlayerDropEffectIdentity::from_intent(&record.intent);
                let expected = self.player_drop_snapshot_from_intent(&record.intent, object_id)?;
                if let Some(stored) = self.ground_drops.get(&object_id) {
                    if stored.player_drop_effect.as_ref() != Some(&identity)
                        || stored.drop != expected
                        || stored.drop_generation != drop_generation
                        || self.objects.get(&object_id).and_then(|o| o.expires_at_ms)
                            != Some(record.intent.expires_at_unix_ms())
                    {
                        return Err("invalid live player-drop custody".into());
                    }
                } else if let Some(claim) = self.claimed_ground_drops.get(&object_id) {
                    let ticket = claim
                        .ticket
                        .as_ref()
                        .ok_or("player-drop claim missing ticket")?;
                    if ticket.player_drop_effect.as_ref() != Some(&identity)
                        || ticket.drop != expected
                        || ticket.drop_generation != drop_generation
                    {
                        return Err("invalid claimed player-drop custody".into());
                    }
                } else if let Some(custody) = self.detached_ground_drop_custody.get(&object_id) {
                    if custody.player_drop_effect.as_ref() != Some(&identity)
                        || custody.drop_generation != drop_generation
                        || custody.object_expires_at_ms != Some(record.intent.expires_at_unix_ms())
                    {
                        return Err("invalid detached player-drop custody".into());
                    }
                } else {
                    return Err("player-drop custody has no presentation or claim".into());
                }
            }
        }
        for stored in self.ground_drops.values() {
            if let Some(identity) = &stored.player_drop_effect {
                if !active.contains(&stored.drop.object_id)
                    || !self
                        .player_drop_effects
                        .contains_key(identity.world_effect_id())
                {
                    return Err("orphan player-drop metadata".into());
                }
            }
        }
        for (id, slot) in &self.player_drop_reservations {
            if id != slot.world_effect_id()
                || slot.zone != self.key
                || slot.reserved_at_unix_ms == 0
                || zone_tile_distance(&slot.origin, &slot.position) > slot.kind.range()
                || self.player_drop_effects.contains_key(id)
            {
                return Err("invalid player-drop reservation".into());
            }
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "player_drop_custody_tests.rs"]
mod tests;

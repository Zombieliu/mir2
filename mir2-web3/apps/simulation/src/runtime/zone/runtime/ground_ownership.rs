//! Crystal-owned monster loot clocks. A countdown is only a projection.
use super::*;
use crate::runtime::zone::online_identity::OnlineOwner;
use crate::runtime::zone::types::{NativeGroundOwner, ZoneGroundDropCustody};

pub(super) const ORIGINAL_MONSTER_ITEM_OWNER_MS: u64 = 60_000;

impl ZoneRuntime {
    pub(crate) fn active_ground_drop_snapshot(&self, object_id: u32) -> Option<GroundDropSnapshot> {
        Some(self.ground_drops.get(&object_id)?.drop.clone())
    }
    pub(crate) fn ground_drop_absolute_clocks(
        &self,
        object_id: u32,
    ) -> Option<(Option<u64>, Option<u64>)> {
        if let Some(drop) = self.ground_drops.get(&object_id) {
            return Some((
                drop.owner_expires_at_ms,
                self.objects.get(&object_id).and_then(|o| o.expires_at_ms),
            ));
        }
        self.claimed_ground_drops
            .get(&object_id)
            .and_then(|c| c.custody.as_ref())
            .or_else(|| self.detached_ground_drop_custody.get(&object_id))
            .map(|c| (c.owner_expires_at_ms, c.object_expires_at_ms))
    }
    pub(super) fn validate_native_ground_custody(&self) -> Result<(), String> {
        let valid_native = |native: &NativeGroundOwner| {
            native.source == self.key
                && native.owner.epoch > 0
                && native.owner.namespace.len() == 32
                && native.expires_at_ms
                    == native
                        .born_at_ms
                        .saturating_add(ORIGINAL_MONSTER_ITEM_OWNER_MS)
        };
        for (id, stored) in &self.ground_drops {
            if stored.native_owner.as_ref().is_some_and(|native| {
                !valid_native(native)
                    || stored.drop.owner_object_id != Some(native.owner.object_id)
                    || stored
                        .owner_expires_at_ms
                        .is_some_and(|t| t != native.expires_at_ms)
            }) {
                return Err(format!("invalid native ground ownership {id}"));
            }
        }
        for (id, claim) in &self.claimed_ground_drops {
            if claim.custody.as_ref().is_some_and(|c| {
                claim
                    .ticket
                    .as_ref()
                    .is_none_or(|t| !c.matches(&self.key, t))
                    || c.native_owner.as_ref().is_some_and(|n| !valid_native(n))
            }) {
                return Err(format!("invalid ground claim custody {id}"));
            }
        }
        for (id, custody) in &self.detached_ground_drop_custody {
            if custody.source != self.key
                || *id != custody.object_id
                || !self.removed_object_ids.contains(id)
                || self.ground_drops.contains_key(id)
                || self.claimed_ground_drops.contains_key(id)
                || custody.drop_generation == 0
                || custody.payload_digest.is_empty()
                || custody
                    .native_owner
                    .as_ref()
                    .is_some_and(|n| !valid_native(n))
            {
                return Err(format!("invalid detached ground custody {id}"));
            }
        }
        Ok(())
    }
    pub(crate) fn native_ground_drop_deadline(&self, object_id: u32) -> Option<Option<u64>> {
        self.ground_drops
            .get(&object_id)
            .filter(|drop| drop.native_owner.is_some())
            .map(|drop| drop.owner_expires_at_ms)
    }

    pub(super) fn bind_native_ground_owner(
        &self,
        owner_object_id: u32,
        born_at_ms: u64,
    ) -> Option<NativeGroundOwner> {
        let owner = self
            .online_presence
            .values()
            .find(|p| p.owner.object_id == owner_object_id)?
            .owner
            .clone();
        Some(NativeGroundOwner {
            owner,
            source: self.key.clone(),
            born_at_ms,
            expires_at_ms: born_at_ms.saturating_add(ORIGINAL_MONSTER_ITEM_OWNER_MS),
        })
    }

    pub(super) fn online_owner_current(&self, owner: &OnlineOwner) -> bool {
        self.online_presence
            .get(&owner.session_id)
            .is_some_and(|p| p.owner == *owner)
    }

    pub(super) fn capture_ground_drop_custody(
        &self,
        stored: &ZoneGroundDrop,
    ) -> ZoneGroundDropCustody {
        ZoneGroundDropCustody {
            source: self.key.clone(),
            object_id: stored.drop.object_id,
            drop_generation: stored.drop_generation,
            payload_digest: stored.payload_digest.clone(),
            owner_expires_at_ms: stored.owner_expires_at_ms,
            native_owner: stored.native_owner.clone(),
            object_expires_at_ms: self
                .objects
                .get(&stored.drop.object_id)
                .and_then(|o| o.expires_at_ms),
        }
    }

    pub(super) fn restore_ground_custody(
        &mut self,
        ticket: &GroundDropClaimTicket,
        custody: Option<ZoneGroundDropCustody>,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        if custody
            .as_ref()
            .is_some_and(|c| !c.matches(&self.key, ticket))
        {
            return None;
        }
        if custody
            .as_ref()
            .is_some_and(|c| c.object_expires_at_ms.is_some_and(|t| now_ms > t))
        {
            // The held item keeps its original TTL. A rejected claim cannot
            // resurrect a ground object that would already have despawned.
            self.removed_object_ids.insert(ticket.object_id);
            return Some(Vec::new());
        }
        let native_owner = custody.as_ref().and_then(|c| c.native_owner.clone());
        let deadline = custody.as_ref().and_then(|c| c.owner_expires_at_ms);
        let mut drop = ticket.drop.clone();
        let still_protected = native_owner
            .as_ref()
            .map(|o| self.online_owner_current(&o.owner) && now_ms <= o.expires_at_ms)
            .unwrap_or_else(|| deadline.is_some_and(|t| now_ms < t));
        if !still_protected {
            drop.ownership_remaining_ticks = None;
        }
        self.ground_drops.insert(
            ticket.object_id,
            ZoneGroundDrop {
                drop: drop.clone(),
                drop_generation: ticket.drop_generation,
                payload_digest: ticket.payload_digest.clone(),
                owner_expires_at_ms: still_protected.then_some(deadline).flatten(),
                native_owner,
            },
        );
        self.apply_zone_object_packets(&[ground_drop_spawn_packet(&drop)], now_ms);
        if let Some(custody) = custody {
            if let Some(object) = self.objects.get_mut(&ticket.object_id) {
                object.expires_at_ms = custody.object_expires_at_ms;
            }
        }
        Some(self.diff_all_zone_object_visibility())
    }
}

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    PendingNativeGroundSpellAction, PendingNativeMonsterHit, PendingNativePlayerHeal,
    PendingNativePlayerHit, PendingNativeProjectile, PendingNativeSummon, ZoneHazardState,
    ZoneObjectDeadState, ZoneRuntime,
};
use crate::runtime::zone::types::{
    SessionId, ZoneGroundDrop, ZoneGroundDropClaim, ZoneKey, ZoneNativeMonster, ZoneObject,
    ZonePlayer,
};
use crate::runtime::zone::ZoneCollision;

const CANONICAL_ZONE_STATE_VERSION: u32 = 1;
const CANONICAL_ZONE_STATE_DOMAIN: &[u8] = b"obelisk.mir2.zone-state.v1\0";

/// The authoritative portion of a zone. Collision data is selected by the
/// signed game module and `ZoneKey`; occupancy, AOI grids, and the ECS mirror
/// are derived indexes rebuilt from these fields and therefore are not hashed.
#[derive(Serialize)]
struct CanonicalZoneState<'a> {
    version: u32,
    key: &'a ZoneKey,
    collision: &'a ZoneCollision,
    players: &'a BTreeMap<SessionId, ZonePlayer>,
    objects: &'a BTreeMap<u32, ZoneObject>,
    dead_object_ids: &'a BTreeMap<u32, ZoneObjectDeadState>,
    revived_object_ids: &'a BTreeSet<u32>,
    removed_object_ids: &'a BTreeSet<u32>,
    harvested_object_ids: &'a BTreeSet<u32>,
    native_monsters: &'a BTreeMap<u32, ZoneNativeMonster>,
    pending_native_hits: &'a [PendingNativeMonsterHit],
    pending_native_projectiles: &'a [PendingNativeProjectile],
    pending_native_player_hits: &'a [PendingNativePlayerHit],
    pending_native_player_heals: &'a [PendingNativePlayerHeal],
    pending_native_summons: &'a [PendingNativeSummon],
    pending_native_ground_spells: &'a [PendingNativeGroundSpellAction],
    ground_drops: &'a BTreeMap<u32, ZoneGroundDrop>,
    claimed_ground_drops: &'a BTreeMap<u32, ZoneGroundDropClaim>,
    open_doors: &'a BTreeMap<u8, u64>,
    hazard: &'a ZoneHazardState,
    next_object_id: u32,
}

impl ZoneRuntime {
    /// Canonical SHA-256 commitment to the complete authoritative zone state.
    /// BTree collections and struct field order make the JSON byte stream
    /// deterministic for a fixed signed game-module version.
    pub fn canonical_state_root(&self) -> Result<String, String> {
        let state = CanonicalZoneState {
            version: CANONICAL_ZONE_STATE_VERSION,
            key: &self.key,
            collision: &self.collision,
            players: &self.players,
            objects: &self.objects,
            dead_object_ids: &self.dead_object_ids,
            revived_object_ids: &self.revived_object_ids,
            removed_object_ids: &self.removed_object_ids,
            harvested_object_ids: &self.harvested_object_ids,
            native_monsters: &self.native_monsters,
            pending_native_hits: &self.pending_native_hits,
            pending_native_projectiles: &self.pending_native_projectiles,
            pending_native_player_hits: &self.pending_native_player_hits,
            pending_native_player_heals: &self.pending_native_player_heals,
            pending_native_summons: &self.pending_native_summons,
            pending_native_ground_spells: &self.pending_native_ground_spells,
            ground_drops: &self.ground_drops,
            claimed_ground_drops: &self.claimed_ground_drops,
            open_doors: &self.open_doors,
            hazard: &self.hazard,
            next_object_id: self.next_object_id,
        };
        let bytes = serde_json::to_vec(&state)
            .map_err(|error| format!("failed to serialize canonical zone state: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(CANONICAL_ZONE_STATE_DOMAIN);
        hasher.update(bytes);
        Ok(hex_lower(&hasher.finalize()))
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

use std::collections::{BTreeMap, BTreeSet};

use mir2_protocol::{
    decode_server_packet, encode_server_packet, ObjectHealthInfo, ObjectManaInfo, Point,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    canonical_ground_drop_claim_idempotency_key, canonical_ground_drop_payload_digest,
    PendingNativeGroundSpellAction, PendingNativeMonsterHit, PendingNativePlayerHeal,
    PendingNativePlayerHit, PendingNativeProjectile, PendingNativeSummon, ZoneHazardState,
    ZoneObjectDeadState, ZoneRuntime,
};
use crate::runtime::zone::types::{
    GroundDropClaimTicket, SessionId, ZoneGroundDrop, ZoneGroundDropClaim, ZoneKey,
    ZoneNativeMonster, ZoneObject, ZonePlayer, ZonePlayerBuff,
};
use crate::runtime::zone::ZoneCollision;

const LEGACY_CANONICAL_ZONE_STATE_VERSION: u32 = 1;
const LEGACY_CANONICAL_ZONE_STATE_DOMAIN: &[u8] = b"obelisk.mir2.zone-state.v1\0";
const CANONICAL_ZONE_STATE_VERSION: u32 = 2;
const CANONICAL_ZONE_STATE_DOMAIN: &[u8] = b"obelisk.mir2.zone-state.v2\0";
const LEGACY_ZONE_RUNTIME_CHECKPOINT_VERSION: u32 = 1;
const ZONE_RUNTIME_CHECKPOINT_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateRootPolicy {
    Strict,
    ReanchorVerifiedWorldCheckpoint,
}

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
    next_ground_drop_generation: u64,
    next_ground_drop_claim_id: u64,
    open_doors: &'a BTreeMap<u8, u64>,
    hazard: &'a ZoneHazardState,
    next_object_id: u32,
}

#[derive(Serialize)]
struct LegacyCanonicalZoneState<'a> {
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
    ground_drops: BTreeMap<u32, LegacyZoneGroundDrop<'a>>,
    claimed_ground_drops: BTreeMap<u32, LegacyZoneGroundDropClaim<'a>>,
    open_doors: &'a BTreeMap<u8, u64>,
    hazard: &'a ZoneHazardState,
    next_object_id: u32,
}

#[derive(Serialize)]
struct LegacyZoneGroundDrop<'a> {
    drop: &'a crate::config::GroundDropSnapshot,
    owner_expires_at_ms: Option<u64>,
}

#[derive(Serialize)]
struct LegacyZoneGroundDropClaim<'a> {
    session_id: &'a SessionId,
    drop: &'a crate::config::GroundDropSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZoneRuntimeCheckpoint {
    version: u32,
    state_root: String,
    key: ZoneKey,
    players: BTreeMap<SessionId, ZonePlayer>,
    objects: BTreeMap<u32, ZoneObjectCheckpoint>,
    dead_object_ids: BTreeMap<u32, ZoneObjectDeadState>,
    revived_object_ids: BTreeSet<u32>,
    removed_object_ids: BTreeSet<u32>,
    harvested_object_ids: BTreeSet<u32>,
    native_monsters: BTreeMap<u32, ZoneNativeMonster>,
    pending_native_hits: Vec<PendingNativeMonsterHit>,
    pending_native_projectiles: Vec<PendingNativeProjectile>,
    pending_native_player_hits: Vec<PendingNativePlayerHit>,
    pending_native_player_heals: Vec<PendingNativePlayerHeal>,
    pending_native_summons: Vec<PendingNativeSummon>,
    pending_native_ground_spells: Vec<PendingNativeGroundSpellAction>,
    ground_drops: BTreeMap<u32, ZoneGroundDrop>,
    claimed_ground_drops: BTreeMap<u32, ZoneGroundDropClaim>,
    #[serde(default)]
    next_ground_drop_generation: u64,
    #[serde(default)]
    next_ground_drop_claim_id: u64,
    open_doors: BTreeMap<u8, u64>,
    hazard: ZoneHazardState,
    next_object_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZoneObjectCheckpoint {
    object_id: u32,
    position: Point,
    packet_frame: Vec<u8>,
    health: Option<ObjectHealthInfo>,
    mana: Option<ObjectManaInfo>,
    expires_at_ms: Option<u64>,
    buffs: BTreeMap<u8, ZonePlayerBuff>,
}

impl ZoneObjectCheckpoint {
    fn capture(object: &ZoneObject) -> Result<Self, String> {
        Ok(Self {
            object_id: object.object_id,
            position: object.position.clone(),
            packet_frame: encode_server_packet(&object.packet)
                .map_err(|error| format!("failed to encode retained Zone packet: {error}"))?,
            health: object.health.clone(),
            mana: object.mana.clone(),
            expires_at_ms: object.expires_at_ms,
            buffs: object.buffs.clone(),
        })
    }

    fn restore(self) -> Result<ZoneObject, String> {
        Ok(ZoneObject {
            object_id: self.object_id,
            position: self.position,
            packet: decode_server_packet(&self.packet_frame)
                .map_err(|error| format!("failed to decode retained Zone packet: {error}"))?,
            health: self.health,
            mana: self.mana,
            expires_at_ms: self.expires_at_ms,
            buffs: self.buffs,
        })
    }
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
            next_ground_drop_generation: self.next_ground_drop_generation,
            next_ground_drop_claim_id: self.next_ground_drop_claim_id,
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

    fn legacy_canonical_state_root(&self) -> Result<String, String> {
        let ground_drops = self
            .ground_drops
            .iter()
            .map(|(object_id, stored)| {
                (
                    *object_id,
                    LegacyZoneGroundDrop {
                        drop: &stored.drop,
                        owner_expires_at_ms: stored.owner_expires_at_ms,
                    },
                )
            })
            .collect();
        let claimed_ground_drops = self
            .claimed_ground_drops
            .iter()
            .map(|(object_id, claim)| {
                (
                    *object_id,
                    LegacyZoneGroundDropClaim {
                        session_id: &claim.session_id,
                        drop: &claim.drop,
                    },
                )
            })
            .collect();
        let state = LegacyCanonicalZoneState {
            version: LEGACY_CANONICAL_ZONE_STATE_VERSION,
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
            ground_drops,
            claimed_ground_drops,
            open_doors: &self.open_doors,
            hazard: &self.hazard,
            next_object_id: self.next_object_id,
        };
        let bytes = serde_json::to_vec(&state)
            .map_err(|error| format!("failed to serialize legacy canonical zone state: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(LEGACY_CANONICAL_ZONE_STATE_DOMAIN);
        hasher.update(bytes);
        Ok(hex_lower(&hasher.finalize()))
    }

    fn hydrate_legacy_ground_drop_claim_authority(&mut self) -> Result<(), String> {
        // v1 state roots intentionally predate every field below. Once the
        // legacy root has verified, none of those uncommitted v2 fields can be
        // trusted: JSON can carry them even though the v1 root did not cover
        // them. Start from the v1 payload only, then rebuild the authority
        // deterministically in BTree/object-id order.
        self.next_ground_drop_generation = 0;
        self.next_ground_drop_claim_id = 0;
        for stored in self.ground_drops.values_mut() {
            stored.drop_generation = 0;
            stored.payload_digest.clear();
        }
        for claim in self.claimed_ground_drops.values_mut() {
            claim.ticket = None;
        }

        let mut next_generation = 1_u64;
        let mut next_claim_id = 1_u64;
        for stored in self.ground_drops.values_mut() {
            stored.drop_generation = next_generation;
            next_generation = next_generation
                .checked_add(1)
                .ok_or_else(|| "legacy ground drop generation space exhausted".to_string())?;
            stored.payload_digest = canonical_ground_drop_payload_digest(&stored.drop);
        }
        let key = self.key.clone();
        for (object_id, claim) in &mut self.claimed_ground_drops {
            let drop_generation = next_generation;
            next_generation = next_generation
                .checked_add(1)
                .ok_or_else(|| "legacy ground drop generation space exhausted".to_string())?;
            let claim_id = next_claim_id;
            next_claim_id = next_claim_id
                .checked_add(1)
                .ok_or_else(|| "legacy ground drop claim-id space exhausted".to_string())?;
            let payload_digest = canonical_ground_drop_payload_digest(&claim.drop);
            claim.ticket = Some(GroundDropClaimTicket {
                claim_id,
                object_id: *object_id,
                drop_generation,
                payload_digest: payload_digest.clone(),
                idempotency_key: canonical_ground_drop_claim_idempotency_key(
                    &key,
                    *object_id,
                    drop_generation,
                    claim_id,
                    &payload_digest,
                ),
                session_id: claim.session_id.clone(),
                owner_object_id: claim.drop.owner_object_id,
                drop: claim.drop.clone(),
            });
        }
        self.next_ground_drop_generation = next_generation.max(1);
        self.next_ground_drop_claim_id = next_claim_id.max(1);
        Ok(())
    }
    fn validate_ground_drop_claim_authority(&self) -> Result<(), String> {
        let mut max_generation = 0_u64;
        let mut max_claim_id = 0_u64;
        for (object_id, stored) in &self.ground_drops {
            if stored.drop.object_id != *object_id
                || stored.drop_generation == 0
                || stored.payload_digest != canonical_ground_drop_payload_digest(&stored.drop)
            {
                return Err(format!("invalid authoritative ground drop {object_id}"));
            }
            max_generation = max_generation.max(stored.drop_generation);
        }
        for (object_id, claim) in &self.claimed_ground_drops {
            let Some(ticket) = claim.ticket.as_ref() else {
                return Err(format!(
                    "claimed ground drop {object_id} is missing its ticket"
                ));
            };
            if ticket.object_id != *object_id
                || !self.ground_drop_claim_ticket_matches(claim, &claim.session_id, ticket)
            {
                return Err(format!("invalid claimed ground drop ticket {object_id}"));
            }
            max_generation = max_generation.max(ticket.drop_generation);
            max_claim_id = max_claim_id.max(ticket.claim_id);
        }
        if self.next_ground_drop_generation <= max_generation
            || self.next_ground_drop_claim_id <= max_claim_id
        {
            return Err(
                "ground-drop authority counters do not exceed persisted values".to_string(),
            );
        }
        Ok(())
    }

    pub fn checkpoint_bytes(&self) -> Result<Vec<u8>, String> {
        let checkpoint = ZoneRuntimeCheckpoint {
            version: ZONE_RUNTIME_CHECKPOINT_VERSION,
            state_root: self.canonical_state_root()?,
            key: self.key.clone(),
            players: self.players.clone(),
            objects: self
                .objects
                .iter()
                .map(|(object_id, object)| Ok((*object_id, ZoneObjectCheckpoint::capture(object)?)))
                .collect::<Result<BTreeMap<_, _>, String>>()?,
            dead_object_ids: self.dead_object_ids.clone(),
            revived_object_ids: self.revived_object_ids.clone(),
            removed_object_ids: self.removed_object_ids.clone(),
            harvested_object_ids: self.harvested_object_ids.clone(),
            native_monsters: self.native_monsters.clone(),
            pending_native_hits: self.pending_native_hits.clone(),
            pending_native_projectiles: self.pending_native_projectiles.clone(),
            pending_native_player_hits: self.pending_native_player_hits.clone(),
            pending_native_player_heals: self.pending_native_player_heals.clone(),
            pending_native_summons: self.pending_native_summons.clone(),
            pending_native_ground_spells: self.pending_native_ground_spells.clone(),
            ground_drops: self.ground_drops.clone(),
            claimed_ground_drops: self.claimed_ground_drops.clone(),
            next_ground_drop_generation: self.next_ground_drop_generation,
            next_ground_drop_claim_id: self.next_ground_drop_claim_id,
            open_doors: self.open_doors.clone(),
            hazard: self.hazard.clone(),
            next_object_id: self.next_object_id,
        };
        serde_json::to_vec(&checkpoint)
            .map_err(|error| format!("failed to encode zone runtime checkpoint: {error}"))
    }

    pub fn restore_checkpoint(bytes: &[u8]) -> Result<Self, String> {
        Self::restore_checkpoint_with_policy(bytes, StateRootPolicy::Strict)
    }

    /// Restore bytes already covered by a verified, higher-level World
    /// Director checkpoint commitment and re-anchor them to the current signed
    /// game module. The canonical Zone root includes collision data, which is
    /// intentionally not duplicated in the checkpoint and can change between
    /// releases; a strict old root therefore cannot survive a module upgrade.
    pub(crate) fn restore_verified_world_checkpoint(bytes: &[u8]) -> Result<Self, String> {
        Self::restore_checkpoint_with_policy(
            bytes,
            StateRootPolicy::ReanchorVerifiedWorldCheckpoint,
        )
    }

    fn restore_checkpoint_with_policy(
        bytes: &[u8],
        state_root_policy: StateRootPolicy,
    ) -> Result<Self, String> {
        let checkpoint: ZoneRuntimeCheckpoint = serde_json::from_slice(bytes)
            .map_err(|error| format!("failed to decode zone runtime checkpoint: {error}"))?;
        if checkpoint.version != LEGACY_ZONE_RUNTIME_CHECKPOINT_VERSION
            && checkpoint.version != ZONE_RUNTIME_CHECKPOINT_VERSION
        {
            return Err(format!(
                "unsupported zone runtime checkpoint version {}, expected {} or {}",
                checkpoint.version,
                LEGACY_ZONE_RUNTIME_CHECKPOINT_VERSION,
                ZONE_RUNTIME_CHECKPOINT_VERSION
            ));
        }
        let checkpoint_version = checkpoint.version;
        let expected_state_root = checkpoint.state_root.clone();
        for (session_id, player) in &checkpoint.players {
            if session_id != &player.session_id {
                return Err(format!(
                    "zone runtime checkpoint player key {} does not match embedded session {}",
                    session_id.as_str(),
                    player.session_id.as_str()
                ));
            }
        }
        for (object_id, object) in &checkpoint.objects {
            if object_id != &object.object_id {
                return Err(format!(
                    "zone runtime checkpoint object key {object_id} does not match embedded id {}",
                    object.object_id
                ));
            }
        }

        let mut runtime = Self::new(checkpoint.key);
        runtime.players = checkpoint.players;
        runtime.objects = checkpoint
            .objects
            .into_iter()
            .map(|(object_id, object)| Ok((object_id, object.restore()?)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        runtime.dead_object_ids = checkpoint.dead_object_ids;
        runtime.revived_object_ids = checkpoint.revived_object_ids;
        runtime.removed_object_ids = checkpoint.removed_object_ids;
        runtime.harvested_object_ids = checkpoint.harvested_object_ids;
        runtime.native_monsters = checkpoint.native_monsters;
        for monster in runtime.native_monsters.values_mut() {
            // Legacy checkpoints had only the AI-derived boolean. Without the
            // explicit authoritative disposition they must remain untargetable
            // and must not target players.
            monster.hostile_to_player =
                monster.disposition == Some(crate::config::WorldEntityDisposition::Hostile);
        }
        runtime.pending_native_hits = checkpoint.pending_native_hits;
        runtime.pending_native_projectiles = checkpoint.pending_native_projectiles;
        runtime.pending_native_player_hits = checkpoint.pending_native_player_hits;
        runtime.pending_native_player_heals = checkpoint.pending_native_player_heals;
        runtime.pending_native_summons = checkpoint.pending_native_summons;
        runtime.pending_native_ground_spells = checkpoint.pending_native_ground_spells;
        runtime.ground_drops = checkpoint.ground_drops;
        runtime.claimed_ground_drops = checkpoint.claimed_ground_drops;
        runtime.next_ground_drop_generation = checkpoint.next_ground_drop_generation;
        runtime.next_ground_drop_claim_id = checkpoint.next_ground_drop_claim_id;
        runtime.open_doors = checkpoint.open_doors;
        runtime.hazard = checkpoint.hazard;
        runtime.next_object_id = checkpoint.next_object_id;

        for door_index in runtime.open_doors.keys().copied().collect::<Vec<_>>() {
            runtime.collision.open_door(door_index);
        }
        for (session_id, player) in &runtime.players {
            let tile = (player.position.x, player.position.y);
            if runtime.occupancy.insert(tile, session_id.clone()).is_some() {
                return Err(format!(
                    "zone runtime checkpoint has multiple players at {},{}",
                    tile.0, tile.1
                ));
            }
            runtime
                .player_grid
                .insert(session_id.clone(), &player.position);
            runtime
                .ecs
                .upsert_player(session_id, player.object_id, &player.position);
        }
        for (object_id, object) in &runtime.objects {
            runtime.object_grid.insert(*object_id, &object.position);
        }

        let restored_root = if checkpoint_version == LEGACY_ZONE_RUNTIME_CHECKPOINT_VERSION {
            runtime.legacy_canonical_state_root()?
        } else {
            runtime.canonical_state_root()?
        };
        if restored_root != expected_state_root && state_root_policy == StateRootPolicy::Strict {
            return Err(format!(
                "zone runtime checkpoint state root mismatch: expected {}, got {restored_root}",
                expected_state_root
            ));
        }
        if checkpoint_version == LEGACY_ZONE_RUNTIME_CHECKPOINT_VERSION {
            runtime.hydrate_legacy_ground_drop_claim_authority()?;
        } else {
            runtime.validate_ground_drop_claim_authority()?;
        }
        Ok(runtime)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GroundDropLootSnapshot, GroundDropSnapshot};
    use crate::runtime::zone::types::{
        ZoneChatProfile, ZoneCommand, ZoneJoin, ZoneOutbound, ZonePlayerCombatStats,
    };
    use mir2_protocol::{MirClass, MirDirection, MirGender};

    fn checkpoint_gold_drop(object_id: u32) -> GroundDropSnapshot {
        GroundDropSnapshot {
            object_id,
            name: "Gold".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: 12,
            y: 34,
            quantity: 1,
            source_monster: "legacy-checkpoint".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: Some(20),
            loot: GroundDropLootSnapshot::Gold { amount: 25 },
        }
    }

    /// A real v1-shaped fixture: every authority field added by v2 is absent,
    /// and the root is calculated from the v1 canonical schema only.
    fn legacy_v1_ground_drop_fixture() -> serde_json::Value {
        let mut runtime = ZoneRuntime::new(ZoneKey::for_map("legacy-drop-map"));
        let active = checkpoint_gold_drop(101);
        let active_stored = runtime
            .new_zone_ground_drop(active.clone(), Some(6_000))
            .expect("ground-drop generation");
        runtime.ground_drops.insert(active.object_id, active_stored);

        let claimed = checkpoint_gold_drop(102);
        runtime.claimed_ground_drops.insert(
            claimed.object_id,
            ZoneGroundDropClaim {
                session_id: SessionId::new("legacy-claimer"),
                drop: claimed,
                ticket: None,
            },
        );
        let legacy_root = runtime
            .legacy_canonical_state_root()
            .expect("legacy fixture root");
        let mut fixture: serde_json::Value =
            serde_json::from_slice(&runtime.checkpoint_bytes().expect("checkpoint source bytes"))
                .expect("checkpoint JSON");
        fixture["version"] = serde_json::json!(LEGACY_ZONE_RUNTIME_CHECKPOINT_VERSION);
        fixture["state_root"] = serde_json::json!(legacy_root);
        fixture
            .as_object_mut()
            .expect("checkpoint object")
            .remove("next_ground_drop_generation");
        fixture
            .as_object_mut()
            .expect("checkpoint object")
            .remove("next_ground_drop_claim_id");
        for stored in fixture["ground_drops"]
            .as_object_mut()
            .expect("ground drops")
            .values_mut()
        {
            let stored = stored.as_object_mut().expect("ground drop object");
            stored.remove("drop_generation");
            stored.remove("payload_digest");
        }
        for claim in fixture["claimed_ground_drops"]
            .as_object_mut()
            .expect("claimed ground drops")
            .values_mut()
        {
            claim
                .as_object_mut()
                .expect("claimed ground drop object")
                .remove("ticket");
        }
        fixture
    }

    #[test]
    fn exact_ground_drop_identity_is_checkpointed_and_state_root_protected() {
        use crate::config::{GroundDropItemPayload, GroundDropLootSnapshot, GroundDropSnapshot};
        use mir2_protocol::{UserItem, UserItemExpireInfo, UserItemStat};

        let mut runtime = ZoneRuntime::new(ZoneKey::for_map("drop-identity-checkpoint"));
        let exact_item = GroundDropItemPayload {
            item: UserItem {
                unique_id: 8_888,
                item_index: 404,
                current_dura: 4_321,
                max_dura: 5_432,
                count: 1,
                soul_bound_id: 17,
                identified: false,
                cursed: true,
                slots: Vec::new(),
                gem_count: 0,
                added_stats: vec![UserItemStat { stat: 17, value: 9 }],
                awake_type: 2,
                awake_values: vec![3, 4],
                refined_value: 5,
                refine_added: 6,
                refine_success_chance: 77,
                wedding_ring: 23,
                expire_info: Some(UserItemExpireInfo {
                    expiry_binary_datetime: 123_456,
                }),
                rental_information: None,
                is_shop_item: true,
                sealed_info: None,
                gm_made: true,
            },
            uid_assigned: true,
        };
        let snapshot = GroundDropSnapshot {
            object_id: 91,
            name: "CopperRing".to_string(),
            name_colour_argb: -1,
            icon: 0,
            x: 12,
            y: 34,
            quantity: 1,
            source_monster: "checkpoint-test".to_string(),
            owner_object_id: None,
            ownership_remaining_ticks: None,
            loot: GroundDropLootSnapshot::InventoryItem {
                key: "crystal-item-404".to_string(),
                name: "CopperRing".to_string(),
                description: String::new(),
                weight: 1,
                durability_current: Some(4_321),
                durability_max: Some(5_432),
                added_attack: 0,
                added_defence: 0,
                added_stats: vec![UserItemStat { stat: 17, value: 9 }],
                cursed: true,
                socket_slots: 0,
                show_group_pickup: false,
                exact_item: Some(exact_item),
            },
        };
        let stored = runtime
            .new_zone_ground_drop(snapshot.clone(), None)
            .expect("ground-drop generation");
        runtime.ground_drops.insert(snapshot.object_id, stored);
        let mut claimed_snapshot = snapshot.clone();
        claimed_snapshot.object_id = 92;
        let claimed_stored = runtime
            .new_zone_ground_drop(claimed_snapshot.clone(), None)
            .expect("ground-drop generation");
        let claim_id = runtime
            .allocate_ground_drop_claim_id()
            .expect("ground-drop claim id");
        let claim_session_id = SessionId::new("drop-identity-claim");
        let ticket = GroundDropClaimTicket {
            claim_id,
            object_id: claimed_snapshot.object_id,
            drop_generation: claimed_stored.drop_generation,
            payload_digest: claimed_stored.payload_digest.clone(),
            idempotency_key: canonical_ground_drop_claim_idempotency_key(
                &runtime.key,
                claimed_snapshot.object_id,
                claimed_stored.drop_generation,
                claim_id,
                &claimed_stored.payload_digest,
            ),
            session_id: claim_session_id.clone(),
            owner_object_id: claimed_snapshot.owner_object_id,
            drop: claimed_snapshot.clone(),
        };
        runtime.claimed_ground_drops.insert(
            claimed_snapshot.object_id,
            ZoneGroundDropClaim {
                session_id: claim_session_id,
                drop: claimed_snapshot.clone(),
                ticket: Some(ticket),
            },
        );

        let bytes = runtime.checkpoint_bytes().expect("identity checkpoint");
        let restored = ZoneRuntime::restore_checkpoint(&bytes).expect("identity restore");
        assert_eq!(
            restored
                .ground_drops
                .get(&snapshot.object_id)
                .expect("restored ground drop")
                .drop,
            snapshot
        );
        assert_eq!(
            restored
                .claimed_ground_drops
                .get(&claimed_snapshot.object_id)
                .expect("restored claimed ground drop")
                .drop,
            claimed_snapshot
        );

        let mut checkpoint: ZoneRuntimeCheckpoint =
            serde_json::from_slice(&bytes).expect("checkpoint JSON");
        let GroundDropLootSnapshot::InventoryItem {
            exact_item: Some(exact_item),
            ..
        } = &mut checkpoint
            .claimed_ground_drops
            .get_mut(&92)
            .expect("checkpoint claimed ground drop")
            .drop
            .loot
        else {
            panic!("exact checkpoint payload");
        };
        exact_item.item.awake_type = exact_item.item.awake_type.saturating_add(1);
        let tampered = serde_json::to_vec(&checkpoint).expect("tampered identity checkpoint");
        let error = ZoneRuntime::restore_checkpoint(&tampered)
            .expect_err("identity tamper must fail state-root verification");
        assert!(error.contains("state root mismatch"), "{error}");
    }

    #[test]
    fn complete_zone_checkpoint_restores_authoritative_and_derived_state() {
        let session_id = SessionId::new("checkpoint-player");
        let mut runtime = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("checkpoint-map"),
            ZoneCollision::unbounded(),
        );
        runtime.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session_id.clone(),
            account_id: "checkpoint-account".to_string(),
            character_index: 7,
            object_id: 77,
            name: "Checkpoint".to_string(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 12,
            hp: 55,
            max_hp: 80,
            mp: 21,
            map_file_name: "checkpoint-map".to_string(),
            position: Point { x: 14, y: 27 },
            direction: MirDirection::DownRight,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        }));
        runtime.handle(ZoneCommand::SyncPlayerVitals {
            session_id: session_id.clone(),
            hp: 43,
            max_hp: 80,
            mp: 17,
        });
        runtime.handle(ZoneCommand::ConfigureHazards {
            session_id: session_id.clone(),
            lightning: true,
            fire: true,
            lightning_damage: 9,
            fire_damage: 4,
        });

        let expected_root = runtime.canonical_state_root().expect("state root");
        let bytes = runtime.checkpoint_bytes().expect("checkpoint bytes");
        let mut restored = ZoneRuntime::restore_checkpoint(&bytes).expect("checkpoint restore");

        assert_eq!(
            restored.canonical_state_root().expect("restored root"),
            expected_root
        );
        assert_eq!(
            restored.player_position(&session_id),
            Some(Point { x: 14, y: 27 })
        );
        assert_eq!(restored.player_vitals(&session_id), Some((43, 80, 17)));
        assert!(restored.ecs_mirror_matches_players());
    }

    #[test]
    fn complete_zone_checkpoint_rejects_valid_json_with_changed_state() {
        let runtime = ZoneRuntime::new(ZoneKey::for_map("tamper-map"));
        let bytes = runtime.checkpoint_bytes().expect("checkpoint bytes");
        let mut checkpoint: ZoneRuntimeCheckpoint =
            serde_json::from_slice(&bytes).expect("checkpoint JSON");
        checkpoint.next_object_id = checkpoint.next_object_id.saturating_add(1);
        let tampered = serde_json::to_vec(&checkpoint).expect("tampered checkpoint JSON");

        let error =
            ZoneRuntime::restore_checkpoint(&tampered).expect_err("tampered state must fail");
        assert!(error.contains("state root mismatch"), "{error}");
    }

    #[test]
    fn verified_world_checkpoint_reanchors_a_module_dependent_state_root() {
        let runtime = ZoneRuntime::new(ZoneKey::for_map("module-upgrade-map"));
        let bytes = runtime.checkpoint_bytes().expect("checkpoint bytes");
        let mut checkpoint: ZoneRuntimeCheckpoint =
            serde_json::from_slice(&bytes).expect("checkpoint JSON");
        checkpoint.state_root = "0".repeat(64);
        let previous_module = serde_json::to_vec(&checkpoint).expect("previous module checkpoint");

        assert!(ZoneRuntime::restore_checkpoint(&previous_module).is_err());
        let restored = ZoneRuntime::restore_verified_world_checkpoint(&previous_module)
            .expect("outer-verified World checkpoint should re-anchor");
        assert_ne!(restored.canonical_state_root().unwrap(), "0".repeat(64));
    }

    #[test]
    fn legacy_v1_ground_drop_fixture_rebuilds_authority_from_legacy_payload_only() {
        let fixture = legacy_v1_ground_drop_fixture();
        assert!(fixture.get("next_ground_drop_generation").is_none());
        assert!(fixture.get("next_ground_drop_claim_id").is_none());
        assert!(fixture["ground_drops"]["101"]
            .get("drop_generation")
            .is_none());
        assert!(fixture["ground_drops"]["101"]
            .get("payload_digest")
            .is_none());
        assert!(fixture["claimed_ground_drops"]["102"]
            .get("ticket")
            .is_none());

        let restored = ZoneRuntime::restore_checkpoint(
            &serde_json::to_vec(&fixture).expect("legacy fixture bytes"),
        )
        .expect("strict v1 fixture restore");
        let active = restored
            .ground_drops
            .get(&101)
            .expect("restored active drop");
        assert_eq!(active.drop_generation, 1);
        assert_eq!(
            active.payload_digest,
            canonical_ground_drop_payload_digest(&active.drop)
        );
        let ticket = restored
            .claimed_ground_drops
            .get(&102)
            .and_then(|claim| claim.ticket.as_ref())
            .expect("rebuilt claimed-drop ticket");
        assert_eq!(ticket.claim_id, 1);
        assert_eq!(ticket.drop_generation, 2);
        assert_eq!(
            ticket.payload_digest,
            canonical_ground_drop_payload_digest(&ticket.drop)
        );
        assert_eq!(
            ticket.idempotency_key,
            canonical_ground_drop_claim_idempotency_key(
                &restored.key,
                ticket.object_id,
                ticket.drop_generation,
                ticket.claim_id,
                &ticket.payload_digest,
            )
        );
        assert_eq!(restored.next_ground_drop_generation, 3);
        assert_eq!(restored.next_ground_drop_claim_id, 2);
    }

    #[test]
    fn legacy_v1_restore_ignores_injected_v2_ground_drop_authority_fields() {
        let mut fixture = legacy_v1_ground_drop_fixture();
        fixture["next_ground_drop_generation"] = serde_json::json!(999_u64);
        fixture["next_ground_drop_claim_id"] = serde_json::json!(998_u64);
        fixture["ground_drops"]["101"]["drop_generation"] = serde_json::json!(777_u64);
        fixture["ground_drops"]["101"]["payload_digest"] = serde_json::json!("forged-active");
        fixture["claimed_ground_drops"]["102"]["ticket"] =
            serde_json::to_value(GroundDropClaimTicket {
                claim_id: 555,
                object_id: 102,
                drop_generation: 666,
                payload_digest: "forged-claim".to_string(),
                idempotency_key: "forged-idempotency-key".to_string(),
                session_id: SessionId::new("forged-session"),
                owner_object_id: Some(123),
                drop: checkpoint_gold_drop(102),
            })
            .expect("forged ticket JSON");

        let restored = ZoneRuntime::restore_checkpoint(
            &serde_json::to_vec(&fixture).expect("injected legacy fixture bytes"),
        )
        .expect("injected v2 fields must not affect verified v1 restore");
        let active = restored
            .ground_drops
            .get(&101)
            .expect("restored active drop");
        assert_eq!(active.drop_generation, 1);
        assert_ne!(active.payload_digest, "forged-active");
        assert_eq!(
            active.payload_digest,
            canonical_ground_drop_payload_digest(&active.drop)
        );
        let ticket = restored
            .claimed_ground_drops
            .get(&102)
            .and_then(|claim| claim.ticket.as_ref())
            .expect("rebuilt claimed-drop ticket");
        assert_eq!(ticket.claim_id, 1);
        assert_eq!(ticket.drop_generation, 2);
        assert_ne!(ticket.payload_digest, "forged-claim");
        assert_ne!(ticket.idempotency_key, "forged-idempotency-key");
        assert_eq!(ticket.session_id, SessionId::new("legacy-claimer"));
        assert_eq!(ticket.owner_object_id, None);
        assert_eq!(
            ticket.idempotency_key,
            canonical_ground_drop_claim_idempotency_key(
                &restored.key,
                ticket.object_id,
                ticket.drop_generation,
                ticket.claim_id,
                &ticket.payload_digest,
            )
        );
        assert_eq!(restored.next_ground_drop_generation, 3);
        assert_eq!(restored.next_ground_drop_claim_id, 2);
    }

    #[test]
    fn ground_drop_authority_allocators_fail_closed_at_u64_exhaustion() {
        let mut runtime = ZoneRuntime::new(ZoneKey::for_map("authority-id-exhaustion"));
        runtime.next_ground_drop_generation = u64::MAX;
        runtime.next_ground_drop_claim_id = u64::MAX;

        assert_eq!(runtime.allocate_ground_drop_generation(), None);
        assert_eq!(runtime.allocate_ground_drop_claim_id(), None);
        assert!(runtime
            .new_zone_ground_drop(checkpoint_gold_drop(200), None)
            .is_none());
        assert_eq!(runtime.next_ground_drop_generation, u64::MAX);
        assert_eq!(runtime.next_ground_drop_claim_id, u64::MAX);
    }
    #[test]
    fn verified_world_checkpoint_keeps_v2_ground_drop_authority_validation() {
        let mut runtime = ZoneRuntime::new(ZoneKey::for_map("v2-drop-validation"));
        let drop = checkpoint_gold_drop(201);
        let stored = runtime
            .new_zone_ground_drop(drop.clone(), None)
            .expect("ground-drop generation");
        runtime.ground_drops.insert(drop.object_id, stored);
        let mut checkpoint: ZoneRuntimeCheckpoint =
            serde_json::from_slice(&runtime.checkpoint_bytes().expect("v2 checkpoint bytes"))
                .expect("v2 checkpoint JSON");
        checkpoint
            .ground_drops
            .get_mut(&drop.object_id)
            .expect("v2 stored drop")
            .payload_digest = "forged-v2-digest".to_string();

        let error = ZoneRuntime::restore_verified_world_checkpoint(
            &serde_json::to_vec(&checkpoint).expect("forged v2 checkpoint bytes"),
        )
        .expect_err("v2 authority validation must survive world-root reanchoring");
        assert!(
            error.contains("invalid authoritative ground drop"),
            "{error}"
        );
    }
    #[test]
    fn world_event_monster_without_player_is_authoritative_and_checkpointed() {
        use crate::runtime::zone::types::{ZoneMonsterDefense, ZoneMonsterSpawn};

        let mut runtime = ZoneRuntime::new(ZoneKey::for_map("D022"));
        let spawn = ZoneMonsterSpawn {
            object_id: 9_100_001,
            name: "WoomaSoldier".to_string(),
            name_colour_argb: -1,
            image: 135,
            ai: 2,
            disposition: Some(crate::config::WorldEntityDisposition::Hostile),
            level: 24,
            max_hp: 500,
            hp: 500,
            experience: 1_000,
            move_speed_ms: 0,
            attack_speed_ms: 0,
            friendly_guild: None,
            position: Point { x: 30, y: 30 },
            direction: MirDirection::Down,
            defense: ZoneMonsterDefense::default(),
            drops: Vec::new(),
        };
        let (spawned, outbounds) = runtime.spawn_world_event_monster(&spawn, 1_000);
        assert!(spawned);
        assert!(outbounds.is_empty(), "empty Zone has no AOI recipients");
        assert!(runtime.has_native_monster(spawn.object_id));

        let bytes = runtime.checkpoint_bytes().expect("event checkpoint");
        let restored = ZoneRuntime::restore_checkpoint(&bytes).expect("event checkpoint restore");
        assert!(restored.has_native_monster(spawn.object_id));
        assert_eq!(restored.native_monster_count(), 1);
        assert_eq!(
            restored.canonical_state_root().unwrap(),
            runtime.canonical_state_root().unwrap()
        );
    }

    #[test]
    fn legacy_checkpoint_missing_monster_disposition_fails_closed() {
        use crate::runtime::zone::types::{ZoneMonsterDefense, ZoneMonsterSpawn};

        let mut runtime = ZoneRuntime::new(ZoneKey::for_map("D022"));
        let spawn = ZoneMonsterSpawn {
            object_id: 9_100_099,
            name: "LegacyHostile".to_string(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(crate::config::WorldEntityDisposition::Hostile),
            level: 1,
            max_hp: 10,
            hp: 10,
            experience: 0,
            move_speed_ms: 0,
            attack_speed_ms: 0,
            friendly_guild: None,
            position: Point { x: 30, y: 30 },
            direction: MirDirection::Down,
            defense: ZoneMonsterDefense::default(),
            drops: Vec::new(),
        };
        assert!(runtime.spawn_world_event_monster(&spawn, 0).0);
        let mut checkpoint: serde_json::Value =
            serde_json::from_slice(&runtime.checkpoint_bytes().expect("checkpoint bytes"))
                .expect("checkpoint JSON");
        checkpoint["native_monsters"][spawn.object_id.to_string()]
            .as_object_mut()
            .expect("retained native monster")
            .remove("disposition");

        let legacy_bytes = serde_json::to_vec(&checkpoint).expect("legacy checkpoint bytes");
        let restored = ZoneRuntime::restore_verified_world_checkpoint(&legacy_bytes)
            .expect("verified legacy checkpoint should restore");
        let monster = restored
            .native_monster_snapshots()
            .into_iter()
            .find(|monster| monster.object_id == spawn.object_id)
            .expect("legacy monster");
        assert_eq!(monster.disposition, None);
        assert!(!monster.hostile_to_player);
    }

    #[test]
    fn online_player_receives_world_event_monster_spawn_packet() {
        use crate::runtime::zone::types::{ZoneMonsterDefense, ZoneMonsterSpawn};

        let session_id = SessionId::new("director-witness");
        let mut runtime =
            ZoneRuntime::new_with_collision(ZoneKey::for_map("D024"), ZoneCollision::unbounded());
        runtime.handle(ZoneCommand::Join(ZoneJoin {
            session_id,
            account_id: "witness-account".to_string(),
            character_index: 9,
            object_id: 99,
            name: "Witness".to_string(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 30,
            hp: 300,
            max_hp: 300,
            mp: 80,
            map_file_name: "D024".to_string(),
            position: Point { x: 30, y: 30 },
            direction: MirDirection::Down,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        }));
        let spawn = ZoneMonsterSpawn {
            object_id: 9_100_002,
            name: "WoomaTaurus".to_string(),
            name_colour_argb: -65_281,
            image: 139,
            ai: 58,
            disposition: Some(crate::config::WorldEntityDisposition::Hostile),
            level: 40,
            max_hp: 10_000,
            hp: 10_000,
            experience: 20_000,
            move_speed_ms: 0,
            attack_speed_ms: 0,
            friendly_guild: None,
            position: Point { x: 31, y: 30 },
            direction: MirDirection::Down,
            defense: ZoneMonsterDefense::default(),
            drops: Vec::new(),
        };
        let (spawned, outbounds) = runtime.spawn_world_event_monster(&spawn, 2_000);
        assert!(spawned);
        assert!(outbounds.iter().any(|outbound| match outbound {
            ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. } => {
                packets.iter().any(|packet| {
                    matches!(
                        packet,
                        mir2_protocol::ServerPacket::ObjectMonster { info }
                            if info.object_id == spawn.object_id && info.name == "WoomaTaurus"
                    )
                })
            }
            _ => false,
        }));
    }
}

use std::collections::{BTreeMap, BTreeSet};

use mir2_protocol::{
    decode_server_packet, encode_server_packet, ObjectHealthInfo, ObjectManaInfo, Point,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    PendingNativeGroundSpellAction, PendingNativeMonsterHit, PendingNativePlayerHeal,
    PendingNativePlayerHit, PendingNativeProjectile, PendingNativeSummon, ZoneHazardState,
    ZoneObjectDeadState, ZoneRuntime,
};
use crate::runtime::zone::types::{
    SessionId, ZoneGroundDrop, ZoneGroundDropClaim, ZoneKey, ZoneNativeMonster, ZoneObject,
    ZonePlayer, ZonePlayerBuff,
};
use crate::runtime::zone::ZoneCollision;

const CANONICAL_ZONE_STATE_VERSION: u32 = 1;
const CANONICAL_ZONE_STATE_DOMAIN: &[u8] = b"obelisk.mir2.zone-state.v1\0";
const ZONE_RUNTIME_CHECKPOINT_VERSION: u32 = 1;

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
            open_doors: self.open_doors.clone(),
            hazard: self.hazard.clone(),
            next_object_id: self.next_object_id,
        };
        serde_json::to_vec(&checkpoint)
            .map_err(|error| format!("failed to encode zone runtime checkpoint: {error}"))
    }

    pub fn restore_checkpoint(bytes: &[u8]) -> Result<Self, String> {
        let checkpoint: ZoneRuntimeCheckpoint = serde_json::from_slice(bytes)
            .map_err(|error| format!("failed to decode zone runtime checkpoint: {error}"))?;
        if checkpoint.version != ZONE_RUNTIME_CHECKPOINT_VERSION {
            return Err(format!(
                "unsupported zone runtime checkpoint version {}, expected {}",
                checkpoint.version, ZONE_RUNTIME_CHECKPOINT_VERSION
            ));
        }
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
        runtime.pending_native_hits = checkpoint.pending_native_hits;
        runtime.pending_native_projectiles = checkpoint.pending_native_projectiles;
        runtime.pending_native_player_hits = checkpoint.pending_native_player_hits;
        runtime.pending_native_player_heals = checkpoint.pending_native_player_heals;
        runtime.pending_native_summons = checkpoint.pending_native_summons;
        runtime.pending_native_ground_spells = checkpoint.pending_native_ground_spells;
        runtime.ground_drops = checkpoint.ground_drops;
        runtime.claimed_ground_drops = checkpoint.claimed_ground_drops;
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

        let restored_root = runtime.canonical_state_root()?;
        if restored_root != checkpoint.state_root {
            return Err(format!(
                "zone runtime checkpoint state root mismatch: expected {}, got {restored_root}",
                checkpoint.state_root
            ));
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
    use crate::runtime::zone::types::{
        ZoneChatProfile, ZoneCommand, ZoneJoin, ZoneOutbound, ZonePlayerCombatStats,
    };
    use mir2_protocol::{MirClass, MirDirection, MirGender};

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
    fn world_event_monster_without_player_is_authoritative_and_checkpointed() {
        use crate::runtime::zone::types::{ZoneMonsterDefense, ZoneMonsterSpawn};

        let mut runtime = ZoneRuntime::new(ZoneKey::for_map("D022"));
        let spawn = ZoneMonsterSpawn {
            object_id: 9_100_001,
            name: "WoomaSoldier".to_string(),
            name_colour_argb: -1,
            image: 135,
            ai: 2,
            level: 24,
            max_hp: 500,
            hp: 500,
            experience: 1_000,
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
            level: 40,
            max_hp: 10_000,
            hp: 10_000,
            experience: 20_000,
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

use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use bevy_tasks::{ComputeTaskPool, TaskPool};
use mir2_protocol::{MirDirection, Point, Spell};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use super::online_identity::{OnlineIdentityBook, OnlinePresenceSnapshot};

/// Below this many zones, parallel ticking's task overhead outweighs the gain,
/// so `tick_all` runs inline. Measured break-even; see the multi-zone scaling
/// section of `examples/zone_load.rs` and docs/L2-ECS-ZONE-DESIGN.md.
const PARALLEL_TICK_MIN_ZONES: usize = 4;
const ZONE_MANAGER_CHECKPOINT_VERSION: u32 = 3;

use super::runtime::ZoneRuntime;
use super::types::{
    GroundDropClaimTicket, SessionId, ZoneCommand, ZoneJoin, ZoneKey, ZoneNativeMonsterSnapshot,
    ZoneMonsterKillAward, ZoneOutbound,
};

// Not `Clone`/`Default`-derived: it holds `ZoneRuntime`s which own a
// non-cloneable `bevy_ecs::World`, and nothing cloned/defaulted the manager.
#[derive(Debug)]
pub struct ZoneManager {
    zones: BTreeMap<ZoneKey, ZoneRuntime>,
    session_zones: BTreeMap<SessionId, ZoneKey>,
    online_identities: OnlineIdentityBook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZoneManagerCheckpoint {
    version: u32,
    zones: Vec<(ZoneKey, String)>,
    session_zones: BTreeMap<SessionId, ZoneKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    online_identities: Option<OnlineIdentityBook>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    state_root: String,
}

impl ZoneManagerCheckpoint {
    fn canonical_root(&self) -> Result<String, String> {
        let mut canonical = self.clone();
        canonical.state_root.clear();
        let bytes = serde_json::to_vec(&canonical).map_err(|e|format!("manager commitment: {e}"))?;
        let mut hasher = Sha256::new();
        hasher.update(b"obelisk.mir2.zone-manager.v3\0");
        hasher.update(bytes);
        Ok(hasher.finalize().iter().map(|byte|format!("{byte:02x}")).collect())
    }
}

impl ZoneManager {
    fn online_snapshot(&self) -> OnlinePresenceSnapshot {
        self.zones.values().flat_map(|zone|zone.online_player_snapshot(&self.online_identities)).collect()
    }
    fn refresh_online_presence(&mut self) {
        let snapshot = self.online_snapshot();
        for zone in self.zones.values_mut() { zone.ingest_online_presence(snapshot.clone(), true); }
    }
    /// Server-only proof: no packet handler admits or chooses its epoch.
    pub fn online_owner_proof_is_current(&self, encoded: &str) -> bool {
        self.online_identities.validates_encoded(encoded)
    }
    pub fn online_owner_proof_matches_session(&self, encoded: &str, session_id: &SessionId) -> bool {
        self.online_identities.validates_for_session(encoded, session_id)
    }
    pub fn online_owner_proof_for_session(&self, session_id: &SessionId) -> Option<String> {
        Some(self.online_identities.owner(session_id)?.encoded())
    }
    /// An online Node alone cannot choose an award's source map or payload.
    /// Only the named source Zone's retained issuance can authorize delivery.
    pub fn issued_monster_award_is_current(&self, source: &ZoneKey, session_id: &SessionId,
        encoded: &str, award: &ZoneMonsterKillAward) -> bool {
        self.online_owner_proof_matches_session(encoded, session_id)
            && self.zones.get(source).is_some_and(|zone|zone.issued_monster_award_matches(session_id, encoded, award))
    }
    pub fn acknowledge_issued_monster_award(&mut self, source: &ZoneKey, session_id: &SessionId,
        award: &ZoneMonsterKillAward) {
        if let Some(zone) = self.zones.get_mut(source) { zone.acknowledge_issued_monster_award(session_id, award); }
    }
    pub fn ground_drop_snapshot_for_key(&self, key: &ZoneKey, object_id: u32) -> Option<crate::config::GroundDropSnapshot> {
        self.zones.get(key)?.active_ground_drop_snapshot(object_id)
    }
    pub fn ground_drop_absolute_clocks_for_key(&self, key: &ZoneKey, object_id: u32) -> Option<(Option<u64>, Option<u64>)> {
        self.zones.get(key)?.ground_drop_absolute_clocks(object_id)
    }
    /// Some(None) means original native protection was released. None means
    /// this is not a source-owned native monster drop and has a different policy.
    pub fn native_ground_drop_deadline_for_key(&self, key: &ZoneKey, object_id: u32) -> Option<Option<u64>> {
        self.zones.get(key)?.native_ground_drop_deadline(object_id)
    }
    pub fn forget_detached_ground_custody(&mut self, key: &ZoneKey, ticket: &GroundDropClaimTicket) -> bool {
        self.zones.get_mut(key).is_some_and(|zone|zone.forget_detached_ground_custody(ticket))
    }
    pub fn conquest_projection_ready(&self, session_id: &SessionId) -> bool {
        self.session_zones.get(session_id).and_then(|key| self.zones.get(key))
            .is_some_and(|zone| zone.conquest_projection_ready())
    }
    pub fn conquest_record_revision(&self, session_id: &SessionId) -> Option<u64> {
        self.session_zones.get(session_id).and_then(|key| self.zones.get(key)).and_then(|zone| zone.conquest_record_revision())
    }
    pub fn conquest_membership_matches(&self, session_id: &SessionId, guild_id: Option<&str>) -> bool {
        self.session_zones.get(session_id).and_then(|key| self.zones.get(key)).is_some_and(|zone| zone.conquest_membership_matches(session_id,guild_id))
    }
    pub fn conquest_player_kill_is_lawful(&self, session_id: &SessionId, attacker_object_id: u32, target_object_id: u32, now_ms: u64) -> bool {
        self.session_zones.get(session_id).and_then(|key| self.zones.get(key))
            .is_some_and(|zone| zone.conquest_player_kill_is_lawful(session_id, attacker_object_id, target_object_id, now_ms))
    }
    pub fn conquest_player_samples(&self)->Vec<super::ZoneConquestPlayerSample> {
        self.zones.values().flat_map(|zone|zone.conquest_player_samples()).collect()
    }
    pub fn conquest_defense_samples(&self)->Vec<super::ZoneConquestDefenseSample> {
        self.zones.values().flat_map(|zone|zone.conquest_defense_samples()).collect()
    }
    pub fn apply_conquest_projection(
        &mut self, policy:&crate::conquest::ConquestPolicy, record:&crate::conquest::SharedConquestRecord,
        memberships:&[super::ZoneConquestMembership],epoch_now_ms:u64,zone_now_ms:u64,
    )->Result<Vec<ZoneOutbound>,String> {
        let mut outbounds=Vec::new();
        for (key,zone) in &mut self.zones {
            if key.shard_id!="primary" || key.channel_id!=0 || key.instance_id!="main" {continue;}
            outbounds.extend(zone.apply_conquest_projection(policy,record,memberships,epoch_now_ms,zone_now_ms)?);
        }
        Ok(outbounds)
    }
    pub fn next_pending_movement_deadline_ms(&self) -> Option<u64> {
        self.zones
            .values()
            .filter_map(ZoneRuntime::next_pending_movement_deadline_ms)
            .min()
    }

    pub fn tick_pending_movement(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        self.zones
            .values_mut()
            .flat_map(|zone| zone.tick_pending_movement(now_ms))
            .collect()
    }

    pub fn sync_intelligent_creature(
        &mut self,
        session_id: &SessionId,
        creature: Option<mir2_protocol::ClientIntelligentCreature>,
        allowed_object_ids: std::collections::BTreeSet<u32>,
        group_members: Vec<String>,
        map_allows: bool,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(key) = self.session_zones.get(session_id) else {
            return vec![];
        };
        self.zones
            .get_mut(key)
            .map(|z| {
                z.sync_intelligent_creature(
                    session_id,
                    creature,
                    allowed_object_ids,
                    group_members,
                    map_allows,
                    now_ms,
                )
            })
            .unwrap_or_default()
    }
    pub fn request_intelligent_creature_pickup(
        &mut self,
        session_id: &SessionId,
        mouse_mode: bool,
        location: Point,
    ) -> bool {
        let Some(key) = self.session_zones.get(session_id) else {
            return false;
        };
        self.zones.get_mut(key).is_some_and(|z| {
            z.request_intelligent_creature_pickup(session_id, mouse_mode, location)
        })
    }
    pub fn intelligent_creature_object_id(&self, session_id: &SessionId) -> Option<u32> {
        self.zones
            .get(self.session_zones.get(session_id)?)?
            .intelligent_creature_object_id(session_id)
    }
    pub fn intelligent_creature_intent_is_current(
        &self,
        intent: &super::CreaturePickupIntent,
    ) -> bool {
        self.session_zones
            .get(&intent.owner.session_id)
            .and_then(|key| self.zones.get(key))
            .is_some_and(|z| z.intelligent_creature_intent_is_current(intent))
    }
    pub fn settle_intelligent_creature_pickup(
        &mut self,
        session_id: &SessionId,
        creature_object_id: u32,
        drop_id: u32,
    ) -> bool {
        let Some(key) = self.session_zones.get(session_id) else {
            return false;
        };
        self.zones.get_mut(key).is_some_and(|z| {
            z.settle_intelligent_creature_pickup(session_id, creature_object_id, drop_id)
        })
    }
    /// Search all zones because an already-started operation survives leaving its
    /// visual actor. Preserve every other owner's entries in their original order.
    pub fn drain_intelligent_creature_operations(
        &mut self,
        session_id: &SessionId,
    ) -> Vec<super::CreatureOperation> {
        self.zones
            .values_mut()
            .flat_map(|z| z.drain_intelligent_creature_operations_for(session_id))
            .collect()
    }
    pub fn drain_intelligent_creature_pickup_intents(
        &mut self,
        session_id: &SessionId,
    ) -> Vec<super::CreaturePickupIntent> {
        self.zones
            .values_mut()
            .flat_map(|z| z.drain_intelligent_creature_pickup_intents_for(session_id))
            .collect()
    }
    pub fn settle_intelligent_creature_operation(
        &mut self,
        session_id: &SessionId,
        operation_id: &str,
    ) -> bool {
        let mut settled = false;
        for zone in self.zones.values_mut() {
            settled |= zone.settle_intelligent_creature_operation(session_id, operation_id);
        }
        settled
    }
    /// Internal authenticated-account recovery path. Session IDs identify the
    /// original delivery, while account + character owns the durable receipt.
    /// Callers must derive both arguments from their active identity, never wire
    /// input. Ack using the returned operation's original owner.session_id.
    pub fn peek_intelligent_creature_operations_for_identity(
        &self,
        account_id: &str,
        character_index: i32,
    ) -> Vec<super::CreatureOperation> {
        if account_id.is_empty() {
            return Vec::new();
        }
        self.zones
            .values()
            .flat_map(|zone| {
                zone.peek_intelligent_creature_operations_for_identity(account_id, character_index)
            })
            .collect()
    }
    pub fn new() -> Self {
        Self {
            zones: BTreeMap::new(),
            session_zones: BTreeMap::new(),
            online_identities: OnlineIdentityBook::default(),
        }
    }

    pub fn checkpoint_bytes(&self) -> Result<Vec<u8>, String> {
        let zones = self
            .zones
            .iter()
            .map(|(key, runtime)| Ok((key.clone(), STANDARD.encode(runtime.checkpoint_bytes()?))))
            .collect::<Result<Vec<_>, String>>()?;
        let mut checkpoint = ZoneManagerCheckpoint {
            version: ZONE_MANAGER_CHECKPOINT_VERSION,
            zones,
            session_zones: self.session_zones.clone(),
            online_identities: Some(self.online_identities.clone()),
            state_root: String::new(),
        };
        checkpoint.state_root = checkpoint.canonical_root()?;
        serde_json::to_vec(&checkpoint)
        .map_err(|error| format!("failed to encode zone manager checkpoint: {error}"))
    }

    pub fn restore_checkpoint(bytes: &[u8]) -> Result<Self, String> {
        Self::restore_checkpoint_with(bytes, ZoneRuntime::restore_checkpoint)
    }

    /// Restore a Zone manager that is already covered by a verified World
    /// Director checkpoint commitment. Runtime roots are re-anchored to the
    /// current signed game module before all session-owned players are removed.
    pub fn restore_verified_world_checkpoint(bytes: &[u8]) -> Result<Self, String> {
        Self::restore_checkpoint_with(bytes, ZoneRuntime::restore_verified_world_checkpoint)
    }

    fn restore_checkpoint_with(
        bytes: &[u8],
        restore_runtime: impl Fn(&[u8]) -> Result<ZoneRuntime, String>,
    ) -> Result<Self, String> {
        let checkpoint: ZoneManagerCheckpoint = serde_json::from_slice(bytes)
            .map_err(|error| format!("failed to decode zone manager checkpoint: {error}"))?;
        if checkpoint.version != 2 && checkpoint.version != ZONE_MANAGER_CHECKPOINT_VERSION {
            return Err(format!(
                "unsupported zone manager checkpoint version {}, expected {}",
                checkpoint.version, ZONE_MANAGER_CHECKPOINT_VERSION
            ));
        }
        if checkpoint.version == ZONE_MANAGER_CHECKPOINT_VERSION && checkpoint.state_root != checkpoint.canonical_root()? {
            return Err("zone manager checkpoint state root mismatch".into());
        }
        let mut zones = BTreeMap::new();
        for (expected_key, runtime_encoded) in checkpoint.zones {
            let runtime_bytes = STANDARD
                .decode(runtime_encoded.as_bytes())
                .map_err(|error| {
                    format!("failed to decode zone manager runtime checkpoint: {error}")
                })?;
            let mut runtime = restore_runtime(&runtime_bytes)?;
            if runtime.key() != &expected_key {
                return Err(format!(
                    "zone manager checkpoint key mismatch for map {}",
                    expected_key.map_file_name
                ));
            }
            // Integrity (including a verified outer checkpoint) is not live
            // admission. A cold restore has no original spawned Nodes.
            runtime.clear_online_authority();
            if zones.insert(expected_key.clone(), runtime).is_some() {
                return Err(format!(
                    "zone manager checkpoint contains duplicate map {}",
                    expected_key.map_file_name
                ));
            }
        }
        for (session_id, key) in &checkpoint.session_zones {
            let runtime = zones.get(key).ok_or_else(|| {
                format!(
                    "zone manager checkpoint routes session {} to a missing zone",
                    session_id.as_str()
                )
            })?;
            if runtime.player_object_id(session_id).is_none() {
                return Err(format!(
                    "zone manager checkpoint routes session {} without a player",
                    session_id.as_str()
                ));
            }
        }
        Ok(Self {
            zones,
            session_zones: checkpoint.session_zones,
            online_identities: OnlineIdentityBook::default(),
        })
    }

    pub fn join(&mut self, join: ZoneJoin) -> Vec<ZoneOutbound> {
        let key = ZoneKey::for_map(join.map_file_name.clone());
        self.handle_for_key(key, ZoneCommand::Join(join))
    }

    /// Gateway supplies only authenticated personal equipment/cosmetics.
    /// Calibrate the new Zone actor before its first ObjectPlayer is sent.
    pub fn join_with_appearance(
        &mut self,
        join: ZoneJoin,
        appearance: super::types::ZonePlayerAppearance,
    ) -> Vec<ZoneOutbound> {
        let session_id = join.session_id.clone();
        let key = ZoneKey::for_map(&join.map_file_name);
        let mut outbounds = self.join(join);
        if self.session_zones.get(&session_id) == Some(&key) {
            if let Some(zone) = self.zones.get_mut(&key) {
                zone.calibrate_joined_player_appearance(&session_id, appearance, &mut outbounds);
            }
        }
        outbounds
    }

    pub fn handle(&mut self, command: ZoneCommand) -> Vec<ZoneOutbound> {
        match &command {
            ZoneCommand::Join(join) => self.join(join.clone()),
            ZoneCommand::Leave { session_id }
            | ZoneCommand::Walk { session_id, .. }
            | ZoneCommand::Run { session_id, .. }
            | ZoneCommand::Turn { session_id, .. }
            | ZoneCommand::TeleportToNpc { session_id, .. }
            | ZoneCommand::UpdateChatProfile { session_id, .. }
            | ZoneCommand::UpdatePlayerCombatStats { session_id, .. }
            | ZoneCommand::SyncPlayerCombatState { session_id, .. }
            | ZoneCommand::SyncPlayerTransform { session_id, .. }
            | ZoneCommand::SyncPlayerVitals { session_id, .. }
            | ZoneCommand::SyncPlayerVitalsAndLife { session_id, .. }
            | ZoneCommand::SyncPlayerAppearance { session_id, .. }
            | ZoneCommand::Chat { session_id, .. }
            | ZoneCommand::BroadcastPackets { session_id, .. }
            | ZoneCommand::SyncSharedObjects { session_id, .. }
            | ZoneCommand::BroadcastSharedObjectPackets { session_id, .. }
            | ZoneCommand::SyncGroundDrops { session_id, .. }
            | ZoneCommand::SpawnMonster { session_id, .. }
            | ZoneCommand::SyncNativeMonsters { session_id, .. }
            | ZoneCommand::PlayerAttackObject { session_id, .. }
            | ZoneCommand::PlayerAttackMaterializedObject { session_id, .. }
            | ZoneCommand::PlayerRangeAttackObject { session_id, .. }
            | ZoneCommand::PlayerRangeAttackMaterializedObject { session_id, .. }
            | ZoneCommand::PlayerCastMagic { session_id, .. }
            | ZoneCommand::PlayerCastMagicWithItem { session_id, .. }
            | ZoneCommand::PreparePlayerFlamingSword { session_id, .. }
            | ZoneCommand::ResolveReincarnation { session_id, .. }
            | ZoneCommand::ClaimGroundDrop { session_id, .. }
            | ZoneCommand::ClaimNearestGroundDrop { session_id, .. }
            | ZoneCommand::CommitGroundDropClaim { session_id, .. }
            | ZoneCommand::CommitGroundDropClaimWithTicket { session_id, .. }
            | ZoneCommand::CancelGroundDropClaim { session_id, .. }
            | ZoneCommand::CancelGroundDropClaimWithTicket { session_id, .. }
            | ZoneCommand::CancelPendingMovement { session_id }
            | ZoneCommand::OpenDoor { session_id, .. }
            | ZoneCommand::ConfigureHazards { session_id, .. }
            | ZoneCommand::TickPlayerMovement { session_id, .. } => {
                let Some(key) = self.session_zones.get(session_id).cloned() else {
                    return Vec::new();
                };
                self.handle_for_key(key, command)
            }
            ZoneCommand::Tick { now_ms } => self.tick_all(*now_ms),
        }
    }

    /// Remove every session-owned player while retaining persistent Zone state.
    ///
    /// World-level checkpoints must not resurrect Gateway sessions after a
    /// process restart. The Gateway owns session recovery; the Zone manager only
    /// contributes persistent map/world state to those checkpoints.
    pub fn zone_key_for_session(&self, session_id: &SessionId) -> Option<ZoneKey> {
        self.session_zones.get(session_id).cloned()
    }

    pub fn detached_ground_drop_claim_ticket_is_canonical(
        &self,
        key: &ZoneKey,
        ticket: &GroundDropClaimTicket,
    ) -> bool {
        self.zones
            .get(key)
            .is_some_and(|zone| zone.detached_ground_drop_claim_ticket_is_canonical(ticket))
    }

    pub fn has_detached_ground_drop_claim_ticket(
        &self,
        key: &ZoneKey,
        ticket: &GroundDropClaimTicket,
    ) -> bool {
        self.zones
            .get(key)
            .is_some_and(|zone| zone.has_detached_ground_drop_claim_ticket(ticket))
    }

    pub fn detach_ground_drop_claim(
        &mut self,
        session_id: &SessionId,
        ticket: &GroundDropClaimTicket,
    ) -> Option<ZoneKey> {
        let key = self.session_zones.get(session_id)?.clone();
        self.zones
            .get_mut(&key)?
            .detach_ground_drop_claim(session_id, ticket)
            .then_some(key)
    }

    pub fn detach_all_ground_drop_claims(
        &mut self,
    ) -> Vec<(ZoneKey, SessionId, GroundDropClaimTicket)> {
        let mut detached = Vec::new();
        for (key, zone) in &mut self.zones {
            detached.extend(
                zone.detach_all_ground_drop_claims()
                    .into_iter()
                    .map(|(session_id, ticket)| (key.clone(), session_id, ticket)),
            );
        }
        detached
    }

    pub fn restore_detached_ground_drop_claim(
        &mut self,
        key: &ZoneKey,
        ticket: &GroundDropClaimTicket,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        self.zones
            .get_mut(key)?
            .restore_detached_ground_drop_claim(ticket, now_ms)
    }

    pub fn has_pending_ground_drop_claim_ticket(
        &self,
        session_id: &SessionId,
        ticket: &GroundDropClaimTicket,
    ) -> bool {
        let Some(key) = self.session_zones.get(session_id) else {
            return false;
        };
        self.zones
            .get(key)
            .is_some_and(|zone| zone.has_pending_ground_drop_claim_ticket(session_id, ticket))
    }

    /// Enumerate the authoritative Zone claims awaiting Gateway settlement.
    /// Gateway checkpoint recovery validates each entry against its own
    /// presence mappings before adopting it.
    pub fn pending_ground_drop_claim_tickets(&self) -> Vec<(SessionId, GroundDropClaimTicket)> {
        self.zones
            .values()
            .flat_map(ZoneRuntime::pending_ground_drop_claim_tickets)
            .collect()
    }

    pub fn leave_all_sessions(&mut self) -> usize {
        let session_ids = self.session_zones.keys().cloned().collect::<Vec<_>>();
        let session_count = session_ids.len();
        for session_id in session_ids {
            let _ = self.handle(ZoneCommand::Leave { session_id });
        }
        session_count
    }

    pub fn handle_for_key(&mut self, key: ZoneKey, command: ZoneCommand) -> Vec<ZoneOutbound> {
        // All Join entry points share this transfer path. Carry the same online
        // player's receipt clock before removing its old Zone incarnation;
        // otherwise map changes restart sequences at one under the same ID.
        if let ZoneCommand::Join(join) = command {
            let session_id = join.session_id.clone();
            let previous_key = self.session_zones.get(&session_id).cloned();
            let mut outbounds = Vec::new();
            let mut transferred_clock = None;
            let mut transferred_action_clock = None;
            let mut transferred_poison_clock = None;
            let online_map_transfer = previous_key.as_ref().is_some_and(|previous| previous != &key);
            let retain_online = online_map_transfer && self.online_identities.owner(&session_id)
                .is_some_and(|owner|owner.matches_join(&join));
            if !retain_online {
                self.online_identities.revoke(&session_id);
                for zone in self.zones.values_mut() { zone.revoke_online_session(&session_id); }
            }
            if self.online_identities.admit(&join, retain_online).is_none() { return Vec::new(); }
            // Retain the same trusted snapshot across the atomic map detach /
            // attach gap. There is no global despawn in that interval.
            let snapshot = self.online_snapshot();
            if let Some(previous_key) = previous_key.filter(|previous| previous != &key) {
                transferred_poison_clock = self.zones.get(&previous_key)
                    .and_then(|zone| zone.player_finite_control_poison_clock(&session_id));
                transferred_action_clock = self.zones.get(&previous_key)
                    .and_then(|zone| zone.player_action_clock(&session_id));
                transferred_clock = self
                    .zones
                    .get(&previous_key)
                    .and_then(|zone| zone.player_vital_clock(&session_id));
                // Map.Remove is not global Despawn. Preserve the Node book.
                if let Some(zone) = self.zones.get_mut(&previous_key) {
                    outbounds.extend(zone.handle(ZoneCommand::Leave { session_id: session_id.clone() }));
                }
            }
            self.session_zones.insert(session_id.clone(), key.clone());
            let zone = self
                .zones
                .entry(key.clone())
                .or_insert_with(|| ZoneRuntime::new(key));
            zone.ingest_online_presence(snapshot, true);
            let mut joined = zone.handle(ZoneCommand::Join(join));
            if let Some(clock) = transferred_clock {
                zone.restore_player_vital_clock(&session_id, clock);
            }
            if let Some(clock) = transferred_action_clock {
                zone.restore_player_action_clock(&session_id, clock);
            }
            if let Some(clock) = transferred_poison_clock {
                zone.restore_player_finite_control_poison_clock(&session_id, clock);
            }
            if online_map_transfer {
                // A same-character destination needs an explicit current mask,
                // including zero; neither missing snapshot fields nor the old
                // viewport's last ObjectPoisoned can establish that state.
                zone.calibrate_online_join_poison(&session_id, &mut joined);
            }
            outbounds.extend(joined);
            self.refresh_online_presence();
            return outbounds;
        }
        match &command {
            ZoneCommand::Leave { session_id } => {
                self.session_zones.remove(session_id);
                self.online_identities.revoke(session_id);
                for zone in self.zones.values_mut() { zone.revoke_online_session(session_id); }
            }
            _ => {}
        }

        self.refresh_online_presence();

        let zone = self
            .zones
            .entry(key.clone())
            .or_insert_with(|| ZoneRuntime::new(key));
        let outbounds = zone.handle(command);
        self.refresh_online_presence();
        outbounds
    }

    /// Tick every zone. Zones are fully independent (each owns its own ECS
    /// `World` and state), so above a break-even count this ticks them in
    /// parallel on a persistent `ComputeTaskPool` — the Stage-A multi-core win
    /// from docs/L2-ECS-ZONE-DESIGN.md. Chunks are spawned in zone-key order and
    /// the pool returns results in spawn order, so the output is byte-identical
    /// to a sequential tick regardless of which core runs each zone. Below the
    /// threshold (the common case, including today's single "primary" zone) it
    /// ticks inline to avoid task overhead.
    pub fn tick_all(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        self.refresh_online_presence();
        let zone_count = self.zones.len();
        if zone_count < PARALLEL_TICK_MIN_ZONES {
            return self.tick_all_sequential(now_ms);
        }

        let pool = ComputeTaskPool::get_or_init(TaskPool::default);
        let workers = pool.thread_num().max(1);
        let chunk_size = zone_count.div_ceil(workers).max(1);
        let mut zones: Vec<&mut ZoneRuntime> = self.zones.values_mut().collect();

        // Each task ticks a contiguous chunk of zones serially and returns their
        // concatenated outbounds. Chunks are spawned in order; `scope` preserves
        // spawn order, so flattening yields the same order as a sequential tick.
        let per_chunk: Vec<Vec<ZoneOutbound>> = pool.scope(|scope| {
            for chunk in zones.chunks_mut(chunk_size) {
                scope.spawn(async move {
                    chunk
                        .iter_mut()
                        .flat_map(|zone| zone.tick(now_ms))
                        .collect::<Vec<ZoneOutbound>>()
                });
            }
        });
        per_chunk.into_iter().flatten().collect()
    }

    pub fn zone(&self, key: &ZoneKey) -> Option<&ZoneRuntime> {
        self.zones.get(key)
    }

    pub fn prepare_mining_swing(&self,session:&SessionId,direction:MirDirection,
        tool:&super::ZoneMiningTool,now_ms:u64)->Option<super::ZoneMiningSwing> {
        let key=self.session_zones.get(session)?;
        self.zones.get(key)?.prepare_mining_swing(session,direction,tool,now_ms)
    }

    pub fn commit_mining_swing(&mut self,session:&SessionId,swing:&super::ZoneMiningSwing)
        ->Option<Vec<ZoneOutbound>> {
        let key=self.session_zones.get(session)?.clone();
        self.zones.get_mut(&key)?.commit_mining_swing(swing)
    }

    pub fn player_flaming_sword_armed(&self, session_id: &SessionId, now_ms: u64) -> bool {
        self.session_zones.get(session_id).and_then(|key| self.zones.get(key))
            .is_some_and(|zone| zone.player_flaming_sword_armed(session_id, now_ms))
    }

    /// Install caller-configured policy before any session joins the Zone.
    /// Used by deterministic fixtures and trusted bootstrap configuration;
    /// replacing an active Zone is deliberately rejected.
    pub fn install_empty_zone(&mut self, zone: ZoneRuntime) -> bool {
        let key = zone.key().clone();
        if self.session_zones.values().any(|active| active == &key) {
            return false;
        }
        self.zones.insert(key, zone);
        true
    }

    pub fn native_monster_snapshots(&self, key: &ZoneKey) -> Vec<ZoneNativeMonsterSnapshot> {
        self.zones
            .get(key)
            .map(ZoneRuntime::native_monster_snapshots)
            .unwrap_or_default()
    }

    pub fn spawn_world_event_monster(
        &mut self,
        key: ZoneKey,
        spawn: &super::types::ZoneMonsterSpawn,
        now_ms: u64,
    ) -> (bool, Vec<ZoneOutbound>) {
        self.zones
            .entry(key.clone())
            .or_insert_with(|| ZoneRuntime::new(key))
            .spawn_world_event_monster(spawn, now_ms)
    }

    pub fn broadcast_world_event_message(
        &mut self,
        key: ZoneKey,
        message: &str,
    ) -> Vec<ZoneOutbound> {
        self.zones
            .entry(key.clone())
            .or_insert_with(|| ZoneRuntime::new(key))
            .broadcast_world_event_message(message)
    }

    /// Tick all zones on the current thread. Identical output to `tick_all`;
    /// used to force single-threaded execution (constrained environments) and as
    /// the benchmark baseline that proves the parallel path's speedup.
    pub fn tick_all_sequential(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        self.refresh_online_presence();
        self.zones
            .values_mut()
            .flat_map(|zone| zone.tick(now_ms))
            .collect()
    }

    pub fn player_transform(&self, session_id: &SessionId) -> Option<(Point, MirDirection)> {
        let key = self.session_zones.get(session_id)?;
        let zone = self.zones.get(key)?;
        Some((
            zone.player_position(session_id)?,
            zone.player_direction(session_id)?,
        ))
    }

    /// Read the current Zone's legal front-cell target before a melee dispatch.
    /// The caller commits progression only after that same swing is accepted.
    pub fn melee_primary_target_present(
        &self,
        session_id: &SessionId,
        direction: MirDirection,
        materialized: Option<&super::types::ZoneMonsterSpawn>,
    ) -> bool {
        self.session_zones
            .get(session_id)
            .and_then(|key| self.zones.get(key))
            .is_some_and(|zone| zone.melee_primary_target_present(session_id, direction, materialized))
    }

    pub fn player_last_seen_move_seq(&self, session_id: &SessionId) -> Option<u64> {
        let key = self.session_zones.get(session_id)?;
        self.zones.get(key)?.player_last_seen_move_seq(session_id)
    }

    pub fn player_has_visible_object(
        &self,
        session_id: &SessionId,
        object_id: u32,
    ) -> Option<bool> {
        let key = self.session_zones.get(session_id)?;
        self.zones
            .get(key)?
            .player_has_visible_object(session_id, object_id)
    }

    pub fn player_life_generation(&self, session_id: &SessionId) -> Option<u64> {
        let key = self.session_zones.get(session_id)?;
        self.zones.get(key)?.player_life_generation(session_id)
    }

    pub fn player_is_dead(&self, session_id: &SessionId) -> Option<bool> {
        let key = self.session_zones.get(session_id)?;
        self.zones.get(key)?.player_is_dead(session_id)
    }

    /// Trusted server-only Harvest admission query for the player's active
    /// Zone. No raw client command can synchronize the predicates it reads.
    pub fn player_vitals(&self, session_id: &SessionId) -> Option<(i32, i32, i32)> {
        let key = self.session_zones.get(session_id)?;
        self.zones.get(key)?.player_vitals(session_id)
    }

    /// Trusted server-only Harvest admission query for the player's active
    /// Zone. No raw client command can synchronize the predicates it reads.
    pub fn player_harvest_admitted(&self, session_id: &SessionId, now_ms: u64) -> bool {
        let Some(key) = self.session_zones.get(session_id) else {
            return false;
        };
        self.zones
            .get(key)
            .is_some_and(|zone| zone.player_harvest_admitted(session_id, now_ms))
    }

    pub fn can_player_cast_magic(
        &self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        direction: MirDirection,
        target: &Point,
        cast: bool,
        damage: i32,
        mp_cost: i32,
        cooldown_ms: u64,
        now_ms: u64,
    ) -> bool {
        let Some(key) = self.session_zones.get(session_id) else {
            return false;
        };
        let Some(zone) = self.zones.get(key) else {
            return false;
        };
        zone.can_player_cast_magic(
            session_id,
            object_id,
            spell,
            direction,
            target,
            cast,
            damage,
            mp_cost,
            cooldown_ms,
            now_ms,
        )
    }

    pub fn player_cast_magic_requires_item_consumption(
        &self,
        session_id: &SessionId,
        spell: Spell,
    ) -> bool {
        let Some(key) = self.session_zones.get(session_id) else {
            return true;
        };
        let Some(zone) = self.zones.get(key) else {
            return true;
        };
        zone.player_cast_magic_requires_item_consumption(session_id, spell)
    }
}

#[cfg(test)]
mod parallel_tick_tests {
    //! Stage-A multi-core proof: parallel `tick_all` must produce byte-identical
    //! output to a sequential tick. Zones are independent (each owns its World),
    //! so the parallel path differs only in *which core* runs each zone, never in
    //! the result. See docs/L2-ECS-ZONE-DESIGN.md.
    use super::*;
    use crate::runtime::zone::types::{ZoneChatProfile, ZonePlayerCombatStats};
    use mir2_protocol::{MirClass, MirGender};

    fn join_on_map(map: &str, session: &str, object_id: u32, x: i32, y: i32) -> ZoneJoin {
        ZoneJoin {
            session_id: SessionId::new(session),
            account_id: format!("{session}-acct"),
            character_index: object_id as i32,
            object_id,
            name: format!("P{object_id}"),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 7,
            hp: 60,
            max_hp: 60,
            mp: 100,
            map_file_name: map.to_string(),
            position: Point { x, y },
            direction: MirDirection::Down,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        }
    }

    // Populate a manager with `maps` zones, each holding two adjacent players who
    // walk, so every zone has real per-tick visibility/movement work.
    fn populate(maps: usize) -> ZoneManager {
        let mut mgr = ZoneManager::new();
        for m in 0..maps {
            let map = format!("map{m}");
            mgr.join(join_on_map(&map, &format!("{map}-a"), 1, 330, 270));
            mgr.join(join_on_map(&map, &format!("{map}-b"), 2, 332, 270));
            mgr.handle_for_key(
                ZoneKey::for_map(map.clone()),
                ZoneCommand::Walk {
                    session_id: SessionId::new(format!("{map}-a")),
                    direction: MirDirection::Right,
                    seq: 1,
                    now_ms: 0,
                },
            );
        }
        mgr
    }

    #[test]
    fn parallel_tick_all_matches_sequential_across_many_zones() {
        // Two independently-built, identical managers; tick one in parallel and
        // the other sequentially, then compare the full outbound streams.
        let mut parallel = populate(8);
        let mut sequential = populate(8);
        let par_out = parallel.tick_all(10);
        let seq_out = sequential.tick_all_sequential(10);
        assert_eq!(
            par_out, seq_out,
            "parallel tick_all must equal the sequential reference"
        );
        assert!(!par_out.is_empty(), "expected real per-zone outbound work");
    }

    #[test]
    fn single_zone_takes_sequential_path_and_still_works() {
        let mut mgr = populate(1);
        let out = mgr.tick_all(10);
        // One zone: cheap sequential path, still produces the same kind of output.
        assert_eq!(out, populate(1).tick_all_sequential(10));
    }

    #[test]
    fn leave_all_sessions_preserves_zones_but_removes_players() {
        let mut manager = populate(2);
        assert_eq!(manager.leave_all_sessions(), 4);
        assert!(manager.session_zones.is_empty());
        assert_eq!(manager.zones.len(), 2);
        for zone in manager.zones.values() {
            assert_eq!(zone.player_count(), 0);
        }
    }
}

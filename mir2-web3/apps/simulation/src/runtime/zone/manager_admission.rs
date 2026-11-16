//! Manager-owned admission identity. Bounded metadata; not a reward dedupe log.
use super::*;
use crate::runtime::zone::{LogicalWorldId, PayloadDigest, StableActionId};
use hmac::{Hmac, KeyInit, Mac};
use rand_core::{OsRng, RngCore};

/// A common base must carry this secret before replay can lazily create maps.
/// Redact it from Debug just like the individual map business-random seeds.
#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "MasterWire", into = "MasterWire")]
pub(super) struct ManagerMapRandomMaster([u8; 32]);
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MasterWire {
    version: u32,
    seed: [u8; 32],
}
impl std::fmt::Debug for ManagerMapRandomMaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ManagerMapRandomMaster { version: 1, .. }")
    }
}
impl ManagerMapRandomMaster {
    pub(super) fn fresh() -> Self {
        let mut seed = [0_u8; 32];
        OsRng.fill_bytes(&mut seed);
        Self(seed)
    }
    pub(super) fn verification_placeholder() -> Self {
        Self([0; 32])
    }
    pub(super) fn seed_for(&self, key: &ZoneKey) -> [u8; 32] {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&self.0)
            .expect("fixed-size HMAC key is valid");
        mac.update(b"mir2.manager.map-random-seed.v1\0");
        mac.update(&serde_json::to_vec(key).expect("logical ZoneKey is serializable"));
        mac.finalize().into_bytes().into()
    }
}
impl TryFrom<MasterWire> for ManagerMapRandomMaster {
    type Error = String;
    fn try_from(value: MasterWire) -> Result<Self, String> {
        if value.version != 1 {
            return Err("unsupported manager map random master version".into());
        }
        Ok(Self(value.seed))
    }
}
impl From<ManagerMapRandomMaster> for MasterWire {
    fn from(value: ManagerMapRandomMaster) -> Self {
        Self {
            version: 1,
            seed: value.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagerAdmissionReceipt {
    version: u32,
    id: StableActionId,
    payload_digest: PayloadDigest,
}

impl ManagerAdmissionReceipt {
    pub fn id(&self) -> &StableActionId {
        &self.id
    }
    pub fn payload_digest(&self) -> &PayloadDigest {
        &self.payload_digest
    }
}

#[derive(Serialize)]
enum TimerAdmission {
    PendingMovement { now_ms: u64 },
}

impl ZoneManager {
    pub fn bind_logical_world(&mut self, world: LogicalWorldId) -> Result<(), String> {
        match &self.logical_world {
            Some(current) if current != &world => {
                Err("manager logical world cannot be rebound".into())
            }
            Some(_) => Ok(()),
            None => {
                if self.admission_sequence != 0 || self.last_admission.is_some() {
                    return Err("unbound manager has admission history".into());
                }
                self.logical_world = Some(world);
                Ok(())
            }
        }
    }
    pub fn logical_world(&self) -> Option<&LogicalWorldId> {
        self.logical_world.as_ref()
    }
    pub fn last_admitted_action(&self) -> Option<&ManagerAdmissionReceipt> {
        self.last_admission.as_ref()
    }
    pub fn admitted_action_count(&self) -> u64 {
        self.admission_sequence
    }

    /// Call while holding the same single-writer gate as a prepared external
    /// commit. A read performed before releasing that gate is not a reservation.
    pub fn ensure_admission_capacity(&self, count: u64) -> Result<(), String> {
        if self.logical_world.is_some() && self.admission_sequence.checked_add(count).is_none() {
            return Err("manager admission sequence exhausted".into());
        }
        Ok(())
    }

    pub(super) fn admit_command(
        &mut self,
        key: &ZoneKey,
        command: &ZoneCommand,
    ) -> Result<(), String> {
        self.admit_serialized(std::slice::from_ref(key), command)
    }

    /// Explicit trusted operation kinds keep a prepared commit distinct from a
    /// raw command with coincidentally equal arguments. The tag is server code.
    pub(super) fn admit_special(
        &mut self,
        keys: &[ZoneKey],
        kind: &'static str,
        payload: &impl Serialize,
    ) -> Result<(), String> {
        self.admit_serialized(keys, &(1_u32, kind, payload))
    }

    pub(super) fn admit_timers(
        &mut self,
        keys: &[ZoneKey],
        now_ms: u64,
        movement_only: bool,
    ) -> Result<(), String> {
        if movement_only {
            self.admit_serialized(keys, &TimerAdmission::PendingMovement { now_ms })
        } else {
            self.admit_serialized(keys, &ZoneCommand::Tick { now_ms })
        }
    }

    fn admit_serialized(
        &mut self,
        keys: &[ZoneKey],
        payload: &impl Serialize,
    ) -> Result<(), String> {
        let Some(world) = self.logical_world.as_ref() else {
            return Ok(());
        };
        let count = u64::try_from(keys.len()).map_err(|_| "admission batch length exceeds u64")?;
        let next = self
            .admission_sequence
            .checked_add(count)
            .ok_or("manager admission sequence exhausted")?;
        if keys.is_empty() {
            return Ok(());
        }
        let bytes = serde_json::to_vec(payload).map_err(|e| format!("admission payload: {e}"))?;
        let digest = PayloadDigest::of_bytes(&bytes);
        let mut last = None;
        for (offset, key) in keys.iter().enumerate() {
            // Batch overflow was rejected before any admission or game state change.
            let sequence = self.admission_sequence + offset as u64 + 1;
            last = Some(ManagerAdmissionReceipt {
                version: 1,
                id: StableActionId::from_source_sequence(world.clone(), key.clone(), sequence)?,
                payload_digest: digest.clone(),
            });
        }
        self.admission_sequence = next;
        self.last_admission = last;
        Ok(())
    }
}

pub(super) fn validate_checkpoint_admission(
    checkpoint: &ZoneManagerCheckpoint,
) -> Result<(), String> {
    match (
        &checkpoint.logical_world,
        checkpoint.admission_sequence,
        &checkpoint.last_admission,
    ) {
        (None, 0, None) | (Some(_), 0, None) => Ok(()),
        (Some(world), count, Some(receipt))
            if receipt.version == 1
                && receipt.id.world() == world
                && receipt.id.source_sequence() == count =>
        {
            Ok(())
        }
        _ => Err("manager checkpoint admission identity is inconsistent".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exhausted() -> ZoneManager {
        let mut manager = ZoneManager::new();
        manager
            .bind_logical_world(LogicalWorldId::legacy_execution_domain("exhaustion").unwrap())
            .unwrap();
        let key = ZoneKey::for_map("exhaustion-map");
        manager.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("owner"),
            account_id: "owner-account".into(),
            character_index: 1,
            object_id: 101,
            name: "Owner".into(),
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 7,
            hp: 60,
            max_hp: 60,
            mp: 30,
            map_file_name: key.map_file_name.clone(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        manager.admission_sequence = u64::MAX;
        manager.last_admission = Some(ManagerAdmissionReceipt {
            version: 1,
            id: StableActionId::from_source_sequence(
                manager.logical_world.clone().unwrap(),
                key,
                u64::MAX,
            )
            .unwrap(),
            payload_digest: PayloadDigest::of_bytes(b"prior committed action"),
        });
        // Construct the exhaustion through a valid, authenticated v4 checkpoint.
        ZoneManager::restore_checkpoint(&manager.checkpoint_bytes().unwrap()).unwrap()
    }

    #[test]
    fn exhausted_manager_rejects_commands_and_all_timer_paths_before_any_change() {
        let mut manager = exhausted();
        let original = manager.checkpoint_bytes().unwrap();
        let partition = PartitionKey::whole_zone(ZoneKey::for_map("exhaustion-map"));
        assert!(manager
            .handle_for_partition(partition, ZoneCommand::Tick { now_ms: 99 })
            .is_err());
        assert_eq!(manager.checkpoint_bytes().unwrap(), original);
        assert!(manager.tick_all(100).is_empty());
        assert_eq!(manager.checkpoint_bytes().unwrap(), original);
        assert!(manager.tick_all_sequential(101).is_empty());
        assert_eq!(manager.checkpoint_bytes().unwrap(), original);
        assert!(manager.tick_pending_movement(102).is_empty());
        assert_eq!(manager.checkpoint_bytes().unwrap(), original);
        assert_eq!(manager.admitted_action_count(), u64::MAX);
    }

    #[test]
    fn batch_exhaustion_does_not_allocate_partial_timer_ids() {
        let mut manager = exhausted();
        manager.install_empty_zone(ZoneRuntime::new(ZoneKey::for_map("second-map")));
        manager.admission_sequence = u64::MAX - 1;
        manager.last_admission.as_mut().unwrap().id = StableActionId::from_source_sequence(
            manager.logical_world.clone().unwrap(),
            ZoneKey::for_map("exhaustion-map"),
            u64::MAX - 1,
        )
        .unwrap();
        let before = manager.checkpoint_bytes().unwrap();
        assert!(manager.tick_all(10).is_empty());
        assert_eq!(manager.checkpoint_bytes().unwrap(), before);
        assert!(manager.tick_pending_movement(11).is_empty());
        assert_eq!(manager.checkpoint_bytes().unwrap(), before);
    }

    #[test]
    fn specialized_commits_reject_exhaustion_before_vitals_or_spawn_mutation() {
        let mut manager = exhausted();
        let before = manager.checkpoint_bytes().unwrap();
        let session = SessionId::new("owner");
        assert!(manager.ensure_admission_capacity(1).is_err());
        assert!(manager
            .commit_owner_vitals(&session, 1, 60, 0, true)
            .is_empty());
        assert_eq!(manager.player_vitals(&session), Some((60, 60, 30)));
        assert_eq!(manager.checkpoint_bytes().unwrap(), before);
        assert!(manager
            .restore_player_saved_pets(
                &session,
                &super::super::super::ZoneSavedPetSnapshot::default(),
                10
            )
            .is_err());
        assert!(manager
            .sync_intelligent_creature(&session, None, Default::default(), Vec::new(), true, 10)
            .is_empty());
        assert!(!manager.request_intelligent_creature_pickup(
            &session,
            false,
            Point { x: 10, y: 10 }
        ));
        assert!(manager.refresh_online_pets_at(10).is_empty());
        let spawn = super::super::super::ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 900,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            image: 1,
            ai: 0,
            disposition: Some(crate::WorldEntityDisposition::Hostile),
            level: 1,
            max_hp: 20,
            hp: 20,
            experience: 1,
            move_speed_ms: 1000,
            attack_speed_ms: 1000,
            friendly_guild: None,
            position: Point { x: 12, y: 10 },
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        };
        assert!(
            !manager
                .spawn_world_event_monster(ZoneKey::for_map("new-map"), &spawn, 10)
                .0
        );
        assert_eq!(manager.checkpoint_bytes().unwrap(), before);
    }

    #[test]
    fn live_bulk_leave_is_atomic_on_exhaustion_and_cold_cleanup_keeps_the_watermark() {
        let mut manager = exhausted();
        let before = manager.checkpoint_bytes().unwrap();
        assert_eq!(manager.leave_all_sessions(), 0);
        assert_eq!(manager.checkpoint_bytes().unwrap(), before);
        let last = manager.last_admitted_action().unwrap().clone();
        assert_eq!(manager.clear_sessions_for_cold_world(), 1);
        assert_eq!(manager.admitted_action_count(), u64::MAX);
        assert_eq!(manager.last_admitted_action(), Some(&last));
        assert!(manager
            .zone_key_for_session(&SessionId::new("owner"))
            .is_none());
        assert_eq!(
            manager.zones[&ZoneKey::for_map("exhaustion-map")].player_count(),
            0
        );
        let restored =
            ZoneManager::restore_checkpoint(&manager.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(restored.logical_world(), manager.logical_world());
        assert_eq!(restored.admitted_action_count(), u64::MAX);
    }

    #[test]
    fn live_bulk_leave_rejects_a_partial_remaining_budget() {
        let mut manager = exhausted();
        manager.admission_sequence = u64::MAX - 2;
        manager.last_admission.as_mut().unwrap().id = StableActionId::from_source_sequence(
            manager.logical_world.clone().unwrap(),
            ZoneKey::for_map("exhaustion-map"),
            u64::MAX - 2,
        )
        .unwrap();
        manager.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("second-owner"),
            account_id: "second-account".into(),
            character_index: 2,
            object_id: 102,
            name: "Second".into(),
            class: mir2_protocol::MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 7,
            hp: 60,
            max_hp: 60,
            mp: 30,
            map_file_name: "exhaustion-map".into(),
            position: Point { x: 11, y: 10 },
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        assert_eq!(manager.admitted_action_count(), u64::MAX - 1);
        assert_eq!(manager.session_zones.len(), 2);
        let before = manager.checkpoint_bytes().unwrap();
        assert_eq!(manager.leave_all_sessions(), 0);
        assert_eq!(manager.checkpoint_bytes().unwrap(), before);
        assert_eq!(manager.clear_sessions_for_cold_world(), 2);
        assert_eq!(manager.admitted_action_count(), u64::MAX - 1);
    }
}

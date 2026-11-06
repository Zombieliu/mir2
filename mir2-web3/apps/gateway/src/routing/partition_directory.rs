//! Recoverable logical topology, independent of physical owners and their leases.
use super::*;
use mir2_simulation::{
    CellSize, LogicalWorldId, PartitionKey, TopologyRevision, WorldMapEffectOwner,
    WorldPartitionKey, ZonePartitionDescriptor,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalPartitionDirectoryRow {
    pub key: WorldPartitionKey,
    pub execution_domain: ZoneId,
    pub topology_revision: TopologyRevision,
    pub cell_size: CellSize,
    pub effect_owner: WorldMapEffectOwner,
}

impl GlobalPartitionDirectoryRow {
    fn from_descriptor(
        domain: &ZoneId,
        descriptor: ZonePartitionDescriptor,
    ) -> Result<Self, String> {
        let world = LogicalWorldId::legacy_execution_domain(domain.as_str())?;
        Ok(Self {
            key: WorldPartitionKey::new(world.clone(), descriptor.partition.clone()),
            execution_domain: domain.clone(),
            topology_revision: descriptor.topology_revision,
            cell_size: descriptor.cell_size,
            effect_owner: WorldMapEffectOwner::for_map(world, descriptor.partition.zone),
        })
    }
}

/// Only logical mapping is persisted. No host, transport, owner or write fence
/// may be imported from this record. Existing authority must revalidate a lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalPartitionDirectorySnapshot {
    version: u32,
    rows: Vec<GlobalPartitionDirectoryRow>,
    commitment: String,
}

impl GlobalPartitionDirectorySnapshot {
    pub fn rows(&self) -> &[GlobalPartitionDirectoryRow] {
        &self.rows
    }

    fn new(mut rows: Vec<GlobalPartitionDirectoryRow>) -> Result<Self, String> {
        rows.sort_by(|left, right| left.key.cmp(&right.key));
        if rows.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err("duplicate global partition identity".into());
        }
        let bytes = serde_json::to_vec(&(1_u32, &rows)).map_err(|e| e.to_string())?;
        let mut digest = Sha256::new();
        digest.update(b"mir2.global-partition-directory.v1\0");
        digest.update(bytes);
        let commitment = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok(Self {
            version: 1,
            rows,
            commitment,
        })
    }

    fn validate(&self, expected: &Self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported global partition directory version".into());
        }
        if self != expected {
            return Err(
                "global partition directory does not match the committed world topology".into(),
            );
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct ManagerDirectoryReadSet {
    version: u32,
    zones: Vec<(ZoneKey, serde::de::IgnoredAny)>,
}

pub(super) fn directory_from_checkpoints(
    zones: &BTreeMap<ZoneId, SharedInProcessZoneStateCheckpoint>,
) -> Result<GlobalPartitionDirectorySnapshot, String> {
    let mut rows = Vec::new();
    for (domain, checkpoint) in zones {
        // Reading key metadata does not cold-restore players, mint random
        // streams, clone world queues or activate any online authority.
        let manager: ManagerDirectoryReadSet =
            serde_json::from_slice(&checkpoint.zone_manager_bytes)
                .map_err(|error| format!("partition directory manager metadata: {error}"))?;
        if !matches!(manager.version, 2 | 3 | 4) {
            return Err("unsupported manager version in partition directory".into());
        }
        for (key, _) in manager.zones {
            rows.push(GlobalPartitionDirectoryRow::from_descriptor(
                domain,
                ZonePartitionDescriptor {
                    partition: PartitionKey::whole_zone(key.clone()),
                    topology_revision: TopologyRevision::WHOLE_ZONE,
                    cell_size: CellSize::WHOLE_ZONE,
                    effect_owner: mir2_simulation::MapEffectOwnerRef::for_zone(key),
                },
            )?);
        }
    }
    GlobalPartitionDirectorySnapshot::new(rows)
}

impl SharedInProcessZoneFactoryCheckpoint {
    pub(super) fn validate_partition_directory(&self) -> Result<(), String> {
        match self.version {
            2 if self.partition_directory.is_none() => Ok(()), // Explicit legacy migration.
            2 => Err("legacy factory checkpoint cannot inject a future partition directory".into()),
            SHARED_ZONE_FACTORY_CHECKPOINT_VERSION => self
                .partition_directory
                .as_ref()
                .ok_or_else(|| {
                    "factory checkpoint is missing its global partition directory".to_string()
                })?
                .validate(&directory_from_checkpoints(&self.zones)?),
            _ => Err(format!(
                "unsupported shared Zone factory checkpoint version {}",
                self.version
            )),
        }
    }
}

pub(super) fn bind_manager_world(manager: &mut ZoneManager, domain: &ZoneId) -> Result<(), String> {
    manager.bind_logical_world(LogicalWorldId::legacy_execution_domain(domain.as_str())?)
}

impl SharedInProcessZoneRuntimeFactory {
    pub fn global_partition_directory(&self) -> Result<GlobalPartitionDirectorySnapshot, String> {
        let resources = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex poisoned")?
            .clone();
        let mut rows = Vec::new();
        for (domain, resources) in resources {
            let state = resources
                .zone_state
                .lock()
                .map_err(|_| "partition directory state mutex poisoned")?;
            let expected_world = LogicalWorldId::legacy_execution_domain(domain.as_str())?;
            if state.zone_manager.logical_world() != Some(&expected_world) {
                return Err(
                    "partition manager logical world does not match its execution domain".into(),
                );
            }
            for descriptor in state.zone_manager.partition_directory() {
                rows.push(GlobalPartitionDirectoryRow::from_descriptor(
                    &domain, descriptor,
                )?);
            }
        }
        GlobalPartitionDirectorySnapshot::new(rows)
    }

    /// Unlike the legacy unscoped lookup this uniquely identifies equal map
    /// keys on different lines. Querying it never allocates or starts a world.
    pub fn global_partition_route(
        &self,
        key: &WorldPartitionKey,
    ) -> Result<Option<GlobalPartitionDirectoryRow>, String> {
        Ok(self
            .global_partition_directory()?
            .rows
            .into_iter()
            .find(|row| row.key == *key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> SharedInProcessZoneRuntimeFactory {
        let mut factory = SharedInProcessZoneRuntimeFactory::new();
        factory.autonomous_ticks_by_default = false;
        for domain in ["directory-a", "directory-z"] {
            factory
                .resources_for_zone(&ZoneId::new(domain))
                .zone_state
                .lock()
                .unwrap()
                .zone_manager
                .handle_for_key(
                    ZoneKey::for_map("directory-map"),
                    ZoneCommand::Tick { now_ms: 0 },
                );
        }
        factory
    }

    #[test]
    fn global_directory_is_committed_and_strictly_validated_before_restore() {
        let source = fixture();
        let bytes = source.checkpoint_bytes().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["version"], 3);
        assert_eq!(
            value["partitionDirectory"]["rows"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let target = fixture();
        let before = target.checkpoint_bytes().unwrap();
        let mut invalid = Vec::new();
        let mut changed = value.clone();
        changed
            .as_object_mut()
            .unwrap()
            .remove("partitionDirectory");
        invalid.push(changed);
        let mut changed = value.clone();
        changed["partitionDirectory"]["version"] = 2.into();
        invalid.push(changed);
        let mut changed = value.clone();
        changed["partitionDirectory"]["rows"][0]["execution_domain"] = "foreign".into();
        invalid.push(changed);
        let mut changed = value.clone();
        changed["partitionDirectory"]["rows"][0]["cell_size"] = 32.into();
        invalid.push(changed);
        let mut changed = value.clone();
        changed["partitionDirectory"]["rows"][0]["effect_owner"] = serde_json::Value::Null;
        invalid.push(changed);
        let mut changed = value.clone();
        let duplicate = changed["partitionDirectory"]["rows"][0].clone();
        changed["partitionDirectory"]["rows"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        invalid.push(changed);
        let mut changed = value.clone();
        changed["ownerId"] = "injected-physical-owner".into();
        invalid.push(changed);
        let mut changed = value.clone();
        changed["version"] = 2.into(); // Old schema cannot smuggle new fields.
        invalid.push(changed);
        for candidate in invalid {
            assert!(target
                .install_checkpoint_bytes(&serde_json::to_vec(&candidate).unwrap())
                .is_err());
            assert_eq!(target.checkpoint_bytes().unwrap(), before);
        }
        target.install_checkpoint_bytes(&bytes).unwrap();
        assert_eq!(
            target.global_partition_directory().unwrap(),
            source.global_partition_directory().unwrap()
        );
    }

    #[test]
    fn legacy_factory_v2_migrates_without_importing_owner_authority() {
        let source = fixture();
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&source.checkpoint_bytes().unwrap()).unwrap();
        legacy["version"] = 2.into();
        legacy.as_object_mut().unwrap().remove("partitionDirectory");
        let restored = fixture();
        restored
            .install_checkpoint_bytes(&serde_json::to_vec(&legacy).unwrap())
            .unwrap();
        assert_eq!(
            restored.global_partition_directory().unwrap(),
            source.global_partition_directory().unwrap()
        );
        let rewritten: serde_json::Value =
            serde_json::from_slice(&restored.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(rewritten["version"], 3);
        assert!(rewritten["partitionDirectory"]["rows"].is_array());
    }

    #[test]
    fn wrong_world_in_later_manager_does_not_partially_replace_live_state() {
        let target = fixture();
        let a = ZoneId::new("directory-a");
        let z = ZoneId::new("directory-z");
        let original_resource = target.resources_for_zone(&a);
        let before = target.checkpoint_bytes().unwrap();
        let mut candidate: SharedInProcessZoneFactoryCheckpoint =
            serde_json::from_slice(&before).unwrap();
        candidate.zones.get_mut(&a).unwrap().next_zone_object_id += 123;
        candidate.zones.get_mut(&z).unwrap().zone_manager_bytes =
            candidate.zones[&a].zone_manager_bytes.clone();
        let error = target
            .install_checkpoint_bytes(&serde_json::to_vec(&candidate).unwrap())
            .unwrap_err();
        assert!(error.contains("cannot be rebound"), "{error}");
        assert_eq!(target.checkpoint_bytes().unwrap(), before);
        assert!(Arc::ptr_eq(
            &original_resource.zone_state,
            &target.resources_for_zone(&a).zone_state
        ));
    }
}

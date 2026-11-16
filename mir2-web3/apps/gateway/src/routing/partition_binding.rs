//! Read-only projection from logical partitions to the existing execution domain.
//!
//! These values neither install owners nor authorize mutations. Owner fields are
//! observed only after the existing lease authority validates the supplied lease.
//! No owner observation is serialized into a world checkpoint or kept as a live
//! authority cache; cold restore therefore cannot activate a snapshot's owner.

use super::*;
use mir2_simulation::{
    LogicalWorldId, ManagerAdmissionReceipt, PartitionKey, WorldPartitionKey,
    ZonePartitionDescriptor,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartitionRouteBinding {
    descriptor: ZonePartitionDescriptor,
    execution_domain: ZoneId,
}

impl PartitionRouteBinding {
    pub fn descriptor(&self) -> &ZonePartitionDescriptor {
        &self.descriptor
    }

    pub fn partition(&self) -> &PartitionKey {
        &self.descriptor.partition
    }

    pub fn execution_domain(&self) -> &ZoneId {
        &self.execution_domain
    }

    pub fn global_partition_key(&self) -> Result<WorldPartitionKey, String> {
        Ok(WorldPartitionKey::new(
            LogicalWorldId::legacy_execution_domain(self.execution_domain.as_str())?,
            self.descriptor.partition.clone(),
        ))
    }
}

/// Point-in-time observation, not a validation receipt or write capability.
/// Callers must still validate the lease at every existing mutation boundary.
/// Private fields and no Deserialize prevent importing a snapshot as authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartitionOwnerRef {
    execution_domain: ZoneId,
    owner_id: String,
    fencing_token: u64,
}

impl PartitionOwnerRef {
    pub fn execution_domain(&self) -> &ZoneId {
        &self.execution_domain
    }

    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub fn fencing_token(&self) -> u64 {
        self.fencing_token
    }

    fn observe(
        authority: &dyn ZoneOwnerLeaseAuthority,
        lease: &ZoneOwnerLease,
    ) -> Result<Self, String> {
        authority.validate_owner_lease(lease)?;
        Ok(Self {
            execution_domain: lease.zone_id().clone(),
            owner_id: lease.owner_id().to_string(),
            fencing_token: lease.fencing_token(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PartitionExecutionBinding {
    route: PartitionRouteBinding,
    owner: PartitionOwnerRef,
}

impl PartitionExecutionBinding {
    pub fn route(&self) -> &PartitionRouteBinding {
        &self.route
    }

    pub fn owner(&self) -> &PartitionOwnerRef {
        &self.owner
    }
}

pub(super) fn validate_runtime_execution_domain(
    runtime: &ZoneRuntimeHandle,
    lease: &ZoneOwnerLease,
) -> Result<(), String> {
    if let Some(shared) = runtime
        .as_ref()
        .as_any()
        .downcast_ref::<SharedInProcessZoneSessionRuntime>()
    {
        validate_execution_domain(&shared.inventory_zone_id, lease.zone_id())?;
    }
    Ok(())
}

fn validate_execution_domain(expected: &ZoneId, supplied: &ZoneId) -> Result<(), String> {
    if expected != supplied {
        return Err(format!(
            "partition execution domain mismatch: runtime belongs to {expected}, lease belongs to {supplied}"
        ));
    }
    Ok(())
}

fn route_for_partition(
    manager: &ZoneManager,
    execution_domain: &ZoneId,
    partition: &PartitionKey,
) -> Result<PartitionRouteBinding, String> {
    let descriptor = manager
        .partition_descriptor(&partition.zone)
        .filter(|descriptor| descriptor.partition == *partition)
        .ok_or_else(|| "partition is absent from this execution domain".to_string())?;
    Ok(PartitionRouteBinding {
        descriptor,
        execution_domain: execution_domain.clone(),
    })
}

impl HostedZoneOwnerCommandClient {
    /// Inspect the canonical simulation admission stream. RPC journal/economy
    /// sequences are distinct and must never be relabelled as game ActionIds.
    pub fn last_partition_admission(
        &self,
        lease: &ZoneOwnerLease,
    ) -> Result<Option<ManagerAdmissionReceipt>, String> {
        let authority = self
            .owner_lease_authority
            .as_ref()
            .ok_or("partition admission observation requires a lease authority")?;
        authority.validate_owner_lease(lease)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "hosted runtime mutex poisoned")?;
        let shared = runtime
            .as_ref()
            .ok_or("runtime handed off")?
            .as_ref()
            .as_any()
            .downcast_ref::<SharedInProcessZoneSessionRuntime>()
            .ok_or("runtime has no shared admission stream")?;
        validate_execution_domain(&shared.inventory_zone_id, lease.zone_id())?;
        let state = shared
            .zone_state
            .lock()
            .map_err(|_| "shared admission mutex poisoned")?;
        Ok(state.zone_manager.last_admitted_action().cloned())
    }

    /// Resolve an existing logical partition under a freshly validated lease.
    /// The command path's original lease checks remain the mutation authority.
    pub fn partition_binding(
        &self,
        lease: &ZoneOwnerLease,
        partition: &PartitionKey,
    ) -> Result<PartitionExecutionBinding, String> {
        let authority = self.owner_lease_authority.as_ref().ok_or_else(|| {
            "partition owner observation requires an owner lease authority".to_string()
        })?;
        let owner = PartitionOwnerRef::observe(authority.as_ref(), lease)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_ref()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        let shared = runtime
            .as_ref()
            .as_any()
            .downcast_ref::<SharedInProcessZoneSessionRuntime>()
            .ok_or_else(|| "runtime does not expose a shared partition directory".to_string())?;
        validate_execution_domain(&shared.inventory_zone_id, lease.zone_id())?;
        let state = shared
            .zone_state
            .lock()
            .map_err(|_| "shared partition directory mutex was poisoned".to_string())?;
        let route = route_for_partition(&state.zone_manager, &shared.inventory_zone_id, partition)?;
        Ok(PartitionExecutionBinding { route, owner })
    }

    /// Query the actor admitted by the real Join path. A personal snapshot alone
    /// cannot produce an active partition or resurrect an owner after restore.
    pub fn active_partition_binding(
        &self,
        lease: &ZoneOwnerLease,
    ) -> Result<Option<PartitionExecutionBinding>, String> {
        let authority = self.owner_lease_authority.as_ref().ok_or_else(|| {
            "partition owner observation requires an owner lease authority".to_string()
        })?;
        let owner = PartitionOwnerRef::observe(authority.as_ref(), lease)?;
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "zone owner hosted runtime mutex was poisoned".to_string())?;
        let runtime = runtime
            .as_ref()
            .ok_or_else(|| "zone owner hosted runtime was already handed off".to_string())?;
        let shared = runtime
            .as_ref()
            .as_any()
            .downcast_ref::<SharedInProcessZoneSessionRuntime>()
            .ok_or_else(|| "runtime does not expose a shared partition directory".to_string())?;
        validate_execution_domain(&shared.inventory_zone_id, lease.zone_id())?;
        // Match authoritative movement's state -> ingress lock order.
        let state = shared
            .zone_state
            .lock()
            .map_err(|_| "shared partition directory mutex was poisoned".to_string())?;
        let movement = shared
            .movement_ingress
            .session_state
            .lock()
            .map_err(|_| "partition movement presence mutex was poisoned".to_string())?;
        let Some(session) = movement
            .presence_key
            .as_ref()
            .and_then(|presence| state.zone_sessions.get(presence))
        else {
            return Ok(None);
        };
        let partition = state
            .zone_manager
            .partition_for_session(session)
            .ok_or_else(|| "admitted actor has no authoritative partition".to_string())?;
        let route =
            route_for_partition(&state.zone_manager, &shared.inventory_zone_id, &partition)?;
        Ok(Some(PartitionExecutionBinding { route, owner }))
    }
}

impl SharedInProcessZoneRuntimeFactory {
    /// Resolve within this factory's observed domains. Legacy hot-map lines can
    /// contain equal logical ZoneKeys; reject that ambiguity rather than choose
    /// the first host. This read projection is not a global placement authority.
    pub fn partition_route(
        &self,
        partition: &PartitionKey,
    ) -> Result<Option<PartitionRouteBinding>, String> {
        let resources = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
            .iter()
            .map(|(domain, resources)| (domain.clone(), resources.clone()))
            .collect::<Vec<_>>();
        let mut resolved: Option<PartitionRouteBinding> = None;
        for (execution_domain, resources) in resources {
            let state = resources
                .zone_state
                .lock()
                .map_err(|_| "shared partition directory mutex was poisoned".to_string())?;
            let Some(descriptor) = state
                .zone_manager
                .partition_descriptor(&partition.zone)
                .filter(|descriptor| descriptor.partition == *partition)
            else {
                continue;
            };
            if resolved.is_some() {
                return Err(
                    "ambiguous logical partition occurs in multiple execution domains".into(),
                );
            }
            resolved = Some(PartitionRouteBinding {
                descriptor,
                execution_domain,
            });
        }
        Ok(resolved)
    }

    /// A derived route projection, including after checkpoint restore. It does
    /// not allocate missing resources, install topology, or observe an owner.
    pub fn partition_routes(
        &self,
        execution_domain: &ZoneId,
    ) -> Result<Vec<PartitionRouteBinding>, String> {
        let resources = self
            .zones
            .lock()
            .map_err(|_| "shared Zone factory mutex was poisoned".to_string())?
            .get(execution_domain)
            .cloned();
        let Some(resources) = resources else {
            return Ok(Vec::new());
        };
        let state = resources
            .zone_state
            .lock()
            .map_err(|_| "shared partition directory mutex was poisoned".to_string())?;
        Ok(state
            .zone_manager
            .partition_directory()
            .into_iter()
            .map(|descriptor| PartitionRouteBinding {
                descriptor,
                execution_domain: execution_domain.clone(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{MirClass, MirGender};
    use mir2_simulation::PartitionId;

    fn factory() -> SharedInProcessZoneRuntimeFactory {
        let mut factory = SharedInProcessZoneRuntimeFactory::new();
        factory.autonomous_ticks_by_default = false;
        factory
    }

    fn execute(
        hosted: &HostedZoneOwnerCommandClient,
        lease: &ZoneOwnerLease,
        authenticated: bool,
        packet: ClientPacket,
    ) -> Vec<ServerPacket> {
        hosted
            .execute_request(ZoneOwnerCommandRequest::production_player(
                lease.clone(),
                authenticated,
                WorldCommand::ClientPacket(packet),
            ))
            .unwrap()
            .packets
    }

    fn join_player(hosted: &HostedZoneOwnerCommandClient, lease: &ZoneOwnerLease) {
        execute(
            hosted,
            lease,
            false,
            ClientPacket::NewAccount {
                account_id: "p1bind1".into(),
                password: "p1bind1".into(),
                birth_date_binary: 0,
                user_name: String::new(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            },
        );
        execute(
            hosted,
            lease,
            false,
            ClientPacket::Login {
                account_id: "p1bind1".into(),
                password: "p1bind1".into(),
            },
        );
        let character_index = execute(
            hosted,
            lease,
            true,
            ClientPacket::NewCharacter {
                name: "OwnerOne".into(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            },
        )
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("character creation must succeed on the normal owner request path");
        execute(
            hosted,
            lease,
            true,
            ClientPacket::StartGame { character_index },
        );
    }

    #[test]
    fn partition_binding_real_join_projects_existing_domain_and_validated_owner() {
        let factory = factory();
        let domain = ZoneId::new("p1-owner-domain");
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let lease = authority.handoff_zone_owner(&domain, "owner-host-a");
        let hosted = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            factory.create_runtime(GatewayConfig::default(), &domain),
            authority.clone(),
        );
        assert!(hosted.active_partition_binding(&lease).unwrap().is_none());
        join_player(&hosted, &lease);
        let binding = hosted.active_partition_binding(&lease).unwrap().unwrap();
        assert_eq!(binding.route().execution_domain(), &domain);
        assert_eq!(
            binding.route().partition().partition_id,
            PartitionId::WHOLE_ZONE
        );
        assert_eq!(binding.owner().execution_domain(), &domain);
        assert_eq!(binding.owner().owner_id(), lease.owner_id());
        assert_eq!(binding.owner().fencing_token(), lease.fencing_token());
        assert_eq!(
            hosted
                .partition_binding(&lease, binding.route().partition())
                .unwrap(),
            binding
        );
        assert!(factory
            .partition_routes(&domain)
            .unwrap()
            .iter()
            .any(|route| route == binding.route()));
        assert_eq!(
            factory
                .partition_route(binding.route().partition())
                .unwrap(),
            Some(binding.route().clone())
        );
        assert!(factory
            .partition_routes(&ZoneId::new("missing"))
            .unwrap()
            .is_empty());
        assert_eq!(factory.active_zone_count(), 1);
    }

    #[test]
    fn partition_binding_rejects_wrong_execution_domain_before_real_join_mutation() {
        let factory = factory();
        let domain = ZoneId::new("p1-correct-domain");
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let wrong_lease = authority.owner_lease(&ZoneId::new("p1-other-domain"));
        let hosted = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            factory.create_runtime(GatewayConfig::default(), &domain),
            authority,
        );
        let before = factory.zone_checkpoint_bytes(&domain).unwrap();
        let result = hosted.execute_request(ZoneOwnerCommandRequest::production_player(
            wrong_lease.clone(),
            true,
            WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
        ));
        assert!(result
            .unwrap_err()
            .contains("partition execution domain mismatch"));
        assert!(hosted
            .active_partition_binding(&wrong_lease)
            .unwrap_err()
            .contains("partition execution domain mismatch"));
        assert_eq!(factory.zone_checkpoint_bytes(&domain).unwrap(), before);
    }

    #[test]
    fn partition_binding_in_process_requests_share_the_domain_guard() {
        let factory = factory();
        let domain = ZoneId::new("p1-local-domain");
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let wrong_lease = authority.owner_lease(&ZoneId::new("p1-local-other"));
        let client = InProcessZoneOwnerCommandClient::with_owner_lease_authority(authority);
        let mut runtime = factory.create_runtime(GatewayConfig::default(), &domain);
        let before = factory.zone_checkpoint_bytes(&domain).unwrap();
        let result = client.execute(
            &mut runtime,
            ZoneOwnerCommandRequest::production_player(
                wrong_lease,
                true,
                WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
            ),
        );
        assert!(result
            .unwrap_err()
            .contains("partition execution domain mismatch"));
        assert_eq!(factory.zone_checkpoint_bytes(&domain).unwrap(), before);
    }

    #[test]
    fn partition_binding_stale_lease_and_unknown_partition_do_not_mutate_state() {
        let factory = factory();
        let domain = ZoneId::new("p1-fence-domain");
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let old_lease = authority.handoff_zone_owner(&domain, "old-owner");
        let hosted = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            factory.create_runtime(GatewayConfig::default(), &domain),
            authority.clone(),
        );
        join_player(&hosted, &old_lease);
        let old_binding = hosted
            .active_partition_binding(&old_lease)
            .unwrap()
            .unwrap();
        let current = authority.handoff_zone_owner(&domain, "new-owner");
        let before = factory.zone_checkpoint_bytes(&domain).unwrap();
        assert!(hosted
            .partition_binding(&old_lease, old_binding.route().partition())
            .unwrap_err()
            .contains("stale zone owner lease"));
        let mut unknown = old_binding.route().partition().clone();
        unknown.partition_id = PartitionId(9);
        assert!(hosted.partition_binding(&current, &unknown).is_err());
        let fresh = hosted
            .partition_binding(&current, old_binding.route().partition())
            .unwrap();
        assert_eq!(fresh.owner().fencing_token(), current.fencing_token());
        assert_eq!(fresh.owner().owner_id(), "new-owner");
        assert_eq!(factory.zone_checkpoint_bytes(&domain).unwrap(), before);
    }

    #[test]
    fn partition_binding_cold_restore_derives_routes_without_reactivating_snapshot_owner() {
        let source = factory();
        let domain = ZoneId::new("p1-restore-domain");
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let old_lease = authority.handoff_zone_owner(&domain, "snapshot-owner");
        let hosted = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            source.create_runtime(GatewayConfig::default(), &domain),
            authority.clone(),
        );
        join_player(&hosted, &old_lease);
        let partition = hosted
            .active_partition_binding(&old_lease)
            .unwrap()
            .unwrap()
            .route()
            .partition()
            .clone();
        let bytes = source.checkpoint_bytes().unwrap();
        let restored = factory();
        restored.install_checkpoint_bytes(&bytes).unwrap();
        assert_eq!(
            restored.partition_routes(&domain).unwrap(),
            source.partition_routes(&domain).unwrap()
        );
        let current = authority.handoff_zone_owner(&domain, "restored-owner");
        let restored_host = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            restored.create_runtime(GatewayConfig::default(), &domain),
            authority,
        );
        let before = restored.zone_checkpoint_bytes(&domain).unwrap();
        assert!(restored_host
            .active_partition_binding(&current)
            .unwrap()
            .is_none());
        assert!(restored_host
            .partition_binding(&old_lease, &partition)
            .is_err());
        let observed = restored_host
            .partition_binding(&current, &partition)
            .unwrap();
        assert_eq!(observed.owner().owner_id(), "restored-owner");
        assert_eq!(observed.owner().fencing_token(), current.fencing_token());
        assert_eq!(restored.zone_checkpoint_bytes(&domain).unwrap(), before);
    }

    #[test]
    fn partition_binding_unvalidated_host_cannot_observe_an_active_owner() {
        let factory = factory();
        let domain = ZoneId::new("p1-no-authority");
        let lease = ZoneOwnerLease::in_process(&domain);
        let hosted = HostedZoneOwnerCommandClient::new(
            factory.create_runtime(GatewayConfig::default(), &domain),
        );
        assert!(hosted
            .active_partition_binding(&lease)
            .unwrap_err()
            .contains("requires an owner lease authority"));
    }

    #[test]
    fn partition_binding_factory_rejects_legacy_cross_domain_identity_ambiguity() {
        let factory = factory();
        let partition = PartitionKey::whole_zone(ZoneKey::for_map("p1-shared-logical-key"));
        assert!(factory.partition_route(&partition).unwrap().is_none());
        assert_eq!(factory.active_zone_count(), 0);
        for domain in ["line-1", "line-2"] {
            factory
                .resources_for_zone(&ZoneId::new(domain))
                .zone_state
                .lock()
                .unwrap()
                .zone_manager
                .handle_for_key(partition.zone.clone(), ZoneCommand::Tick { now_ms: 0 });
        }
        assert!(factory
            .partition_route(&partition)
            .unwrap_err()
            .contains("ambiguous logical partition"));
        let global = factory.global_partition_directory().unwrap();
        assert_eq!(global.rows().len(), 2);
        assert_ne!(global.rows()[0].key, global.rows()[1].key);
        for row in global.rows() {
            assert_eq!(row.key.partition(), &partition);
            assert_eq!(
                factory.global_partition_route(&row.key).unwrap().as_ref(),
                Some(row)
            );
        }
        assert_eq!(factory.active_zone_count(), 2);
    }

    #[test]
    fn partition_binding_admission_is_stable_across_owner_handoff_and_cold_restore() {
        let source = factory();
        let domain = ZoneId::new("p1-admission-owner-domain");
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let first = authority.handoff_zone_owner(&domain, "host-a");
        let hosted = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            source.create_runtime(GatewayConfig::default(), &domain),
            authority.clone(),
        );
        assert!(hosted.last_partition_admission(&first).unwrap().is_none());
        join_player(&hosted, &first);
        let before = hosted.last_partition_admission(&first).unwrap().unwrap();
        let binding = hosted.active_partition_binding(&first).unwrap().unwrap();
        assert_eq!(
            before.id().world(),
            binding.route().global_partition_key().unwrap().world()
        );
        assert_eq!(before.id().zone(), &binding.route().partition().zone);
        assert!(before.id().source_sequence() > 0);
        let current = authority.handoff_zone_owner(&domain, "host-b");
        assert!(hosted
            .last_partition_admission(&first)
            .unwrap_err()
            .contains("stale zone owner lease"));
        assert_eq!(
            hosted.last_partition_admission(&current).unwrap(),
            Some(before.clone())
        );
        let checkpoint = source.checkpoint_bytes().unwrap();
        let restored = factory();
        restored.install_checkpoint_bytes(&checkpoint).unwrap();
        let cold_host = HostedZoneOwnerCommandClient::with_owner_lease_authority(
            restored.create_runtime(GatewayConfig::default(), &domain),
            authority,
        );
        assert_eq!(
            cold_host.last_partition_admission(&current).unwrap(),
            Some(before)
        );
        assert!(cold_host
            .active_partition_binding(&current)
            .unwrap()
            .is_none());
        assert_eq!(
            restored.global_partition_directory().unwrap(),
            source.global_partition_directory().unwrap()
        );
    }
}

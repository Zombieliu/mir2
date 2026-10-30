//! Routing-only evidence. The PostgreSQL-shaped config is never connected;
//! dispatch probes stop before gameplay or persistence and never claim a commit.
use super::*;

use mir2_simulation::{AccountStoreDatabaseMode, ZoneSaveFence};

const PROBE_STOP: &str = "zone-save-fence test dispatch observed; no persistence attempted";

fn lease(token: u64) -> ZoneOwnerLease {
    ZoneOwnerLease::new(ZoneId::new("zone-save-fence-test"), "test-owner", token)
}

fn expected_fence(lease: &ZoneOwnerLease) -> ZoneSaveFence {
    ZoneSaveFence::new(
        lease.zone_id().as_str().into(),
        lease.owner_id().into(),
        lease.fencing_token(),
    )
    .unwrap()
}

fn postgres_shaped_config() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    config.account_store_database_url =
        Some("postgresql://zone-save-fence-routing-test.invalid/test".into());
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config
}

struct ExactAuthority {
    current: Mutex<ZoneOwnerLease>,
    transactional: bool,
    validations: Mutex<Vec<ZoneOwnerLease>>,
    lookups: AtomicU64,
    outer_guards: AtomicU64,
}

impl ExactAuthority {
    fn new(lease: ZoneOwnerLease, transactional: bool) -> Arc<Self> {
        Arc::new(Self {
            current: Mutex::new(lease),
            transactional,
            validations: Mutex::new(Vec::new()),
            lookups: AtomicU64::new(0),
            outer_guards: AtomicU64::new(0),
        })
    }
}

impl ZoneOwnerLeaseAuthority for ExactAuthority {
    fn requires_transactional_zone_save_fence(&self) -> bool {
        self.transactional
    }

    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        let current = self.current.lock().unwrap().clone();
        assert_eq!(current.zone_id(), zone_id);
        current
    }

    fn validate_owner_lease(&self, supplied: &ZoneOwnerLease) -> Result<(), String> {
        self.validations.lock().unwrap().push(supplied.clone());
        let current = self.current.lock().unwrap();
        if *current != *supplied {
            return Err(stale_zone_owner_lease_error(supplied, &current));
        }
        Ok(())
    }

    fn supports_durable_npc_purchase_lease(&self) -> bool {
        true
    }

    fn with_durable_npc_purchase_lease(
        &self,
        supplied: &ZoneOwnerLease,
        operation: &mut (dyn FnMut() + Send),
    ) -> Result<(), String> {
        assert!(
            !self.transactional,
            "transactional fence must not acquire the separate-connection outer guard"
        );
        self.outer_guards.fetch_add(1, Ordering::SeqCst);
        self.validate_owner_lease(supplied)?;
        operation();
        Ok(())
    }
}

type Observations = Arc<Mutex<Vec<(&'static str, ZoneSaveFence)>>>;

struct DispatchProbe {
    inner: InProcessWorldRuntime,
    observations: Observations,
    binding_supported: bool,
}

impl DispatchProbe {
    fn boxed(config: GatewayConfig, binding_supported: bool) -> (ZoneRuntimeHandle, Observations) {
        let observations = Arc::new(Mutex::new(Vec::new()));
        (
            Box::new(Self {
                inner: InProcessWorldRuntime::new(config),
                observations: observations.clone(),
                binding_supported,
            }),
            observations,
        )
    }

    fn observe(&self, operation: &'static str) -> Result<(), String> {
        let fence = self
            .inner
            .bound_zone_save_fence()
            .expect("routing must bind the authoritative runtime config before dispatch");
        self.observations.lock().unwrap().push((operation, fence));
        Err(PROBE_STOP.into())
    }
}

impl WorldRuntime for DispatchProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn supports_zone_save_fence(&self) -> bool {
        self.binding_supported
    }
    fn bind_zone_save_fence(&mut self, fence: ZoneSaveFence) -> Result<(), String> {
        if !self.binding_supported {
            return Err("test runtime does not support zone save fencing".into());
        }
        self.inner.bind_zone_save_fence(fence)
    }
    fn bound_zone_save_fence(&self) -> Option<ZoneSaveFence> {
        self.inner.bound_zone_save_fence()
    }
    fn on_connect(&self) -> Vec<ServerPacket> {
        Vec::new()
    }
    fn execute(&mut self, _: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        self.observe("execute")?;
        unreachable!()
    }
    fn world_snapshot(&self) -> WorldSnapshot {
        self.inner.world_snapshot()
    }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        None
    }
    fn save_active_character(&mut self) -> Result<(), String> {
        self.observe("save")
    }
    fn prepare_character_handoff(&mut self) -> Result<CharacterHandoffCheckpoint, String> {
        self.observe("prepare-save")?;
        unreachable!()
    }
    fn refresh_active_external_mail(&mut self) -> bool {
        false
    }
}

fn command(lease: ZoneOwnerLease) -> ZoneOwnerCommandRequest {
    ZoneOwnerCommandRequest::direct(
        lease,
        WorldCommand::SetLanguage {
            language: "en".into(),
        },
    )
}

#[test]
fn zone_save_fence_in_process_and_hosted_bind_before_execute_and_save_dispatch() {
    let current = lease(17);
    let authority = ExactAuthority::new(current.clone(), true);
    let authority_handle: SharedZoneOwnerLeaseAuthority = authority.clone();
    let client =
        InProcessZoneOwnerCommandClient::with_owner_lease_authority(authority_handle.clone());
    let (mut runtime, observations) = DispatchProbe::boxed(postgres_shaped_config(), true);
    assert_eq!(
        client
            .execute(&mut runtime, command(current.clone()))
            .unwrap_err(),
        PROBE_STOP
    );
    assert_eq!(
        observations.lock().unwrap().as_slice(),
        &[("execute", expected_fence(&current))]
    );

    let (hosted_runtime, hosted_observations) =
        DispatchProbe::boxed(postgres_shaped_config(), true);
    let hosted =
        HostedZoneOwnerCommandClient::with_owner_lease_authority(hosted_runtime, authority_handle);
    assert_eq!(
        ZoneOwnerRpcTransport::save_active_character(&hosted).unwrap_err(),
        "ZONE_SAVE_FENCE_NOT_BOUND"
    );
    assert!(hosted_observations.lock().unwrap().is_empty());
    assert_eq!(
        hosted
            .execute_request(command(current.clone()))
            .unwrap_err(),
        PROBE_STOP
    );
    assert_eq!(
        ZoneOwnerRpcTransport::save_active_character(&hosted).unwrap_err(),
        PROBE_STOP
    );
    assert_eq!(
        hosted.prepare_character_handoff(&current).unwrap_err(),
        PROBE_STOP
    );
    assert_eq!(
        hosted_observations.lock().unwrap().as_slice(),
        &[
            ("execute", expected_fence(&current)),
            ("save", expected_fence(&current)),
            ("prepare-save", expected_fence(&current)),
        ]
    );
    assert_eq!(authority.outer_guards.load(Ordering::SeqCst), 0);
}

#[test]
fn zone_save_fence_stale_unsupported_and_file_runtimes_never_dispatch() {
    let current = lease(18);
    let authority = ExactAuthority::new(current.clone(), true);
    let authority_handle: SharedZoneOwnerLeaseAuthority = authority;
    let client = InProcessZoneOwnerCommandClient::with_owner_lease_authority(authority_handle);
    let mut file_config = GatewayConfig::default();
    file_config.account_store_path =
        Some(std::path::PathBuf::from("unused-zone-save-fence-test.json"));
    let cases = [
        (postgres_shaped_config(), true, lease(17)),
        (postgres_shaped_config(), false, current.clone()),
        (file_config, true, current),
    ];
    for (config, supported, supplied) in cases {
        let (mut runtime, observations) = DispatchProbe::boxed(config, supported);
        assert!(client.execute(&mut runtime, command(supplied)).is_err());
        assert!(observations.lock().unwrap().is_empty());
        assert_eq!(runtime.bound_zone_save_fence(), None);
    }
}

#[test]
fn zone_save_fence_bound_validation_rejects_old_generation_without_adopting_current() {
    let original = lease(17);
    let authority = ExactAuthority::new(original.clone(), true);
    let authority_handle: SharedZoneOwnerLeaseAuthority = authority.clone();
    let mut runtime: ZoneRuntimeHandle =
        Box::new(InProcessWorldRuntime::new(postgres_shaped_config()));
    assert_eq!(
        validate_bound_runtime_zone_save_fence(&runtime, Some(&authority_handle)),
        Err("ZONE_SAVE_FENCE_NOT_BOUND".into())
    );
    bind_runtime_zone_save_fence(&mut runtime, Some(&authority_handle), &original).unwrap();
    validate_bound_runtime_zone_save_fence(&runtime, Some(&authority_handle)).unwrap();
    *authority.current.lock().unwrap() = lease(18);
    assert!(validate_bound_runtime_zone_save_fence(&runtime, Some(&authority_handle)).is_err());
    assert!(
        bind_runtime_zone_save_fence(&mut runtime, Some(&authority_handle), &original).is_err()
    );
    assert_eq!(
        runtime.bound_zone_save_fence(),
        Some(expected_fence(&original))
    );
    assert_eq!(authority.lookups.load(Ordering::SeqCst), 0);
    assert!(authority
        .validations
        .lock()
        .unwrap()
        .iter()
        .all(|value| value == &original));
}

#[test]
fn zone_save_fence_transactional_guard_calls_once_and_never_uses_outer_lock() {
    let current = lease(17);
    let authority = ExactAuthority::new(current.clone(), true);
    let authority_handle: SharedZoneOwnerLeaseAuthority = authority.clone();
    let callbacks = AtomicU64::new(0);
    with_owner_operation_guard(&authority_handle, &current, &mut || {
        callbacks.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();
    assert_eq!(callbacks.load(Ordering::SeqCst), 1);
    assert_eq!(authority.outer_guards.load(Ordering::SeqCst), 0);
    *authority.current.lock().unwrap() = lease(18);
    assert!(
        with_owner_operation_guard(&authority_handle, &current, &mut || {
            callbacks.fetch_add(1, Ordering::SeqCst);
        })
        .is_err()
    );
    assert_eq!(callbacks.load(Ordering::SeqCst), 1);
}

#[test]
fn zone_save_fence_legacy_authority_still_uses_outer_guard() {
    let current = lease(17);
    let authority = ExactAuthority::new(current.clone(), false);
    let authority_handle: SharedZoneOwnerLeaseAuthority = authority.clone();
    let callbacks = AtomicU64::new(0);
    with_owner_operation_guard(&authority_handle, &current, &mut || {
        callbacks.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();
    assert_eq!(callbacks.load(Ordering::SeqCst), 1);
    assert_eq!(authority.outer_guards.load(Ordering::SeqCst), 1);
    let mut runtime: ZoneRuntimeHandle =
        Box::new(InProcessWorldRuntime::new(GatewayConfig::default()));
    bind_runtime_zone_save_fence(&mut runtime, Some(&authority_handle), &current).unwrap();
    assert_eq!(runtime.bound_zone_save_fence(), None);
    validate_bound_runtime_zone_save_fence(&runtime, Some(&authority_handle)).unwrap();
}

#[test]
fn zone_save_fence_shared_world_runtime_delegates_binding_to_personal_runtime_config() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut shared = super::tests::shared_session_runtime(state);
    shared.inner = InProcessWorldRuntime::new(postgres_shaped_config());
    let mut runtime: ZoneRuntimeHandle = Box::new(shared);
    let current = lease(17);
    let authority: SharedZoneOwnerLeaseAuthority = ExactAuthority::new(current.clone(), true);
    assert!(runtime.supports_zone_save_fence());
    bind_runtime_zone_save_fence(&mut runtime, Some(&authority), &current).unwrap();
    let shared = runtime
        .as_any()
        .downcast_ref::<SharedInProcessZoneSessionRuntime>()
        .unwrap();
    assert_eq!(
        runtime.bound_zone_save_fence(),
        Some(expected_fence(&current))
    );
    assert_eq!(
        shared.inner.bound_zone_save_fence(),
        runtime.bound_zone_save_fence()
    );
    validate_bound_runtime_zone_save_fence(&runtime, Some(&authority)).unwrap();
}

//! Coordination failure must freeze work without poisoning the owner loop.
use super::*;
use crate::routing::{
    InProcessZoneRuntimeFactory, SessionRouteRequest, ZoneOwnerLeaseAuthority, ZoneRegistry,
};
use crate::GatewaySession;
use mir2_simulation::{ZoneOwnerHealthChange, ZoneOwnerHealthCursor, ZoneRuntimeHandle};

struct FaultAuthority {
    unavailable: AtomicBool,
    calls: AtomicUsize,
}

impl FaultAuthority {
    fn new(unavailable: bool) -> Self {
        Self {
            unavailable: AtomicBool::new(unavailable),
            calls: AtomicUsize::new(0),
        }
    }
}

impl ZoneOwnerLeaseAuthority for FaultAuthority {
    fn owner_lease(&self, _: &ZoneId) -> ZoneOwnerLease {
        panic!("runtime paths must use the fallible lease API")
    }

    fn refresh_owner_lease(&self, _: &ZoneId) -> ZoneOwnerLease {
        panic!("runtime refresh must use the fallible lease API")
    }

    fn try_owner_lease(&self, zone: &ZoneId) -> Result<ZoneOwnerLease, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.unavailable.load(Ordering::SeqCst) {
            Err("coordination_unavailable".into())
        } else {
            Ok(ZoneOwnerLease::in_process(zone))
        }
    }

    fn try_refresh_owner_lease(&self, zone: &ZoneId) -> Result<ZoneOwnerLease, String> {
        self.try_owner_lease(zone)
    }

    fn validate_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        if self.try_owner_lease(lease.zone_id())? == *lease {
            Ok(())
        } else {
            Err("stale_owner".into())
        }
    }
}

struct NeverCreateRuntime;

impl ZoneRuntimeFactory for NeverCreateRuntime {
    fn create_runtime(&self, _: GatewayConfig, _: &ZoneId) -> ZoneRuntimeHandle {
        panic!("failed routing must not create a personal or shared world")
    }
}

#[test]
fn fallible_routing_rejects_missing_authority_before_creating_a_world() {
    let registry = ZoneRegistry::with_router_and_owner_lease_authority(
        ZoneId::primary(),
        Arc::new(NeverCreateRuntime),
        Arc::new(crate::routing::SingleZoneSessionRouter),
        Arc::new(FaultAuthority::new(true)),
    );
    let result =
        registry.try_open_session_for(GatewayConfig::default(), SessionRouteRequest::anonymous());
    assert_eq!(result.err().as_deref(), Some("coordination_unavailable"));
}

#[test]
fn both_transport_admission_constructors_return_errors_before_creating_a_world() {
    let registry = ZoneRegistry::with_router_and_owner_lease_authority(
        ZoneId::primary(),
        Arc::new(NeverCreateRuntime),
        Arc::new(crate::routing::SingleZoneSessionRouter),
        Arc::new(FaultAuthority::new(true)),
    );
    assert_eq!(
        GatewaySession::try_new_with_zone_registry(GatewayConfig::default(), &registry)
            .err()
            .as_deref(),
        Some("coordination_unavailable")
    );
    assert_eq!(
        GatewaySession::try_new_with_zone_registry_and_event_sink(
            GatewayConfig::default(),
            &registry,
            Arc::new(crate::InMemoryGameplayEventSink::default()),
        )
        .err()
        .as_deref(),
        Some("coordination_unavailable")
    );
}

#[test]
fn failed_refresh_preserves_the_previous_session_binding_and_can_recover() {
    let zone = ZoneId::primary();
    let lease = ZoneOwnerLease::in_process(&zone);
    let authority = Arc::new(FaultAuthority::new(true));
    let runtime = InProcessZoneRuntimeFactory.create_runtime(GatewayConfig::default(), &zone);
    let mut session = GatewaySession::with_routed_world_runtime_and_owner_authority(
        zone,
        lease.clone(),
        Some(authority.clone()),
        runtime,
    );
    assert_eq!(
        session.refresh_zone_owner_lease().err().as_deref(),
        Some("coordination_unavailable")
    );
    assert_eq!(session.zone_owner_lease(), &lease);
    authority.unavailable.store(false, Ordering::SeqCst);
    assert_eq!(session.refresh_zone_owner_lease().unwrap(), &lease);
}

fn health_envelope() -> ZoneRpcEnvelope {
    ZoneRpcEnvelope {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        auth_token: Some("lease-boundary-private-test-token".into()),
        session_id: "lease-failure-health".into(),
        zone_id: "primary".into(),
        request: ZoneRpcRequest::ValidateOwnerHealth {
            source_registration_id: 1,
            change: ZoneOwnerHealthChange {
                cursor: ZoneOwnerHealthCursor {
                    session_id: "unused-test-identity".into(),
                    online_owner: "unused-owner".into(),
                    object_id: 1,
                    life_generation: 1,
                    dead: false,
                    health_sequence: 1,
                },
                hp_before: 10,
                mp_before: 10,
                hp: 9,
                mp: 10,
            },
        },
    }
}

#[test]
fn owner_health_reports_coordination_error_without_creating_or_poisoning_a_session() {
    let authority = Arc::new(FaultAuthority::new(true));
    let server = ZoneHostServer::with_options(
        GatewayConfig::default(),
        authority.clone(),
        Some("lease-boundary-private-test-token".into()),
        ZoneRpcLimits::default(),
    );
    let error = server.handle_envelope(health_envelope()).unwrap_err();
    assert!(error.message.contains("coordination_unavailable"));
    assert_eq!(server.session_count(), 0);
    assert!(server.journal.lock().unwrap().entries.is_empty());
    authority.unavailable.store(false, Ordering::SeqCst);
    assert!(matches!(
        server.handle_envelope(health_envelope()).unwrap(),
        ZoneRpcPayload::Bool { value: false }
    ));
    assert_eq!(server.session_count(), 0);
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "owner cadence did not make progress"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn owner_cadence_freezes_on_coordination_loss_and_resumes_without_restarting() {
    let authority = Arc::new(FaultAuthority::new(true));
    let factory = Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
        Duration::from_millis(10),
        BTreeMap::new(),
    ));
    let server = ZoneHostServer::with_options_and_factory(
        GatewayConfig::default(),
        authority.clone(),
        None,
        ZoneRpcLimits::default(),
        factory.clone(),
    );
    let zone = ZoneId::primary();
    let runtime = factory.create_runtime(GatewayConfig::default(), &zone);
    wait_until(|| authority.calls.load(Ordering::SeqCst) >= 2);
    assert_eq!(factory.zone_tick_count(&zone), 0);
    assert!(server.journal.lock().unwrap().entries.is_empty());
    authority.unavailable.store(false, Ordering::SeqCst);
    wait_until(|| factory.zone_tick_count(&zone) > 0);
    assert!(!server.journal.lock().unwrap().entries.is_empty());
    assert!(server.operation_gate.lock_zone(&zone).is_ok());
    drop(runtime);
}

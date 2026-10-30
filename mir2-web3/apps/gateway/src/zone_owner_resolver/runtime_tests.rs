use super::super::{ResolveError, ZoneLeaseObserver};
use super::*;
use crate::zone_lease::ObservedZoneLease;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Mutex;

#[derive(Debug)]
struct Observer {
    row: Mutex<Option<ObservedZoneLease>>,
    fail: AtomicBool,
    calls: AtomicUsize,
    advance_after_read: AtomicBool,
}

impl Observer {
    fn new() -> Self {
        Self {
            row: Mutex::new(Some(Self::row("host-a", 7, 200))),
            fail: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
            advance_after_read: AtomicBool::new(false),
        }
    }

    fn row(owner: &str, epoch: u64, expires_at_ms: u64) -> ObservedZoneLease {
        ObservedZoneLease {
            owner_id: owner.into(),
            fencing_token: epoch,
            expires_at_ms,
            heartbeat_at_ms: 100,
        }
    }
}

impl ZoneLeaseObserver for Observer {
    fn observe(&self, _: &ZoneId) -> Result<Option<ObservedZoneLease>, ResolveError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(ResolveError::ObserverUnavailable);
        }
        let row = self.row.lock().unwrap().clone();
        if self.advance_after_read.swap(false, Ordering::SeqCst) {
            *self.row.lock().unwrap() = Some(Self::row("host-b", 8, 500));
        }
        Ok(row)
    }
}

fn environment() -> BTreeMap<String, OsString> {
    BTreeMap::from([
        (
            CONFIG_ENV.into(),
            OsString::from("private/config-path.json"),
        ),
        (
            OBSERVER_DATABASE_ENV.into(),
            OsString::from("postgres://reader:private-password@127.0.0.1/world"),
        ),
    ])
}

fn configuration(
    values: &BTreeMap<String, OsString>,
) -> Result<Option<RuntimeConfiguration>, String> {
    RuntimeConfiguration::from_lookup(|name| values.get(name).cloned())
}

fn registry(endpoint: &str) -> Vec<u8> {
    serde_json::json!({"version": 1, "hosts": [
        {"ownerId":"host-a", "endpoint":endpoint},
        {"ownerId":"host-b", "endpoint":"127.0.0.1:7402"}
    ]})
    .to_string()
    .into_bytes()
}

fn runtime_fixture() -> (Arc<ZoneOwnerResolverRuntime>, Arc<Observer>, Arc<AtomicU64>) {
    let observer = Arc::new(Observer::new());
    let now = Arc::new(AtomicU64::new(100));
    let clock = now.clone();
    let runtime = build_runtime(
        configuration(&environment()).unwrap(),
        |_| Ok(registry("127.0.0.1:7401")),
        |_| Ok(observer.clone()),
        Arc::new(move || clock.load(Ordering::SeqCst)),
    )
    .unwrap()
    .unwrap();
    (runtime, observer, now)
}

#[test]
fn disabled_requires_both_activation_variables_to_be_absent() {
    assert!(configuration(&BTreeMap::new()).unwrap().is_none());
    for name in [CONFIG_ENV, OBSERVER_DATABASE_ENV] {
        let values = BTreeMap::from([(name.to_string(), OsString::from(""))]);
        assert_eq!(configuration(&values).err().unwrap(), INCOMPLETE_CONFIG);
    }
}

#[test]
fn disabled_mode_preserves_legacy_and_unrelated_environment() {
    let values = BTreeMap::from([
        (
            "MIR2_ZONE_HOST_ADDR".into(),
            OsString::from("127.0.0.1:7400"),
        ),
        (
            "MIR2_GATE15_VALIDATOR_URLS".into(),
            OsString::from("legacy"),
        ),
        (TOKEN_ENV.into(), OsString::from("token")),
    ]);
    assert!(configuration(&values).unwrap().is_none());
    assert!(build_runtime(
        None,
        |_| panic!("disabled startup must not read files"),
        |_| panic!("disabled startup must not connect"),
        Arc::new(|| 100),
    )
    .unwrap()
    .is_none());
}

#[test]
fn blank_required_configuration_is_an_error() {
    for (name, value) in [
        (CONFIG_ENV, ""),
        (CONFIG_ENV, "   "),
        (OBSERVER_DATABASE_ENV, ""),
        (OBSERVER_DATABASE_ENV, "\t"),
    ] {
        let mut values = environment();
        values.insert(name.into(), value.into());
        assert_eq!(configuration(&values).err().unwrap(), INVALID_CONFIG);
    }
}

#[test]
fn every_real_gate15_or_static_route_knob_conflicts_even_when_empty() {
    for name in CONFLICTING_ENV {
        for value in ["enabled", ""] {
            let mut values = environment();
            values.insert((*name).into(), value.into());
            assert_eq!(configuration(&values).err().unwrap(), CONFLICTING_CONFIG);
        }
    }
}

#[test]
fn unrelated_host_rpc_and_gateway_configuration_is_ignored() {
    let mut values = environment();
    for name in [
        "MIR2_GATEWAY_INSTANCE_ID",
        "MIR2_ZONE_HOST_ID",
        "MIR2_ZONE_HOST_BUILD_ID",
        "MIR2_ZONE_RPC_CODEC",
        "MIR2_ZONE_RPC_TIMEOUT_MS",
        "MIR2_SOMETHING_ELSE",
    ] {
        values.insert(name.into(), "private-ignored-value".into());
    }
    assert!(configuration(&values).unwrap().is_some());
}

#[test]
fn registry_io_and_parser_failures_are_fixed_and_sanitized() {
    for (failure, expected) in [
        (RegistryReadError::Unavailable, REGISTRY_UNAVAILABLE),
        (RegistryReadError::TooLarge, REGISTRY_TOO_LARGE),
    ] {
        let result = build_runtime(
            configuration(&environment()).unwrap(),
            |_| Err(failure),
            |_| panic!("invalid file cannot connect"),
            Arc::new(|| 100),
        );
        assert_eq!(result.unwrap_err(), expected);
    }
    for bytes in [
        b"private-invalid-json".to_vec(),
        vec![0xff, 0xfe],
        b"{}".to_vec(),
    ] {
        let result = build_runtime(
            configuration(&environment()).unwrap(),
            |_| Ok(bytes),
            |_| panic!("invalid registry cannot connect"),
            Arc::new(|| 100),
        );
        assert_eq!(result.unwrap_err(), INVALID_REGISTRY);
    }
}

#[test]
fn oversized_injected_registry_is_rejected_before_parsing_or_connecting() {
    let result = build_runtime(
        configuration(&environment()).unwrap(),
        |_| Ok(vec![b' '; MAX_CONFIG_BYTES + 1]),
        |_| panic!("oversized registry cannot connect"),
        Arc::new(|| 100),
    );
    assert_eq!(result.unwrap_err(), REGISTRY_TOO_LARGE);
}

#[test]
fn every_nonloopback_registry_endpoint_requires_a_token_before_connecting() {
    for endpoint in ["10.20.0.1:7401", "192.168.1.2:7401", "[2001:db8::1]:7401"] {
        let result = build_runtime(
            configuration(&environment()).unwrap(),
            |_| Ok(registry(endpoint)),
            |_| panic!("missing authentication cannot connect"),
            Arc::new(|| 100),
        );
        assert_eq!(result.unwrap_err(), TOKEN_REQUIRED);
    }
}

#[test]
fn loopback_including_normalized_mapped_ipv4_can_omit_the_token() {
    for endpoint in ["127.0.0.1:7401", "[::1]:7401", "[::ffff:127.0.0.1]:7401"] {
        let runtime = build_runtime(
            configuration(&environment()).unwrap(),
            |_| Ok(registry(endpoint)),
            |_| Ok(Arc::new(Observer::new())),
            Arc::new(|| 100),
        )
        .unwrap()
        .unwrap();
        assert!(runtime.token.is_none());
    }
}

#[test]
fn explicit_token_and_readonly_dsn_are_preserved_but_never_debugged() {
    let mut values = environment();
    values.insert(TOKEN_ENV.into(), "private-rpc-token".into());
    let runtime = build_runtime(
        configuration(&values).unwrap(),
        |_| Ok(registry("10.20.0.1:7401")),
        |dsn| {
            assert_eq!(dsn, "postgres://reader:private-password@127.0.0.1/world");
            Ok(Arc::new(Observer::new()))
        },
        Arc::new(|| 100),
    )
    .unwrap()
    .unwrap();
    assert_eq!(runtime.token.as_deref(), Some("private-rpc-token"));
    let debug = format!("{runtime:?}");
    for private in ["private-rpc-token", "private-password", "config-path"] {
        assert!(!debug.contains(private));
    }
}

#[test]
fn observer_connection_failures_use_one_fixed_error() {
    let result = build_runtime(
        configuration(&environment()).unwrap(),
        |_| Ok(registry("127.0.0.1:7401")),
        |_| Err(ResolveError::InvalidLease),
        Arc::new(|| 100),
    );
    assert_eq!(result.unwrap_err(), OBSERVER_UNAVAILABLE);
}

#[test]
fn runtime_shares_one_resolver_and_observation_authority() {
    let (runtime, _, _) = runtime_fixture();
    assert!(Arc::ptr_eq(&runtime.resolver, &runtime.resolver()));
    assert!(Arc::ptr_eq(
        &runtime.authority,
        &runtime.owner_lease_authority()
    ));
    assert!(!runtime.authority.supports_durable_npc_purchase_lease());
    assert_eq!(
        runtime.limits.max_frame_bytes,
        ZoneRpcLimits::default().max_frame_bytes
    );
}

#[test]
fn observed_authority_refresh_sees_current_owner_but_validation_cannot_adopt_it() {
    let (runtime, observer, _) = runtime_fixture();
    let zone = ZoneId::new("map:0");
    let initial = runtime.authority.try_owner_lease(&zone).unwrap();
    assert_eq!(initial.owner_id(), "host-a");
    *observer.row.lock().unwrap() = Some(Observer::row("host-b", 8, 500));
    assert_eq!(
        runtime
            .authority
            .validate_owner_lease(&initial)
            .unwrap_err(),
        ResolveError::RequiresRebind.code()
    );
    assert_eq!(
        runtime.authority.renew_owner_lease(&initial).unwrap_err(),
        ResolveError::RequiresRebind.code()
    );
    let current = runtime.authority.try_refresh_owner_lease(&zone).unwrap();
    assert_eq!(current.owner_id(), "host-b");
    assert_eq!(current.fencing_token(), 8);
}

#[test]
fn renewal_observes_once_and_cannot_adopt_a_generation_between_two_reads() {
    let (runtime, observer, _) = runtime_fixture();
    let lease = ZoneOwnerLease::new(ZoneId::new("map:0"), "host-a", 7);
    observer.advance_after_read.store(true, Ordering::SeqCst);
    assert_eq!(runtime.authority.renew_owner_lease(&lease).unwrap(), lease);
    assert_eq!(observer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime.authority.validate_owner_lease(&lease).unwrap_err(),
        ResolveError::RequiresRebind.code()
    );
}

#[test]
fn renewal_only_accepts_current_expiry_and_ignores_caller_clock() {
    let (runtime, observer, now) = runtime_fixture();
    let lease = ZoneOwnerLease::new(ZoneId::new("map:0"), "host-a", 7);
    now.store(250, Ordering::SeqCst);
    assert_eq!(
        runtime
            .authority
            .renew_owner_lease_at(&lease, 0)
            .unwrap_err(),
        ResolveError::LeaseExpired.code()
    );
    *observer.row.lock().unwrap() = Some(Observer::row("host-a", 7, 500));
    assert_eq!(
        runtime
            .authority
            .renew_owner_lease_at(&lease, u64::MAX)
            .unwrap(),
        lease
    );
}

#[test]
fn observation_failure_is_never_replaced_by_a_cached_or_fabricated_lease() {
    let (runtime, observer, _) = runtime_fixture();
    let zone = ZoneId::new("map:0");
    runtime.authority.try_owner_lease(&zone).unwrap();
    observer.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        runtime.authority.try_owner_lease(&zone).unwrap_err(),
        ResolveError::ObserverUnavailable.code()
    );
    assert_eq!(
        runtime
            .authority
            .try_refresh_owner_lease(&zone)
            .unwrap_err(),
        ResolveError::ObserverUnavailable.code()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime
            .authority
            .owner_lease(&zone)))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime
            .authority
            .refresh_owner_lease(&zone)))
        .is_err()
    );
}

#[test]
fn an_uninitialized_global_is_compatible_only_when_activation_is_absent() {
    let cell = OnceLock::new();
    assert!(global_from_lookup(&cell, |_| None).unwrap().is_none());
    for name in [CONFIG_ENV, OBSERVER_DATABASE_ENV] {
        assert_eq!(
            global_from_lookup(&cell, |candidate| (candidate == name).then(OsString::new))
                .unwrap_err(),
            NOT_INITIALIZED
        );
    }
    assert!(cell.get().is_none());
}

#[test]
fn startup_records_disabled_without_repeating_initialization() {
    let cell = OnceLock::new();
    assert!(initialize_cell(&cell, || Ok(None)).unwrap().is_none());
    assert!(
        initialize_cell(&cell, || panic!("disabled is already initialized"))
            .unwrap()
            .is_none()
    );
    assert!(global_from_lookup(&cell, |_| panic!(
        "initialized global must not inspect environment"
    ))
    .unwrap()
    .is_none());
}

#[test]
fn startup_records_failure_without_request_time_retries_or_fallback() {
    let cell = OnceLock::new();
    assert_eq!(
        initialize_cell(&cell, || Err(OBSERVER_UNAVAILABLE.into())).unwrap_err(),
        OBSERVER_UNAVAILABLE
    );
    assert_eq!(
        initialize_cell(&cell, || panic!("failed initialization cannot retry")).unwrap_err(),
        OBSERVER_UNAVAILABLE
    );
    assert_eq!(
        global_from_lookup(&cell, |_| None).unwrap_err(),
        OBSERVER_UNAVAILABLE
    );
}

#[test]
fn startup_shares_the_enabled_runtime_without_environment_rechecks() {
    let cell = OnceLock::new();
    let (runtime, _, _) = runtime_fixture();
    let initial = initialize_cell(&cell, || Ok(Some(runtime.clone())))
        .unwrap()
        .unwrap();
    let current = global_from_lookup(&cell, |_| panic!("already initialized"))
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&initial, &current));
}

#[cfg(any(unix, windows))]
#[test]
fn nonunicode_credentials_are_rejected_without_source_text() {
    #[cfg(windows)]
    let invalid = {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0xd800])
    };
    #[cfg(unix)]
    let invalid = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![0xff])
    };
    for name in [OBSERVER_DATABASE_ENV, TOKEN_ENV] {
        let mut values = environment();
        values.insert(name.into(), invalid.clone());
        assert_eq!(configuration(&values).err().unwrap(), INVALID_CONFIG);
    }
}

#[test]
fn real_file_reader_enforces_the_byte_limit() {
    use std::io::Write;
    struct TestFile(PathBuf);
    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "mir2-owner-registry-{}-{nanos}.json",
        std::process::id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let fixture = TestFile(path);
    file.write_all(&vec![b' '; MAX_CONFIG_BYTES]).unwrap();
    file.flush().unwrap();
    assert_eq!(
        read_registry_file(&fixture.0).unwrap().len(),
        MAX_CONFIG_BYTES
    );
    file.write_all(b" ").unwrap();
    file.flush().unwrap();
    assert!(matches!(
        read_registry_file(&fixture.0),
        Err(RegistryReadError::TooLarge)
    ));
    drop(file);
}

#[test]
fn session_projection_factory_creates_no_shared_zone_or_remote_observation() {
    let (runtime, observer, _) = runtime_fixture();
    let factory = runtime.session_runtime_factory();
    for zone in [ZoneId::new("map:0"), ZoneId::new("map:1")] {
        let projection = factory.create_runtime(crate::GatewayConfig::default(), &zone);
        assert!(projection
            .as_ref()
            .as_any()
            .is::<mir2_simulation::InProcessWorldRuntime>());
        assert!(crate::routing::shared_zone_movement_ingress(&projection).is_none());
        assert!(projection.active_identity().is_none());
        assert_eq!(projection.world_snapshot().tick, 0);
        let _ = projection.on_connect();
    }
    assert_eq!(observer.calls.load(Ordering::SeqCst), 0);
}

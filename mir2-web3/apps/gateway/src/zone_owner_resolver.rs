//! Read-only resolution of an existing Zone owner through an immutable registry.
//!
//! This module does not acquire or renew leases, place maps, connect to Hosts,
//! read environment variables, or migrate live sessions. A registry addition or
//! ordering change cannot move an existing Zone. A changed owner, epoch, or
//! endpoint requires an explicit session rebind instead of a mutation retry.

use crate::routing::{ZoneId, ZoneOwnerLease};
use crate::zone_lease::ObservedZoneLease;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

mod postgres;
pub mod runtime;
mod transport;
pub use postgres::{PostgresObserverLimits, PostgresZoneLeaseObserver};
pub use transport::StableZoneOwnerRpcTransport;

const REGISTRY_VERSION: u32 = 1;
const MAX_REGISTRY_BYTES: usize = 256 * 1024;
const MAX_REGISTRY_HOSTS: usize = 1024;
const MAX_OWNER_ID_BYTES: usize = 160;
const MAX_PG_VALUE: u64 = i64::MAX as u64;

/// Fixed codes deliberately omit parser input, connection strings, and driver
/// errors. Observer implementations must not attach secret-bearing error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveError {
    InvalidRegistryJson,
    UnsupportedRegistryVersion,
    InvalidRegistrySize,
    InvalidOwnerId,
    InvalidEndpoint,
    DuplicateOwnerId,
    DuplicateEndpoint,
    ObserverUnavailable,
    InvalidClock,
    LeaseNotRegistered,
    InvalidLease,
    LeaseExpired,
    OwnerNotRegistered,
    RequiresRebind,
}

impl ResolveError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidRegistryJson => "zone_owner_invalid_registry_json",
            Self::UnsupportedRegistryVersion => "zone_owner_unsupported_registry_version",
            Self::InvalidRegistrySize => "zone_owner_invalid_registry_size",
            Self::InvalidOwnerId => "zone_owner_invalid_owner_id",
            Self::InvalidEndpoint => "zone_owner_invalid_endpoint",
            Self::DuplicateOwnerId => "zone_owner_duplicate_owner_id",
            Self::DuplicateEndpoint => "zone_owner_duplicate_endpoint",
            Self::ObserverUnavailable => "zone_owner_observer_unavailable",
            Self::InvalidClock => "zone_owner_invalid_clock",
            Self::LeaseNotRegistered => "zone_owner_lease_not_registered",
            Self::InvalidLease => "zone_owner_invalid_lease",
            Self::LeaseExpired => "zone_owner_lease_expired",
            Self::OwnerNotRegistered => "zone_owner_host_not_registered",
            Self::RequiresRebind => "zone_owner_requires_rebind",
        }
    }

    pub fn requires_rebind(self) -> bool {
        self == Self::RequiresRebind
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ResolveError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryConfig {
    version: u32,
    hosts: Vec<RegistryHost>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RegistryHost {
    owner_id: String,
    endpoint: String,
}

/// One fixed owner ID maps to one explicit unicast IP socket address. Host list
/// position is never used for routing. Authentication and actual IP ownership
/// must be checked by the transport/deployment layer, not inferred by this map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRegistry {
    hosts: BTreeMap<String, SocketAddr>,
}

impl HostRegistry {
    /// Parse `{"version":1,"hosts":[{"ownerId":"host-a",
    /// "endpoint":"10.20.0.1:7400"}]}`. Derived struct deserialization rejects
    /// duplicate fields at both levels, rather than silently taking the last.
    pub fn from_json(input: &str) -> Result<Self, ResolveError> {
        if input.len() > MAX_REGISTRY_BYTES {
            return Err(ResolveError::InvalidRegistrySize);
        }
        let config: RegistryConfig =
            serde_json::from_str(input).map_err(|_| ResolveError::InvalidRegistryJson)?;
        if config.version != REGISTRY_VERSION {
            return Err(ResolveError::UnsupportedRegistryVersion);
        }
        if config.hosts.is_empty() || config.hosts.len() > MAX_REGISTRY_HOSTS {
            return Err(ResolveError::InvalidRegistrySize);
        }
        let mut hosts = BTreeMap::new();
        let mut endpoints = BTreeSet::new();
        for host in config.hosts {
            if !valid_owner_id(&host.owner_id) {
                return Err(ResolveError::InvalidOwnerId);
            }
            let endpoint = parse_endpoint(&host.endpoint)?;
            if hosts.contains_key(&host.owner_id) {
                return Err(ResolveError::DuplicateOwnerId);
            }
            if !endpoints.insert(endpoint) {
                return Err(ResolveError::DuplicateEndpoint);
            }
            hosts.insert(host.owner_id, endpoint);
        }
        Ok(Self { hosts })
    }

    pub fn endpoint(&self, owner_id: &str) -> Option<SocketAddr> {
        self.hosts.get(owner_id).copied()
    }

    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }
}

fn valid_owner_id(owner_id: &str) -> bool {
    !owner_id.is_empty()
        && owner_id.len() <= MAX_OWNER_ID_BYTES
        && owner_id.as_bytes()[0].is_ascii_alphanumeric()
        && owner_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn parse_endpoint(value: &str) -> Result<SocketAddr, ResolveError> {
    let mut endpoint: SocketAddr = value.parse().map_err(|_| ResolveError::InvalidEndpoint)?;
    if endpoint.port() == 0 {
        return Err(ResolveError::InvalidEndpoint);
    }
    if let SocketAddr::V6(address) = endpoint {
        if address.scope_id() != 0 || address.flowinfo() != 0 {
            return Err(ResolveError::InvalidEndpoint);
        }
        // Treat IPv4 and its mapped IPv6 spelling as the same destination.
        if let Some(ipv4) = address.ip().to_ipv4_mapped() {
            endpoint = SocketAddr::new(IpAddr::V4(ipv4), address.port());
        }
    }
    if endpoint.ip().is_unspecified()
        || endpoint.ip().is_multicast()
        || matches!(endpoint.ip(), IpAddr::V4(ip) if ip.is_broadcast())
    {
        return Err(ResolveError::InvalidEndpoint);
    }
    Ok(endpoint)
}

/// Only observation is exposed: implementations cannot be asked to acquire,
/// renew, release, or hand off ownership by this resolver.
pub trait ZoneLeaseObserver: Send + Sync {
    fn observe(&self, zone_id: &ZoneId) -> Result<Option<ObservedZoneLease>, ResolveError>;
}

pub trait ZoneOwnerClock: Send + Sync {
    fn now_ms(&self) -> u64;
}

impl<F> ZoneOwnerClock for F
where
    F: Fn() -> u64 + Send + Sync,
{
    fn now_ms(&self) -> u64 {
        self()
    }
}

/// A validated snapshot. Its endpoint remains part of the binding even when a
/// caller later compares it against a resolver created with another registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedZoneOwner {
    lease: ZoneOwnerLease,
    endpoint: SocketAddr,
    expires_at_ms: u64,
}

impl ResolvedZoneOwner {
    pub fn lease(&self) -> &ZoneOwnerLease {
        &self.lease
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }
}

/// The observer and clock are injected; the core owns no environment, worker,
/// database connection, route cache, or network retry policy.
pub struct StableZoneOwnerResolver {
    registry: HostRegistry,
    observer: Arc<dyn ZoneLeaseObserver>,
    clock: Arc<dyn ZoneOwnerClock>,
}

impl fmt::Debug for StableZoneOwnerResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StableZoneOwnerResolver")
            .field("host_count", &self.registry.len())
            .finish_non_exhaustive()
    }
}

impl StableZoneOwnerResolver {
    pub fn new(
        registry: HostRegistry,
        observer: Arc<dyn ZoneLeaseObserver>,
        clock: Arc<dyn ZoneOwnerClock>,
    ) -> Self {
        Self {
            registry,
            observer,
            clock,
        }
    }

    pub fn resolve(&self, zone_id: &ZoneId) -> Result<ResolvedZoneOwner, ResolveError> {
        let (lease, expires_at_ms) = self.observe_live_lease(zone_id)?;
        self.bind_endpoint(lease, expires_at_ms)
    }

    /// For a Gateway observer authority: validate the supplied owner and epoch.
    /// A heartbeat may observe a longer TTL but may not adopt a new owner/token.
    pub fn require_lease(
        &self,
        expected: &ZoneOwnerLease,
    ) -> Result<ResolvedZoneOwner, ResolveError> {
        if !valid_owner_id(expected.owner_id())
            || expected.fencing_token() == 0
            || expected.fencing_token() > MAX_PG_VALUE
        {
            return Err(ResolveError::InvalidLease);
        }
        let (lease, expires_at_ms) = self.observe_live_lease(expected.zone_id())?;
        if lease != *expected {
            return Err(ResolveError::RequiresRebind);
        }
        self.bind_endpoint(lease, expires_at_ms)
    }

    /// For a session transport: pin owner, epoch, and endpoint together. Only
    /// current expiry matters; renewal of the same epoch can extend an old pin.
    pub fn require_exact(
        &self,
        expected: &ResolvedZoneOwner,
    ) -> Result<ResolvedZoneOwner, ResolveError> {
        let current = self.require_lease(expected.lease())?;
        if current.endpoint != expected.endpoint {
            return Err(ResolveError::RequiresRebind);
        }
        Ok(current)
    }

    fn observe_live_lease(&self, zone_id: &ZoneId) -> Result<(ZoneOwnerLease, u64), ResolveError> {
        let observed = self
            .observer
            .observe(zone_id)
            .map_err(|_| ResolveError::ObserverUnavailable)?
            .ok_or(ResolveError::LeaseNotRegistered)?;
        if !valid_owner_id(&observed.owner_id)
            || observed.fencing_token == 0
            || observed.fencing_token > MAX_PG_VALUE
            || observed.expires_at_ms > MAX_PG_VALUE
            || observed.heartbeat_at_ms > MAX_PG_VALUE
        {
            return Err(ResolveError::InvalidLease);
        }
        // Read the clock after observation: an IO delay must not make an
        // expired result appear live using a timestamp captured before the IO.
        let now_ms = self.clock.now_ms();
        if now_ms > MAX_PG_VALUE {
            return Err(ResolveError::InvalidClock);
        }
        if observed.expires_at_ms <= now_ms {
            return Err(ResolveError::LeaseExpired);
        }
        Ok((
            ZoneOwnerLease::new(zone_id.clone(), observed.owner_id, observed.fencing_token),
            observed.expires_at_ms,
        ))
    }

    fn bind_endpoint(
        &self,
        lease: ZoneOwnerLease,
        expires_at_ms: u64,
    ) -> Result<ResolvedZoneOwner, ResolveError> {
        let endpoint = self
            .registry
            .endpoint(lease.owner_id())
            .ok_or(ResolveError::OwnerNotRegistered)?;
        Ok(ResolvedZoneOwner {
            lease,
            endpoint,
            expires_at_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Mutex;

    #[derive(Default)]
    struct Observer {
        rows: Mutex<BTreeMap<String, ObservedZoneLease>>,
        calls: AtomicUsize,
        failure: Mutex<Option<ResolveError>>,
    }

    impl Observer {
        fn put(&self, zone: &str, owner: &str, token: u64, expiry: u64) {
            self.rows.lock().unwrap().insert(
                zone.into(),
                ObservedZoneLease {
                    owner_id: owner.into(),
                    fencing_token: token,
                    expires_at_ms: expiry,
                    heartbeat_at_ms: 100,
                },
            );
        }
    }

    impl ZoneLeaseObserver for Observer {
        fn observe(&self, zone: &ZoneId) -> Result<Option<ObservedZoneLease>, ResolveError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = *self.failure.lock().unwrap() {
                return Err(error);
            }
            Ok(self.rows.lock().unwrap().get(zone.as_str()).cloned())
        }
    }

    fn registry(entries: &[(&str, &str)]) -> HostRegistry {
        let hosts: Vec<_> = entries
            .iter()
            .map(|(owner, endpoint)| serde_json::json!({"ownerId":owner,"endpoint":endpoint}))
            .collect();
        HostRegistry::from_json(&serde_json::json!({"version":1,"hosts":hosts}).to_string())
            .unwrap()
    }

    fn fixture() -> (StableZoneOwnerResolver, Arc<Observer>, Arc<AtomicU64>) {
        let observer = Arc::new(Observer::default());
        observer.put("map:0", "host-a", 7, 200);
        let now = Arc::new(AtomicU64::new(100));
        let clock = now.clone();
        let resolver = StableZoneOwnerResolver::new(
            registry(&[("host-a", "10.20.0.1:7400"), ("host-b", "10.20.0.2:7400")]),
            observer.clone(),
            Arc::new(move || clock.load(Ordering::SeqCst)),
        );
        (resolver, observer, now)
    }

    #[test]
    fn registry_rejects_duplicate_json_fields_at_both_levels() {
        for input in [
            r#"{"version":1,"version":1,"hosts":[]}"#,
            r#"{"version":1,"hosts":[],"hosts":[]}"#,
            r#"{"version":1,"hosts":[{"ownerId":"a","ownerId":"b","endpoint":"127.0.0.1:1"}]}"#,
            r#"{"version":1,"hosts":[{"ownerId":"a","endpoint":"127.0.0.1:1","endpoint":"127.0.0.1:2"}]}"#,
        ] {
            assert_eq!(
                HostRegistry::from_json(input),
                Err(ResolveError::InvalidRegistryJson)
            );
        }
    }

    #[test]
    fn registry_rejects_unknown_missing_and_wrongly_typed_fields() {
        for input in [
            r#"{"version":1,"hosts":[],"command":"secret"}"#,
            r#"{"version":1,"hosts":[{"ownerId":"a","endpoint":"127.0.0.1:1","token":"secret"}]}"#,
            r#"{"hosts":[]}"#,
            r#"{"version":1,"hosts":[{"ownerId":"a"}]}"#,
            r#"{"version":"1","hosts":[]}"#,
            r#"{"version":1,"hosts":[]}{}"#,
        ] {
            assert_eq!(
                HostRegistry::from_json(input),
                Err(ResolveError::InvalidRegistryJson)
            );
        }
    }

    #[test]
    fn registry_version_and_size_are_bounded() {
        assert_eq!(
            HostRegistry::from_json(r#"{"version":2,"hosts":[]}"#),
            Err(ResolveError::UnsupportedRegistryVersion)
        );
        assert_eq!(
            HostRegistry::from_json(r#"{"version":1,"hosts":[]}"#),
            Err(ResolveError::InvalidRegistrySize)
        );
        assert_eq!(
            HostRegistry::from_json(&" ".repeat(MAX_REGISTRY_BYTES + 1)),
            Err(ResolveError::InvalidRegistrySize)
        );
        let hosts = vec![
            serde_json::json!({"ownerId":"a","endpoint":"127.0.0.1:1"});
            MAX_REGISTRY_HOSTS + 1
        ];
        assert_eq!(
            HostRegistry::from_json(&serde_json::json!({"version":1,"hosts":hosts}).to_string()),
            Err(ResolveError::InvalidRegistrySize)
        );
    }

    #[test]
    fn owner_identifiers_are_exact_bounded_ascii_components() {
        for owner in ["", " a", "a ", "a b", "a\nb", "a/b", "a@b", "主机", "-a"] {
            let input = serde_json::json!({"version":1,"hosts":[{"ownerId":owner,"endpoint":"127.0.0.1:1"}]}).to_string();
            assert_eq!(
                HostRegistry::from_json(&input),
                Err(ResolveError::InvalidOwnerId)
            );
        }
        let too_long = "a".repeat(MAX_OWNER_ID_BYTES + 1);
        let input = serde_json::json!({"version":1,"hosts":[{"ownerId":too_long,"endpoint":"127.0.0.1:1"}]}).to_string();
        assert_eq!(
            HostRegistry::from_json(&input),
            Err(ResolveError::InvalidOwnerId)
        );
        assert!(registry(&[("zone-host_a.1:worker", "127.0.0.1:1")])
            .endpoint("zone-host_a.1:worker")
            .is_some());
    }

    #[test]
    fn endpoints_require_explicit_nonzero_unicast_ip_sockets() {
        for endpoint in [
            "host.invalid:7400",
            "10.20.0.1",
            "10.20.0.1:0",
            "10.20.0.1:65536",
            " 10.20.0.1:7400",
            "10.20.0.1:7400 ",
            "0.0.0.0:7400",
            "[::]:7400",
            "224.0.0.1:7400",
            "[ff02::1]:7400",
            "255.255.255.255:7400",
            "[fe80::1%1]:7400",
            "[::ffff:0.0.0.0]:7400",
        ] {
            let input =
                serde_json::json!({"version":1,"hosts":[{"ownerId":"a","endpoint":endpoint}]})
                    .to_string();
            assert_eq!(
                HostRegistry::from_json(&input),
                Err(ResolveError::InvalidEndpoint)
            );
        }
        let valid = registry(&[("a", "127.0.0.1:1"), ("b", "[::1]:65535")]);
        assert_eq!(valid.len(), 2);
        assert!(!valid.is_empty());
    }

    #[test]
    fn registry_rejects_duplicate_owners_and_canonical_endpoint_aliases() {
        let input = r#"{"version":1,"hosts":[{"ownerId":"a","endpoint":"127.0.0.1:1"},{"ownerId":"a","endpoint":"127.0.0.1:2"}]}"#;
        assert_eq!(
            HostRegistry::from_json(input),
            Err(ResolveError::DuplicateOwnerId)
        );
        for endpoints in [
            ["10.20.0.1:7400", "10.20.0.1:7400"],
            ["10.20.0.1:7400", "[::ffff:10.20.0.1]:7400"],
            ["[::1]:7400", "[0:0:0:0:0:0:0:1]:7400"],
        ] {
            let input = serde_json::json!({"version":1,"hosts":[{"ownerId":"a","endpoint":endpoints[0]},{"ownerId":"b","endpoint":endpoints[1]}]}).to_string();
            assert_eq!(
                HostRegistry::from_json(&input),
                Err(ResolveError::DuplicateEndpoint)
            );
        }
    }

    #[test]
    fn registry_additions_and_reordering_do_not_change_an_existing_route() {
        let (first, observer, _) = fixture();
        let before = first.resolve(&ZoneId::new("map:0")).unwrap();
        let after = StableZoneOwnerResolver::new(
            registry(&[
                ("host-c", "10.20.0.3:7400"),
                ("host-b", "10.20.0.2:7400"),
                ("host-a", "10.20.0.1:7400"),
            ]),
            observer,
            Arc::new(|| 100),
        );
        assert_eq!(after.require_exact(&before).unwrap(), before);
    }

    #[test]
    fn resolution_uses_observed_owner_instead_of_zone_hash_or_host_order() {
        let (resolver, observer, _) = fixture();
        observer.put("map:1", "host-b", 19, 300);
        let owner = resolver.resolve(&ZoneId::new("map:1")).unwrap();
        assert_eq!(owner.lease().zone_id(), &ZoneId::new("map:1"));
        assert_eq!(owner.lease().owner_id(), "host-b");
        assert_eq!(owner.lease().fencing_token(), 19);
        assert_eq!(owner.endpoint(), "10.20.0.2:7400".parse().unwrap());
        assert_eq!(owner.expires_at_ms(), 300);
    }

    #[test]
    fn missing_lease_is_not_created_or_replaced_with_a_fallback() {
        let (resolver, observer, _) = fixture();
        let before = observer.rows.lock().unwrap().clone();
        assert_eq!(
            resolver.resolve(&ZoneId::new("missing")),
            Err(ResolveError::LeaseNotRegistered)
        );
        assert_eq!(*observer.rows.lock().unwrap(), before);
        assert_eq!(observer.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unknown_owner_is_not_hashed_to_a_registered_host() {
        let (resolver, observer, _) = fixture();
        observer.put("map:0", "unregistered", 7, 200);
        assert_eq!(
            resolver.resolve(&ZoneId::new("map:0")),
            Err(ResolveError::OwnerNotRegistered)
        );
    }

    #[test]
    fn observed_fences_must_fit_positive_postgres_bigint() {
        let (resolver, observer, _) = fixture();
        for token in [0, MAX_PG_VALUE + 1, u64::MAX] {
            observer.put("map:0", "host-a", token, 200);
            assert_eq!(
                resolver.resolve(&ZoneId::new("map:0")),
                Err(ResolveError::InvalidLease)
            );
        }
        observer.put("map:0", "host-a", MAX_PG_VALUE, 200);
        assert_eq!(
            resolver
                .resolve(&ZoneId::new("map:0"))
                .unwrap()
                .lease()
                .fencing_token(),
            MAX_PG_VALUE
        );
    }

    #[test]
    fn lease_expiry_must_be_future_and_fit_postgres_bigint() {
        let (resolver, observer, _) = fixture();
        for expiry in [0, 99, 100] {
            observer.put("map:0", "host-a", 7, expiry);
            assert_eq!(
                resolver.resolve(&ZoneId::new("map:0")),
                Err(ResolveError::LeaseExpired)
            );
        }
        observer.put("map:0", "host-a", 7, 101);
        assert!(resolver.resolve(&ZoneId::new("map:0")).is_ok());
        observer.put("map:0", "host-a", 7, MAX_PG_VALUE + 1);
        assert_eq!(
            resolver.resolve(&ZoneId::new("map:0")),
            Err(ResolveError::InvalidLease)
        );
    }

    #[test]
    fn invalid_observed_owner_and_heartbeat_are_rejected_without_panicking() {
        let (resolver, observer, _) = fixture();
        observer.put("map:0", "", 7, 200);
        assert_eq!(
            resolver.resolve(&ZoneId::new("map:0")),
            Err(ResolveError::InvalidLease)
        );
        observer.put("map:0", "host-a", 7, 200);
        observer
            .rows
            .lock()
            .unwrap()
            .get_mut("map:0")
            .unwrap()
            .heartbeat_at_ms = u64::MAX;
        assert_eq!(
            resolver.resolve(&ZoneId::new("map:0")),
            Err(ResolveError::InvalidLease)
        );
    }

    #[test]
    fn a_clock_outside_postgres_range_is_rejected() {
        let (resolver, _, now) = fixture();
        now.store(u64::MAX, Ordering::SeqCst);
        assert_eq!(
            resolver.resolve(&ZoneId::new("map:0")),
            Err(ResolveError::InvalidClock)
        );
    }

    #[test]
    fn expiry_is_checked_after_observation_finishes() {
        struct SlowObserver(Arc<AtomicU64>);
        impl ZoneLeaseObserver for SlowObserver {
            fn observe(&self, _: &ZoneId) -> Result<Option<ObservedZoneLease>, ResolveError> {
                self.0.store(201, Ordering::SeqCst);
                Ok(Some(ObservedZoneLease {
                    owner_id: "host-a".into(),
                    fencing_token: 7,
                    expires_at_ms: 200,
                    heartbeat_at_ms: 100,
                }))
            }
        }
        let now = Arc::new(AtomicU64::new(100));
        let clock = now.clone();
        let resolver = StableZoneOwnerResolver::new(
            registry(&[("host-a", "10.20.0.1:7400")]),
            Arc::new(SlowObserver(now)),
            Arc::new(move || clock.load(Ordering::SeqCst)),
        );
        assert_eq!(
            resolver.resolve(&ZoneId::new("map:0")),
            Err(ResolveError::LeaseExpired)
        );
    }

    #[test]
    fn observer_failure_cannot_reuse_a_previous_success() {
        let (resolver, observer, _) = fixture();
        let pin = resolver.resolve(&ZoneId::new("map:0")).unwrap();
        *observer.failure.lock().unwrap() = Some(ResolveError::InvalidRegistryJson);
        assert_eq!(
            resolver.require_exact(&pin),
            Err(ResolveError::ObserverUnavailable)
        );
        assert_eq!(observer.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn owner_or_epoch_change_requires_explicit_rebind_even_if_new_owner_is_unknown() {
        let (resolver, observer, _) = fixture();
        let pin = resolver.resolve(&ZoneId::new("map:0")).unwrap();
        for (owner, token) in [("host-b", 7), ("host-a", 8), ("unknown", 8)] {
            observer.put("map:0", owner, token, 200);
            assert_eq!(
                resolver.require_lease(pin.lease()),
                Err(ResolveError::RequiresRebind)
            );
            assert_eq!(
                resolver.require_exact(&pin),
                Err(ResolveError::RequiresRebind)
            );
        }
    }

    #[test]
    fn endpoint_change_in_another_registry_cannot_reuse_an_old_pin() {
        let (resolver, observer, _) = fixture();
        let pin = resolver.resolve(&ZoneId::new("map:0")).unwrap();
        let changed = StableZoneOwnerResolver::new(
            registry(&[("host-a", "10.20.0.99:7400")]),
            observer,
            Arc::new(|| 100),
        );
        assert!(changed.require_lease(pin.lease()).is_ok());
        assert_eq!(
            changed.require_exact(&pin),
            Err(ResolveError::RequiresRebind)
        );
    }

    #[test]
    fn same_epoch_renewal_preserves_the_binding_without_writing_a_lease() {
        let (resolver, observer, now) = fixture();
        let pin = resolver.resolve(&ZoneId::new("map:0")).unwrap();
        observer.put("map:0", "host-a", 7, 400);
        now.store(250, Ordering::SeqCst);
        let before = observer.rows.lock().unwrap().clone();
        let renewed = resolver.require_exact(&pin).unwrap();
        assert_eq!(renewed.lease(), pin.lease());
        assert_eq!(renewed.endpoint(), pin.endpoint());
        assert_eq!(renewed.expires_at_ms(), 400);
        assert_eq!(*observer.rows.lock().unwrap(), before);
    }

    #[test]
    fn an_expired_or_removed_current_lease_never_accepts_an_old_pin() {
        let (resolver, observer, now) = fixture();
        let pin = resolver.resolve(&ZoneId::new("map:0")).unwrap();
        now.store(200, Ordering::SeqCst);
        assert_eq!(
            resolver.require_exact(&pin),
            Err(ResolveError::LeaseExpired)
        );
        observer.rows.lock().unwrap().clear();
        assert_eq!(
            resolver.require_exact(&pin),
            Err(ResolveError::LeaseNotRegistered)
        );
    }

    #[test]
    fn expected_fence_outside_postgres_range_is_not_observed_or_accepted() {
        let (resolver, observer, _) = fixture();
        let expected = ZoneOwnerLease::new(ZoneId::new("map:0"), "host-a", u64::MAX);
        assert_eq!(
            resolver.require_lease(&expected),
            Err(ResolveError::InvalidLease)
        );
        assert_eq!(observer.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn errors_are_sanitized_and_rebind_has_a_machine_readable_code() {
        let input = r#"{"version":1,"hosts":[],"postgres://secret:password@host/db":"token"}"#;
        let error = HostRegistry::from_json(input).unwrap_err();
        assert_eq!(error.to_string(), "zone_owner_invalid_registry_json");
        assert!(!format!("{error:?}").contains("password"));
        assert!(ResolveError::RequiresRebind.requires_rebind());
        assert_eq!(
            ResolveError::RequiresRebind.code(),
            "zone_owner_requires_rebind"
        );
        assert!(!ResolveError::ObserverUnavailable.requires_rebind());
    }
}

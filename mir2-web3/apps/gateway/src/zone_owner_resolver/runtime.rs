//! Explicit Gateway startup wiring for the read-only owner resolver.
//!
//! The observer never acquires or renews ownership. Disabled configuration is
//! recorded at startup; request paths cannot silently bypass a configured but
//! uninitialized resolver. Source errors deliberately omit paths and credentials.

use super::postgres::{PostgresObserverLimits, PostgresZoneLeaseObserver};
use super::transport::StableZoneOwnerRpcTransport;
use super::{HostRegistry, StableZoneOwnerResolver, ZoneLeaseObserver, ZoneOwnerClock};
use crate::routing::{
    InProcessZoneRuntimeFactory, SharedZoneOwnerLeaseAuthority, SharedZoneOwnerRpcTransport,
    SharedZoneRuntimeFactory, ZoneId, ZoneOwnerLease, ZoneOwnerLeaseAuthority,
};
use crate::zone_rpc::ZoneRpcLimits;
use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const CONFIG_ENV: &str = "MIR2_GATEWAY_ZONE_OWNER_RESOLVER_CONFIG";
const OBSERVER_DATABASE_ENV: &str = "MIR2_GATEWAY_ZONE_LEASE_OBSERVER_DATABASE_URL";
const TOKEN_ENV: &str = "MIR2_ZONE_HOST_TOKEN";
const MAX_CONFIG_BYTES: usize = 256 * 1024;

// These names are the actual Gate 15 controls and legacy static/sharded RPC
// routing inputs. General Gateway IDs and unrelated RPC/Host tuning are not
// competing placement policies and do not activate or invalidate this mode.
const CONFLICTING_ENV: &[&str] = &[
    "MIR2_GATE15_VALIDATOR_URLS",
    "MIR2_GATE15_GATEWAY_ID",
    "MIR2_GATE15_SESSION_LEASE_TTL_MS",
    "MIR2_GATE15_OBSERVER_INTERVAL_MS",
    "MIR2_GATE15_LOCAL_ZONE_HOST_ID",
    "MIR2_ZONE_HOST_ADDR",
    "MIR2_ZONE_HOST_ADDRS",
    "MIR2_ZONE_HOST_ACTIVE_ADDRS",
    "MIR2_ZONE_HOST_STANDBY_ADDRS",
    "MIR2_ZONE_HOST_COLOCATED_STANDBY",
    "MIR2_ZONE_HOST_PAIRED_STANDBY",
];

const INVALID_CONFIG: &str = "zone_owner_runtime_invalid_configuration";
const INCOMPLETE_CONFIG: &str = "zone_owner_runtime_incomplete_configuration";
const CONFLICTING_CONFIG: &str = "zone_owner_runtime_conflicting_configuration";
const REGISTRY_UNAVAILABLE: &str = "zone_owner_runtime_registry_unavailable";
const REGISTRY_TOO_LARGE: &str = "zone_owner_runtime_registry_too_large";
const INVALID_REGISTRY: &str = "zone_owner_runtime_invalid_registry";
const OBSERVER_UNAVAILABLE: &str = "zone_owner_runtime_observer_unavailable";
const TOKEN_REQUIRED: &str = "zone_owner_runtime_token_required";
const NOT_INITIALIZED: &str = "zone_owner_runtime_not_initialized";

type RuntimeState = Result<Option<Arc<ZoneOwnerResolverRuntime>>, String>;
static GLOBAL_RUNTIME: OnceLock<RuntimeState> = OnceLock::new();
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

struct RuntimeConfiguration {
    registry_path: PathBuf,
    readonly_dsn: String,
    token: Option<String>,
}

impl RuntimeConfiguration {
    fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<OsString>,
    ) -> Result<Option<Self>, String> {
        let registry_path = lookup(CONFIG_ENV);
        let readonly_dsn = lookup(OBSERVER_DATABASE_ENV);
        let (registry_path, readonly_dsn) = match (registry_path, readonly_dsn) {
            (None, None) => return Ok(None),
            (Some(registry_path), Some(readonly_dsn)) => (registry_path, readonly_dsn),
            _ => return Err(INCOMPLETE_CONFIG.into()),
        };
        if CONFLICTING_ENV.iter().any(|name| lookup(name).is_some()) {
            return Err(CONFLICTING_CONFIG.into());
        }
        if registry_path.is_empty() || registry_path.to_string_lossy().trim().is_empty() {
            return Err(INVALID_CONFIG.into());
        }
        let readonly_dsn = readonly_dsn.into_string().map_err(|_| INVALID_CONFIG)?;
        if readonly_dsn.trim().is_empty() {
            return Err(INVALID_CONFIG.into());
        }
        let token = lookup(TOKEN_ENV)
            .map(|value| value.into_string().map_err(|_| INVALID_CONFIG.to_string()))
            .transpose()?
            .filter(|value| !value.trim().is_empty());
        Ok(Some(Self {
            registry_path: registry_path.into(),
            readonly_dsn,
            token,
        }))
    }
}

/// Read-only authority for Gateway admission and validation. Ownership remains
/// entirely with the existing durable writer running at the authoritative Host.
#[derive(Debug)]
struct ObservedZoneOwnerLeaseAuthority {
    resolver: Arc<StableZoneOwnerResolver>,
}

impl ZoneOwnerLeaseAuthority for ObservedZoneOwnerLeaseAuthority {
    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        self.try_owner_lease(zone_id)
            .expect("read-only Zone owner observation failed")
    }

    fn try_owner_lease(&self, zone_id: &ZoneId) -> Result<ZoneOwnerLease, String> {
        self.resolver
            .resolve(zone_id)
            .map(|owner| owner.lease().clone())
            .map_err(|error| error.to_string())
    }

    fn refresh_owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        self.try_refresh_owner_lease(zone_id)
            .expect("read-only Zone owner refresh failed")
    }

    fn try_refresh_owner_lease(&self, zone_id: &ZoneId) -> Result<ZoneOwnerLease, String> {
        self.try_owner_lease(zone_id)
    }

    fn validate_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        self.resolver
            .require_lease(lease)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn renew_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<ZoneOwnerLease, String> {
        // Observe exactly once and retain the supplied epoch. Calling validate
        // followed by resolve could otherwise adopt a new epoch between reads.
        self.resolver
            .require_lease(lease)
            .map(|owner| owner.lease().clone())
            .map_err(|error| error.to_string())
    }

    fn renew_owner_lease_at(
        &self,
        lease: &ZoneOwnerLease,
        _now_ms: u64,
    ) -> Result<ZoneOwnerLease, String> {
        // An untrusted caller timestamp cannot extend an observed lease's TTL.
        self.renew_owner_lease(lease)
    }
}

pub struct ZoneOwnerResolverRuntime {
    resolver: Arc<StableZoneOwnerResolver>,
    authority: SharedZoneOwnerLeaseAuthority,
    token: Option<String>,
    limits: ZoneRpcLimits,
}

impl fmt::Debug for ZoneOwnerResolverRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZoneOwnerResolverRuntime")
            .field("resolver", &self.resolver)
            .field("authenticated", &self.token.is_some())
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl ZoneOwnerResolverRuntime {
    pub fn resolver(&self) -> Arc<StableZoneOwnerResolver> {
        self.resolver.clone()
    }

    pub fn owner_lease_authority(&self) -> SharedZoneOwnerLeaseAuthority {
        self.authority.clone()
    }

    /// Personal projection required by the existing command-client interface.
    /// Authoritative commands still use the pinned RPC client; this factory
    /// creates no shared Zone resources, cadence worker or movement ingress.
    pub fn session_runtime_factory(&self) -> SharedZoneRuntimeFactory {
        Arc::new(InProcessZoneRuntimeFactory)
    }

    pub fn transport(
        &self,
        initial_lease: ZoneOwnerLease,
    ) -> Result<SharedZoneOwnerRpcTransport, String> {
        let sequence = NEXT_SESSION_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| "zone_owner_runtime_session_ids_exhausted".to_string())?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "zone_owner_runtime_invalid_clock".to_string())?
            .as_nanos();
        let session_id = format!("zone-owner-rpc-{}-{nanos}-{sequence}", std::process::id());
        StableZoneOwnerRpcTransport::new(
            self.resolver.clone(),
            initial_lease,
            session_id,
            self.token.clone(),
            self.limits.clone(),
        )
        .map(|transport| Arc::new(transport) as SharedZoneOwnerRpcTransport)
    }
}

#[derive(Debug, Clone, Copy)]
enum RegistryReadError {
    Unavailable,
    TooLarge,
}

fn read_registry_file(path: &Path) -> Result<Vec<u8>, RegistryReadError> {
    let file = File::open(path).map_err(|_| RegistryReadError::Unavailable)?;
    let metadata = file
        .metadata()
        .map_err(|_| RegistryReadError::Unavailable)?;
    if !metadata.is_file() {
        return Err(RegistryReadError::Unavailable);
    }
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err(RegistryReadError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| RegistryReadError::Unavailable)?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(RegistryReadError::TooLarge);
    }
    Ok(bytes)
}

fn build_runtime(
    configuration: Option<RuntimeConfiguration>,
    read_registry: impl FnOnce(&Path) -> Result<Vec<u8>, RegistryReadError>,
    connect_observer: impl FnOnce(&str) -> Result<Arc<dyn ZoneLeaseObserver>, super::ResolveError>,
    clock: Arc<dyn ZoneOwnerClock>,
) -> RuntimeState {
    let Some(configuration) = configuration else {
        return Ok(None);
    };
    let bytes = read_registry(&configuration.registry_path).map_err(|error| match error {
        RegistryReadError::Unavailable => REGISTRY_UNAVAILABLE.to_string(),
        RegistryReadError::TooLarge => REGISTRY_TOO_LARGE.to_string(),
    })?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(REGISTRY_TOO_LARGE.into());
    }
    let json = std::str::from_utf8(&bytes).map_err(|_| INVALID_REGISTRY.to_string())?;
    let registry = HostRegistry::from_json(json).map_err(|_| INVALID_REGISTRY.to_string())?;
    if configuration.token.is_none()
        && registry
            .hosts
            .values()
            .any(|endpoint| !endpoint.ip().is_loopback())
    {
        return Err(TOKEN_REQUIRED.into());
    }
    let observer = connect_observer(&configuration.readonly_dsn)
        .map_err(|_| OBSERVER_UNAVAILABLE.to_string())?;
    let resolver = Arc::new(StableZoneOwnerResolver::new(registry, observer, clock));
    Ok(Some(Arc::new(ZoneOwnerResolverRuntime {
        authority: Arc::new(ObservedZoneOwnerLeaseAuthority {
            resolver: resolver.clone(),
        }),
        resolver,
        token: configuration.token,
        limits: ZoneRpcLimits::default(),
    })))
}

fn system_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(u64::MAX)
}

/// Construct one observer and resolver from the explicit opt-in configuration.
/// This performs a bounded registry read and the observer's synchronous initial
/// PostgreSQL privilege audit. Neither schema nor leases are modified.
pub fn from_env() -> RuntimeState {
    let configuration = RuntimeConfiguration::from_lookup(|name| std::env::var_os(name))?;
    build_runtime(
        configuration,
        read_registry_file,
        |dsn| {
            PostgresZoneLeaseObserver::connect(dsn, PostgresObserverLimits::default())
                .map(|observer| Arc::new(observer) as Arc<dyn ZoneLeaseObserver>)
        },
        Arc::new(system_now_ms),
    )
}

fn initialize_cell(
    cell: &OnceLock<RuntimeState>,
    initialize: impl FnOnce() -> RuntimeState,
) -> RuntimeState {
    cell.get_or_init(initialize).clone()
}

/// Call before accepting traffic. An absent configuration is stored explicitly;
/// failed initialization is also retained instead of retried on request paths.
pub fn init_global_from_env() -> RuntimeState {
    initialize_cell(&GLOBAL_RUNTIME, from_env)
}

fn global_from_lookup(
    cell: &OnceLock<RuntimeState>,
    mut lookup: impl FnMut(&str) -> Option<OsString>,
) -> RuntimeState {
    if let Some(state) = cell.get() {
        return state.clone();
    }
    if lookup(CONFIG_ENV).is_some() || lookup(OBSERVER_DATABASE_ENV).is_some() {
        return Err(NOT_INITIALIZED.into());
    }
    Ok(None)
}

/// Read startup state without lazy initialization, connection work or fallback
/// when this mode was configured but startup initialization was omitted.
pub fn global() -> RuntimeState {
    global_from_lookup(&GLOBAL_RUNTIME, |name| std::env::var_os(name))
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;

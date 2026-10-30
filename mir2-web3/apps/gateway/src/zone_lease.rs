//! Durable, fenced zone-ownership leases backed by Postgres.
//!
//! This is the coordination primitive for distributed gateway failover. A single
//! `zone_owner_leases` row per zone records the current owner, a monotonic fencing
//! token, and an absolute expiry. Ownership semantics:
//!
//! - **acquire**: claim the zone if it is unowned, owned by us, or expired. Every
//!   expired lease starts a new epoch, including reacquisition by the same owner.
//!   Renewing our own live lease keeps its token. A live different owner is only
//!   observed; neither acquisition nor validation trusts a cached lease.
//! - **renew (heartbeat)**: extend expiry iff we still hold the lease with a
//!   matching fencing token and it has not expired. Otherwise the lease is lost.
//! - **release**: expire the exact owner/token on graceful shutdown while retaining
//!   its fencing history, so a peer can take over immediately without token reuse.
//!
//! A returning "zombie" owner is fenced out: its stale token fails renewal, and any
//! token-checked write is rejected by the new epoch's higher token.
//!
//! `postgres` (sync) creates its own runtime, so every DB call runs on a freshly
//! spawned OS thread — the same pattern the simulation account store uses to avoid
//! nested-Tokio-runtime panics inside the async gateway.
//!
//! Validation is a current-row check, not a lock through an arbitrary later write.
//! Durable writes still need a guard held through their commit. Live runtime
//! transfer and owner process-incarnation uniqueness are separate contracts.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use postgres::{Client, NoTls};

use crate::routing::{
    SharedZoneOwnerLeaseAuthority, ZoneId, ZoneOwnerLease, ZoneOwnerLeaseAuthority,
};

const DEFAULT_LEASE_TTL_MS: u64 = 30_000;

/// Owns a synchronous PostgreSQL resource all the way through its destruction.
/// `postgres::Client::drop` drives its private Tokio runtime. The last owner may
/// disappear on an async worker when a server future is cancelled or fails, so
/// putting only construction/queries on blocking workers is not sufficient.
pub(crate) struct SyncPostgresResource<T: Send> {
    value: Option<T>,
}

impl<T: Send> SyncPostgresResource<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value: Some(value) }
    }
}

impl<T: Send> Deref for SyncPostgresResource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
            .as_ref()
            .expect("PostgreSQL resource is owned until drop")
    }
}

impl<T: Send> Drop for SyncPostgresResource<T> {
    fn drop(&mut self) {
        if tokio::runtime::Handle::try_current().is_err() {
            // Ordinary synchronous callers already have a safe context.
            drop(self.value.take());
            return;
        }
        // This worker does not depend on a live Tokio scheduler, including on a
        // current-thread runtime or during shutdown. Join before returning: do
        // not leak connections or defer cleanup to a runtime being destroyed.
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| drop(self.value.take()));
            if let Err(panic) = worker.join() {
                std::panic::resume_unwind(panic);
            }
        });
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

/// A snapshot of the stored lease for a zone, regardless of who owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedZoneLease {
    pub owner_id: String,
    pub fencing_token: u64,
    pub expires_at_ms: u64,
    pub heartbeat_at_ms: u64,
}

impl ObservedZoneLease {
    pub fn is_live(&self, now_ms: u64) -> bool {
        self.expires_at_ms > now_ms
    }
}

pub struct PostgresZoneOwnerLeaseAuthority {
    database_url: String,
    owner_id: String,
    lease_ttl_ms: u64,
    client: Arc<SyncPostgresResource<Mutex<Option<Client>>>>,
}

impl std::fmt::Debug for PostgresZoneOwnerLeaseAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresZoneOwnerLeaseAuthority")
            .field("owner_id", &self.owner_id)
            .field("lease_ttl_ms", &self.lease_ttl_ms)
            .finish()
    }
}

impl PostgresZoneOwnerLeaseAuthority {
    pub fn new(
        database_url: impl Into<String>,
        owner_id: impl Into<String>,
        lease_ttl_ms: u64,
    ) -> Self {
        let owner_id = owner_id.into();
        assert!(
            !owner_id.trim().is_empty(),
            "zone lease owner id must not be empty"
        );
        Self {
            database_url: database_url.into(),
            owner_id,
            lease_ttl_ms: lease_ttl_ms.max(1),
            client: Arc::new(SyncPostgresResource::new(Mutex::new(None))),
        }
    }

    /// Build from the environment. Returns `None` when no lease database URL is
    /// configured, so the caller can fall back to the in-process authority.
    pub fn from_env() -> Option<Self> {
        let database_url = std::env::var("MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())?;
        let owner_id = configured_owner_id(
            std::env::var("MIR2_GATEWAY_ZONE_LEASE_OWNER_ID").ok(),
            std::env::var("MIR2_GATEWAY_INSTANCE_ID").ok(),
        );
        let lease_ttl_ms = std::env::var("MIR2_GATEWAY_ZONE_LEASE_TTL_MS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_LEASE_TTL_MS);
        Some(Self::new(
            database_url.trim().to_string(),
            owner_id,
            lease_ttl_ms,
        ))
    }

    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub fn lease_ttl_ms(&self) -> u64 {
        self.lease_ttl_ms
    }

    /// Run a blocking Postgres operation on a dedicated OS thread, reusing a
    /// cached connection and dropping it on error so the next call reconnects.
    fn run<R, F>(&self, op: F) -> Result<R, String>
    where
        R: Send + 'static,
        F: FnOnce(&mut Client) -> Result<R, String> + Send + 'static,
    {
        let url = self.database_url.clone();
        let slot = self.client.clone();
        std::thread::spawn(move || {
            let mut guard = slot
                .lock()
                .map_err(|_| "zone lease client mutex poisoned".to_string())?;
            if guard.is_none() {
                let mut client = Client::connect(&url, NoTls)
                    .map_err(|error| format!("zone lease connect failed: {error}"))?;
                mir2_simulation::apply_migrations(&mut client)
                    .map_err(|error| format!("zone lease migration failed: {error}"))?;
                *guard = Some(client);
            }
            let client = guard.as_mut().expect("zone lease client present");
            match op(client) {
                Ok(value) => Ok(value),
                Err(error) => {
                    // Drop the (possibly broken) connection so the next call reconnects.
                    *guard = None;
                    Err(error)
                }
            }
        })
        .join()
        .map_err(|_| "zone lease worker thread panicked".to_string())?
    }

    /// Atomically acquire/renew/steal the lease, returning the resulting stored
    /// lease (whose owner may be a *different* live process).
    pub fn acquire_at(&self, zone_id: &ZoneId, now_ms: u64) -> Result<ZoneOwnerLease, String> {
        let zone = zone_id.as_str().to_string();
        let owner_id = self.owner_id.clone();
        let (now, expires_at) = postgres_lease_times(now_ms, self.lease_ttl_ms)?;
        let zone_for_lease = zone_id.clone();
        self.run(move |client| {
            let row = client
                .query_opt(
                    "INSERT INTO zone_owner_leases \
                     (zone_id, owner_id, fencing_token, expires_at_ms, heartbeat_at_ms, acquired_at_ms) \
                     VALUES ($1, $2, 1, $3, $4, $4) \
                     ON CONFLICT (zone_id) DO UPDATE SET \
                       fencing_token = CASE \
                           WHEN zone_owner_leases.expires_at_ms <= $4 THEN zone_owner_leases.fencing_token + 1 \
                           ELSE zone_owner_leases.fencing_token END, \
                       owner_id = CASE \
                           WHEN zone_owner_leases.owner_id = EXCLUDED.owner_id OR zone_owner_leases.expires_at_ms <= $4 \
                               THEN EXCLUDED.owner_id ELSE zone_owner_leases.owner_id END, \
                       expires_at_ms = CASE \
                           WHEN zone_owner_leases.owner_id = EXCLUDED.owner_id OR zone_owner_leases.expires_at_ms <= $4 \
                               THEN EXCLUDED.expires_at_ms ELSE zone_owner_leases.expires_at_ms END, \
                       heartbeat_at_ms = CASE \
                           WHEN zone_owner_leases.owner_id = EXCLUDED.owner_id OR zone_owner_leases.expires_at_ms <= $4 \
                               THEN EXCLUDED.heartbeat_at_ms ELSE zone_owner_leases.heartbeat_at_ms END, \
                       acquired_at_ms = CASE \
                           WHEN zone_owner_leases.expires_at_ms <= $4 \
                               THEN EXCLUDED.acquired_at_ms ELSE zone_owner_leases.acquired_at_ms END, \
                       updated_at = now() \
                     WHERE zone_owner_leases.fencing_token > 0 \
                       AND (zone_owner_leases.expires_at_ms > $4 \
                            OR zone_owner_leases.fencing_token < 9223372036854775807) \
                     RETURNING owner_id, fencing_token, expires_at_ms",
                    &[&zone, &owner_id, &expires_at, &now],
                )
                .map_err(|error| format!("zone lease acquire failed for {zone}: {error}"))?
                .ok_or_else(|| format!("zone lease fencing token invalid or exhausted for {zone}"))?;
            let owner_id: String = row.get("owner_id");
            let fencing_token: i64 = row.get("fencing_token");
            Ok(ZoneOwnerLease::new(
                zone_for_lease,
                owner_id,
                postgres_fencing_token(fencing_token)?,
            ))
        })
    }

    /// Extend the lease iff we still hold it (matching owner + fencing token and
    /// not yet expired). Errors when the lease has been lost.
    pub fn renew_at(&self, lease: &ZoneOwnerLease, now_ms: u64) -> Result<ZoneOwnerLease, String> {
        if lease.owner_id() != self.owner_id {
            return Err(format!(
                "cannot renew zone lease owned by {} from authority {}",
                lease.owner_id(),
                self.owner_id
            ));
        }
        let zone = lease.zone_id().as_str().to_string();
        let owner_id = self.owner_id.clone();
        let token = checked_postgres_fencing_token(lease.fencing_token())?;
        let (now, expires_at) = postgres_lease_times(now_ms, self.lease_ttl_ms)?;
        let zone_id = lease.zone_id().clone();
        let renewed = self.run(move |client| {
            let updated = client
                .query_opt(
                    "UPDATE zone_owner_leases \
                     SET expires_at_ms = $4, heartbeat_at_ms = $5, updated_at = now() \
                     WHERE zone_id = $1 AND owner_id = $2 AND fencing_token = $3 AND expires_at_ms > $5 \
                     RETURNING fencing_token",
                    &[&zone, &owner_id, &token, &expires_at, &now],
                )
                .map_err(|error| format!("zone lease renew failed for {zone}: {error}"))?;
            match updated {
                Some(_) => Ok(()),
                None => Err(format!(
                    "stale zone owner lease for {zone}: lost ownership (owner={owner_id}, token={token})"
                )),
            }
        });
        match renewed {
            Ok(()) => {
                let lease =
                    ZoneOwnerLease::new(zone_id, self.owner_id.clone(), lease.fencing_token());
                Ok(lease)
            }
            Err(error) => Err(error),
        }
    }

    /// Atomically hand a still-live lease to a new owner and advance its
    /// fencing token. The caller must be the current owner; the compare-and-set
    /// prevents a stale scheduler from overwriting a newer generation.
    pub fn handoff_at(
        &self,
        current: &ZoneOwnerLease,
        new_owner_id: impl Into<String>,
        now_ms: u64,
    ) -> Result<ZoneOwnerLease, String> {
        if current.owner_id() != self.owner_id {
            return Err(format!(
                "cannot hand off zone lease owned by {} from authority {}",
                current.owner_id(),
                self.owner_id
            ));
        }
        let new_owner_id = new_owner_id.into();
        if new_owner_id.trim().is_empty() || new_owner_id == self.owner_id {
            return Err("zone lease handoff requires a distinct non-empty owner".to_string());
        }
        let zone = current.zone_id().as_str().to_string();
        let current_owner_id = self.owner_id.clone();
        let current_token = checked_postgres_fencing_token(current.fencing_token())?;
        let next_token = current_token
            .checked_add(1)
            .ok_or_else(|| format!("zone lease fencing token exhausted for {zone}"))?;
        let (now, new_expires_at) = postgres_lease_times(now_ms, self.lease_ttl_ms)?;
        let zone_id = current.zone_id().clone();
        let owner_for_query = new_owner_id.clone();
        let new_token = self.run(move |client| {
            let row = client
                .query_opt(
                    "UPDATE zone_owner_leases
                     SET owner_id=$4,
                         fencing_token=$7,
                         expires_at_ms=$5,
                         heartbeat_at_ms=$6,
                         acquired_at_ms=$6,
                         updated_at=now()
                     WHERE zone_id=$1
                       AND owner_id=$2
                       AND fencing_token=$3
                       AND expires_at_ms>$6
                     RETURNING fencing_token",
                    &[
                        &zone,
                        &current_owner_id,
                        &current_token,
                        &owner_for_query,
                        &new_expires_at,
                        &now,
                        &next_token,
                    ],
                )
                .map_err(|error| format!("zone lease handoff failed for {zone}: {error}"))?
                .ok_or_else(|| {
                    format!("stale zone owner lease for {zone}: handoff compare-and-set rejected")
                })?;
            let token: i64 = row.get("fencing_token");
            postgres_fencing_token(token)
        })?;
        let lease = ZoneOwnerLease::new(zone_id, new_owner_id, new_token);
        Ok(lease)
    }

    /// Expire only this exact epoch, retaining its monotonic fencing history.
    /// Repeated or stale releases are harmless; they cannot expire a newer epoch.
    pub fn release(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        if lease.owner_id() != self.owner_id {
            return Err("cannot release another authority's zone owner lease".into());
        }
        let zone = lease.zone_id().as_str().to_string();
        let owner_id = self.owner_id.clone();
        let token = checked_postgres_fencing_token(lease.fencing_token())?;
        self.run(move |client| {
            client
                .execute(
                    "UPDATE zone_owner_leases SET expires_at_ms = 0, updated_at = now() \
                     WHERE zone_id = $1 AND owner_id = $2 AND fencing_token = $3 \
                       AND expires_at_ms <> 0",
                    &[&zone, &owner_id, &token],
                )
                .map_err(|error| format!("zone lease release failed for {zone}: {error}"))?;
            Ok(())
        })?;
        Ok(())
    }

    /// Read the current stored lease for a zone, whoever owns it.
    pub fn observe_at(
        &self,
        zone_id: &ZoneId,
        _now_ms: u64,
    ) -> Result<Option<ObservedZoneLease>, String> {
        let zone = zone_id.as_str().to_string();
        self.run(move |client| {
            let row = client
                .query_opt(
                    "SELECT owner_id, fencing_token, expires_at_ms, heartbeat_at_ms \
                     FROM zone_owner_leases WHERE zone_id = $1",
                    &[&zone],
                )
                .map_err(|error| format!("zone lease observe failed for {zone}: {error}"))?;
            row.map(|row| {
                let fencing_token: i64 = row.get("fencing_token");
                let expires_at_ms: i64 = row.get("expires_at_ms");
                let heartbeat_at_ms: i64 = row.get("heartbeat_at_ms");
                Ok(ObservedZoneLease {
                    owner_id: row.get("owner_id"),
                    fencing_token: postgres_fencing_token(fencing_token)?,
                    expires_at_ms: u64::try_from(expires_at_ms)
                        .map_err(|_| "zone lease expiry must not be negative".to_string())?,
                    heartbeat_at_ms: u64::try_from(heartbeat_at_ms)
                        .map_err(|_| "zone lease heartbeat must not be negative".to_string())?,
                })
            })
            .transpose()
        })
    }
}

fn checked_postgres_fencing_token(token: u64) -> Result<i64, String> {
    i64::try_from(token)
        .ok()
        .filter(|token| *token > 0)
        .ok_or_else(|| "zone lease fencing token is outside PostgreSQL BIGINT range".to_string())
}

fn postgres_fencing_token(token: i64) -> Result<u64, String> {
    u64::try_from(token)
        .ok()
        .filter(|token| *token > 0)
        .ok_or_else(|| "stored zone lease fencing token must be positive".to_string())
}

fn postgres_lease_times(now_ms: u64, ttl_ms: u64) -> Result<(i64, i64), String> {
    let now = i64::try_from(now_ms)
        .map_err(|_| "zone lease timestamp exceeds PostgreSQL BIGINT range".to_string())?;
    let expires_at = now_ms
        .checked_add(ttl_ms)
        .and_then(|expiry| i64::try_from(expiry).ok())
        .ok_or_else(|| "zone lease expiry exceeds PostgreSQL BIGINT range".to_string())?;
    Ok((now, expires_at))
}

fn configured_owner_id(explicit_owner: Option<String>, instance_id: Option<String>) -> String {
    explicit_owner
        .filter(|value| !value.trim().is_empty())
        .or_else(|| instance_id.filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(default_instance_id)
}

impl ZoneOwnerLeaseAuthority for PostgresZoneOwnerLeaseAuthority {
    fn supports_durable_npc_purchase_lease(&self) -> bool {
        true
    }

    fn with_durable_npc_purchase_lease(
        &self,
        lease: &ZoneOwnerLease,
        operation: &mut (dyn FnMut() + Send),
    ) -> Result<(), String> {
        // A scoped worker keeps the real row lock through the account commit;
        // no cached/bootstrap lease is accepted and no new schema is created.
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let mut slot = self.client.lock().map_err(|_| "zone lease client mutex poisoned")?;
                if slot.is_none() {
                    *slot = Some(Client::connect(&self.database_url, NoTls)
                        .map_err(|error| format!("durable NPC lease connect failed: {error}"))?);
                }
                let guarded_result = (|| {
                let client = slot.as_mut().expect("connected lease client");
                let mut transaction = client.transaction()
                    .map_err(|error| format!("durable NPC lease transaction failed: {error}"))?;
                let row = transaction.query_opt(
                    "SELECT owner_id, fencing_token, expires_at_ms FROM zone_owner_leases WHERE zone_id=$1 FOR UPDATE",
                    &[&lease.zone_id().as_str()],
                ).map_err(|error| format!("durable NPC owner lease lock failed: {error}"))?
                    .ok_or("durable NPC owner lease is not registered")?;
                let owner: String = row.get("owner_id");
                let token: i64 = row.get("fencing_token");
                let expires: i64 = row.get("expires_at_ms");
                if owner != lease.owner_id() || token <= 0 || token as u64 != lease.fencing_token()
                    || expires <= 0 || expires as u64 <= now_ms() {
                    return Err("stale durable NPC purchase owner lease".into());
                }
                crate::session::catch_gateway_panic("durable NPC lease callback", || operation())?;
                // The row was only locked. Rollback releases the fencing guard;
                // any release error follows the already captured commit receipt.
                transaction.rollback().map_err(|error| format!("durable NPC lease guard release failed: {error}"))
                })();
                if guarded_result.is_err() { *slot = None; }
                guarded_result
            }).join().map_err(|_| "durable NPC lease guard worker panicked".to_string())?
        })
    }

    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        // Legacy callers must opt into the fallible API to handle availability
        // errors. Never mint a bootstrap lease or forward a stale cached owner.
        self.try_owner_lease(zone_id)
            .expect("zone owner lease acquisition failed")
    }

    fn refresh_owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        self.try_refresh_owner_lease(zone_id)
            .expect("zone owner lease refresh failed")
    }

    fn try_owner_lease(&self, zone_id: &ZoneId) -> Result<ZoneOwnerLease, String> {
        self.acquire_at(zone_id, now_ms())
    }

    fn try_refresh_owner_lease(&self, zone_id: &ZoneId) -> Result<ZoneOwnerLease, String> {
        self.acquire_at(zone_id, now_ms())
    }

    fn validate_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        checked_postgres_fencing_token(lease.fencing_token())?;
        let now = now_ms();
        let current = self
            .observe_at(lease.zone_id(), now)
            .map_err(|error| {
                format!(
                    "cannot validate current zone owner lease for {}: {error}",
                    lease.zone_id()
                )
            })?
            .ok_or_else(|| format!("zone owner lease is not registered for {}", lease.zone_id()))?;
        if current.is_live(now_ms())
            && current.owner_id == lease.owner_id()
            && current.fencing_token == lease.fencing_token()
        {
            Ok(())
        } else {
            Err(format!(
                "stale zone owner lease for zone {}: current owner {} fencing token {} expiry {}, got owner {} fencing token {}",
                lease.zone_id(),
                current.owner_id,
                current.fencing_token,
                current.expires_at_ms,
                lease.owner_id(),
                lease.fencing_token()
            ))
        }
    }

    fn renew_owner_lease_at(
        &self,
        lease: &ZoneOwnerLease,
        now_ms: u64,
    ) -> Result<ZoneOwnerLease, String> {
        self.renew_at(lease, now_ms)
    }
}

/// A local Zone Host authority for the authenticated Home Agent hop.
///
/// The Home Agent has already verified the Relay's signed placement envelope
/// over mTLS before it forwards the Zone RPC frame over an authenticated
/// loopback socket. This authority lets the Zone Host adopt that external
/// lease while retaining monotonic fencing locally. It must only be enabled
/// with an explicit allow-list of Commonware Zone Host ids.
#[derive(Debug)]
pub struct TrustedRpcZoneOwnerLeaseAuthority {
    trusted_owners: BTreeSet<String>,
    leases: Mutex<BTreeMap<ZoneId, ZoneOwnerLease>>,
}

impl TrustedRpcZoneOwnerLeaseAuthority {
    pub fn new(trusted_owners: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let trusted_owners = trusted_owners
            .into_iter()
            .map(|owner| owner.trim().to_string())
            .filter(|owner| !owner.is_empty())
            .collect::<BTreeSet<_>>();
        if trusted_owners.is_empty() {
            return Err(
                "trusted-rpc Zone lease authority requires at least one trusted owner".to_string(),
            );
        }
        Ok(Self {
            trusted_owners,
            leases: Mutex::new(BTreeMap::new()),
        })
    }

    fn from_env() -> Result<Self, String> {
        let owners = std::env::var("MIR2_ZONE_OWNER_TRUSTED_RPC_OWNERS")
            .unwrap_or_default()
            .split(',')
            .map(str::to_string)
            .collect::<Vec<_>>();
        Self::new(owners)
    }
}

impl ZoneOwnerLeaseAuthority for TrustedRpcZoneOwnerLeaseAuthority {
    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        self.leases
            .lock()
            .expect("trusted-rpc Zone lease mutex should not be poisoned")
            .get(zone_id)
            .cloned()
            .unwrap_or_else(|| ZoneOwnerLease::in_process(zone_id))
    }

    fn validate_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        if !self.trusted_owners.contains(lease.owner_id()) {
            return Err(format!(
                "untrusted Zone owner {} for authenticated Home Agent RPC",
                lease.owner_id()
            ));
        }
        let mut leases = self
            .leases
            .lock()
            .expect("trusted-rpc Zone lease mutex should not be poisoned");
        match leases.get(lease.zone_id()) {
            None => {
                leases.insert(lease.zone_id().clone(), lease.clone());
                Ok(())
            }
            Some(current) if current == lease => Ok(()),
            Some(current) if lease.fencing_token() > current.fencing_token() => {
                leases.insert(lease.zone_id().clone(), lease.clone());
                Ok(())
            }
            Some(current) => Err(format!(
                "stale zone owner lease for zone {}: current owner {} fencing token {}, got owner {} fencing token {}",
                lease.zone_id(),
                current.owner_id(),
                current.fencing_token(),
                lease.owner_id(),
                lease.fencing_token()
            )),
        }
    }

    fn renew_owner_lease(&self, lease: &ZoneOwnerLease) -> Result<ZoneOwnerLease, String> {
        self.validate_owner_lease(lease)?;
        Ok(lease.clone())
    }
}

fn default_instance_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("gateway-{}-{nanos}", std::process::id())
}

/// The owner-lease authority selected from the environment: a durable Postgres
/// authority when `MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL` is set, otherwise the
/// in-process authority (single-writer, no cross-process failover).
pub fn default_zone_owner_lease_authority_from_env() -> SharedZoneOwnerLeaseAuthority {
    if let Some(authority) = crate::gate15::zone_owner_lease_authority() {
        eprintln!("zone owner lease authority: Gate 15 Commonware finalized placement");
        return authority;
    }
    if std::env::var("MIR2_ZONE_OWNER_LEASE_BACKEND")
        .ok()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("trusted-rpc"))
    {
        let authority = TrustedRpcZoneOwnerLeaseAuthority::from_env()
            .expect("invalid trusted-rpc Zone owner lease configuration");
        eprintln!(
            "zone owner lease authority: authenticated Home Agent RPC ({} trusted owners)",
            authority.trusted_owners.len()
        );
        return Arc::new(authority) as SharedZoneOwnerLeaseAuthority;
    }
    match PostgresZoneOwnerLeaseAuthority::from_env() {
        Some(authority) => {
            eprintln!(
                "zone owner lease authority: postgres (instance {}, ttl {}ms)",
                authority.owner_id(),
                authority.lease_ttl_ms()
            );
            Arc::new(authority) as SharedZoneOwnerLeaseAuthority
        }
        None => Arc::new(crate::routing::InMemoryZoneOwnerLeaseAuthority::new())
            as SharedZoneOwnerLeaseAuthority,
    }
}

#[cfg(test)]
pub(crate) struct LifecyclePostgresTest {
    url: String,
    monitor: Client,
}

#[cfg(test)]
impl LifecyclePostgresTest {
    pub(crate) fn new() -> Self {
        let url = std::env::var("MIR2_GATEWAY_LIFECYCLE_TEST_DATABASE_URL")
            .expect("explicit isolated lifecycle test database URL is required");
        let config = url
            .parse::<postgres::Config>()
            .unwrap_or_else(|_| panic!("invalid lifecycle test database configuration"));
        let database = config
            .get_dbname()
            .expect("explicit lifecycle database required");
        let suffix = database
            .strip_prefix("mir2_playtest_lifecycle_")
            .expect("lifecycle tests refuse ordinary game databases");
        assert!(
            !suffix.is_empty()
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        );
        assert!(config.get_user() == Some("mir2_playtest"));
        assert!(
            config.get_hosts().len() == 1
                && matches!(&config.get_hosts()[0],
            postgres::config::Host::Tcp(host) if host == "127.0.0.1" || host == "::1")
        );
        assert!(
            config.get_ports().len() == 1
                && config.get_ports()[0] != 0
                && config.get_ports()[0] != 5432
        );
        assert!(config.get_hostaddrs().is_empty() && config.get_options().is_none());
        let parsed = reqwest::Url::parse(&url)
            .unwrap_or_else(|_| panic!("lifecycle tests require a PostgreSQL URL"));
        assert!(parsed.query().is_none() && parsed.fragment().is_none());
        let mut monitor = config
            .connect(NoTls)
            .unwrap_or_else(|_| panic!("isolated lifecycle database connection failed"));
        let identity = monitor
            .query_one("SELECT current_database()::text, current_user::text", &[])
            .unwrap_or_else(|_| panic!("lifecycle database identity check failed"));
        assert!(identity.get::<_, &str>(0) == database);
        assert!(identity.get::<_, &str>(1) == "mir2_playtest");
        Self { url, monitor }
    }

    pub(crate) fn tagged_url(&self, label: &str) -> (String, String) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tag = format!("lifecycle-{label}-{}-{nonce}", std::process::id());
        assert!(tag.len() < 64);
        let mut url = reqwest::Url::parse(&self.url)
            .unwrap_or_else(|_| panic!("validated lifecycle URL became invalid"));
        url.query_pairs_mut().append_pair("application_name", &tag);
        (url.to_string(), tag)
    }

    pub(crate) fn connections(&mut self, tag: &str) -> i64 {
        self.monitor.query_one(
            "SELECT count(*)::bigint FROM pg_stat_activity WHERE datname = current_database() AND usename = current_user AND application_name = $1",
            &[&tag],
        ).unwrap_or_else(|_| panic!("lifecycle connection observation failed"))
            .get(0)
    }

    pub(crate) fn assert_connections_closed(&mut self, tag: &str) {
        for _ in 0..200 {
            if self.connections(tag) == 0 {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("owned PostgreSQL connections remained after lifecycle completion");
    }
}

#[cfg(test)]
pub(crate) fn lifecycle_test_runtime(multi_thread: bool) -> tokio::runtime::Runtime {
    let mut builder = if multi_thread {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.worker_threads(2);
        builder
    } else {
        tokio::runtime::Builder::new_current_thread()
    };
    builder
        .enable_all()
        .build()
        .expect("lifecycle test Tokio runtime")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_resource() -> (
        Arc<SyncPostgresResource<tokio::runtime::Runtime>>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        let runtime = lifecycle_test_runtime(false);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        runtime.spawn(async move {
            std::future::pending::<()>().await;
            drop(sender);
        });
        (Arc::new(SyncPostgresResource::new(runtime)), receiver)
    }

    #[test]
    fn sync_postgres_resource_drops_a_real_runtime_before_returning() {
        for multi_thread in [false, true] {
            let outer = lifecycle_test_runtime(multi_thread);
            let (resource, mut receiver) = runtime_resource();
            let weak = Arc::downgrade(&resource);
            outer.block_on(async move {
                drop(resource);
            });
            assert!(weak.upgrade().is_none());
            assert!(matches!(
                receiver.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Closed)
            ));
        }
    }

    #[test]
    fn sync_postgres_resource_handles_concurrent_last_arc_and_cancelled_task() {
        for multi_thread in [false, true] {
            let outer = lifecycle_test_runtime(multi_thread);
            let (resource, mut receiver) = runtime_resource();
            let copies = (0..4).map(|_| Arc::clone(&resource)).collect::<Vec<_>>();
            drop(resource);
            outer.block_on(async move {
                let barrier = Arc::new(tokio::sync::Barrier::new(copies.len()));
                let workers = copies
                    .into_iter()
                    .map(|copy| {
                        let barrier = Arc::clone(&barrier);
                        tokio::spawn(async move {
                            barrier.wait().await;
                            drop(copy);
                        })
                    })
                    .collect::<Vec<_>>();
                for worker in workers {
                    worker.await.expect("last owner must drop safely");
                }
            });
            assert!(matches!(
                receiver.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Closed)
            ));

            let (resource, mut receiver) = runtime_resource();
            outer.block_on(async move {
                let (ready, started) = tokio::sync::oneshot::channel();
                let worker = tokio::spawn(async move {
                    ready.send(()).unwrap();
                    std::future::pending::<()>().await;
                    drop(resource);
                });
                started.await.unwrap();
                worker.abort();
                assert!(worker.await.unwrap_err().is_cancelled());
            });
            assert!(matches!(
                receiver.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Closed)
            ));
        }
    }

    #[test]
    fn sync_postgres_resource_handles_async_initialization_error() {
        for multi_thread in [false, true] {
            let outer = lifecycle_test_runtime(multi_thread);
            let (resource, mut receiver) = runtime_resource();
            let result = outer.block_on(async move {
                let _resource = resource;
                tokio::task::yield_now().await;
                Err::<(), _>("later initializer failed")
            });
            assert!(result.is_err());
            assert!(matches!(
                receiver.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Closed)
            ));
        }
    }

    #[test]
    #[ignore = "requires isolated MIR2_GATEWAY_LIFECYCLE_TEST_DATABASE_URL"]
    fn lifecycle_postgres_closes_actual_zone_authority_cache() {
        let mut database = LifecyclePostgresTest::new();
        for multi_thread in [false, true] {
            let outer = lifecycle_test_runtime(multi_thread);
            let (url, tag) = database.tagged_url("zone-cache");
            let authority = Arc::new(PostgresZoneOwnerLeaseAuthority::new(
                &url,
                "lifecycle-only",
                1_000,
            ));
            // Populate the actual cached-client field. Do not run unrelated game
            // schema migrations or acquire/modify leases in this drop-only test.
            *authority.client.lock().unwrap() = Some(
                Client::connect(&url, NoTls)
                    .unwrap_or_else(|_| panic!("lifecycle cached client connection failed")),
            );
            assert_eq!(
                authority
                    .run(|client| {
                        client
                            .query_one("SELECT 1::int", &[])
                            .map(|row| row.get::<_, i32>(0))
                            .map_err(|_| "lifecycle cached client query failed".to_string())
                    })
                    .unwrap_or_else(|_| panic!("real authority cached operation failed")),
                1
            );
            assert_eq!(database.connections(&tag), 1);
            outer.block_on(async move {
                let (ready, started) = tokio::sync::oneshot::channel();
                let worker = tokio::spawn(async move {
                    ready.send(()).unwrap();
                    std::future::pending::<()>().await;
                    drop(authority);
                });
                started.await.unwrap();
                worker.abort();
                assert!(worker.await.unwrap_err().is_cancelled());
            });
            database.assert_connections_closed(&tag);
        }
        eprintln!(
            "actual zone authority cached-client closure verified in both Tokio runtime flavours"
        );
    }

    #[test]
    fn trusted_rpc_authority_adopts_and_fences_allowed_owner() {
        let authority =
            TrustedRpcZoneOwnerLeaseAuthority::new(["home-a".to_string()]).expect("authority");
        let zone = ZoneId::primary();
        let first = ZoneOwnerLease::new(zone.clone(), "home-a", 2);
        authority
            .validate_owner_lease(&first)
            .expect("first authenticated lease should be adopted");
        assert_eq!(authority.owner_lease(&zone), first);

        let stale = ZoneOwnerLease::new(zone.clone(), "home-a", 1);
        assert!(authority.validate_owner_lease(&stale).is_err());
        let next = ZoneOwnerLease::new(zone.clone(), "home-a", 3);
        authority
            .validate_owner_lease(&next)
            .expect("higher fencing token should replace the current lease");
        assert_eq!(authority.owner_lease(&zone), next);
    }

    #[test]
    fn trusted_rpc_authority_rejects_unlisted_owner() {
        let authority =
            TrustedRpcZoneOwnerLeaseAuthority::new(["home-a".to_string()]).expect("authority");
        let error = authority
            .validate_owner_lease(&ZoneOwnerLease::new(ZoneId::primary(), "home-b", 1))
            .expect_err("unlisted owner must be rejected");
        assert!(error.contains("untrusted Zone owner home-b"));
    }

    #[test]
    fn explicit_lease_owner_is_independent_from_process_instance_id() {
        assert_eq!(
            configured_owner_id(
                Some("logical-active".to_string()),
                Some("physical-zone-1".to_string())
            ),
            "logical-active"
        );
        assert_eq!(
            configured_owner_id(None, Some("physical-zone-1".to_string())),
            "physical-zone-1"
        );
    }

    #[test]
    fn postgres_fencing_values_reject_implicit_integer_wrapping() {
        assert_eq!(
            checked_postgres_fencing_token(i64::MAX as u64),
            Ok(i64::MAX)
        );
        assert!(checked_postgres_fencing_token(0).is_err());
        assert!(checked_postgres_fencing_token(i64::MAX as u64 + 1).is_err());
        assert!(checked_postgres_fencing_token(u64::MAX).is_err());
        assert_eq!(postgres_fencing_token(i64::MAX), Ok(i64::MAX as u64));
        assert!(postgres_fencing_token(0).is_err());
        assert!(postgres_fencing_token(-1).is_err());
    }

    #[test]
    fn postgres_lease_times_reject_expiry_and_timestamp_overflow() {
        assert_eq!(postgres_lease_times(0, 1), Ok((0, 1)));
        assert_eq!(
            postgres_lease_times(i64::MAX as u64 - 1, 1),
            Ok((i64::MAX - 1, i64::MAX))
        );
        assert!(postgres_lease_times(i64::MAX as u64, 1).is_err());
        assert!(postgres_lease_times(i64::MAX as u64 + 1, 1).is_err());
        assert!(postgres_lease_times(1, u64::MAX).is_err());
    }

    #[test]
    fn invalid_epoch_and_time_fail_before_connecting_to_postgres() {
        let authority = PostgresZoneOwnerLeaseAuthority::new("not a database URL", "owner", 1);
        let zone = ZoneId::new("out-of-range-without-database");
        let invalid = ZoneOwnerLease::new(zone.clone(), "owner", u64::MAX);
        assert!(authority.validate_owner_lease(&invalid).is_err());
        assert!(authority.renew_at(&invalid, 0).is_err());
        assert!(authority.handoff_at(&invalid, "other", 0).is_err());
        assert!(authority.release(&invalid).is_err());
        assert!(authority.acquire_at(&zone, u64::MAX).is_err());
    }

    #[test]
    fn fallible_getters_and_validation_return_errors_when_postgres_is_unreachable() {
        // Port zero cannot name an existing PostgreSQL listener. Bound connection
        // time too, and never read the configured test/QA/production database URL.
        let authority = PostgresZoneOwnerLeaseAuthority::new(
            "host=127.0.0.1 port=0 user=unavailable dbname=mir2_zone_lease_test_unreachable connect_timeout=1",
            "previous-owner",
            30_000,
        );
        let zone = ZoneId::new("unreachable-pg-must-not-create-a-lease");
        let previous = ZoneOwnerLease::new(zone.clone(), "previous-owner", 7);
        for result in [
            authority.try_owner_lease(&zone),
            authority.try_refresh_owner_lease(&zone),
        ] {
            assert!(result.unwrap_err().contains("zone lease connect failed"));
        }
        assert!(authority
            .validate_owner_lease(&previous)
            .unwrap_err()
            .contains("zone lease connect failed"));
    }

    fn pg_test_url() -> Option<String> {
        let url = match std::env::var("MIR2_TEST_POSTGRES_URL") {
            Ok(url) => url,
            Err(std::env::VarError::NotPresent) => {
                eprintln!(
                    "skipping zone lease PG test: no explicit isolated MIR2_TEST_POSTGRES_URL"
                );
                return None;
            }
            Err(_) => panic!("zone lease test database URL must be valid Unicode"),
        };
        let config = url
            .parse::<postgres::Config>()
            .unwrap_or_else(|_| panic!("invalid zone lease test database configuration"));
        let suffix = config
            .get_dbname()
            .and_then(|database| database.strip_prefix("mir2_zone_lease_test_"))
            .filter(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            });
        assert!(
            suffix.is_some(),
            "zone lease tests require a dedicated mir2_zone_lease_test_* database"
        );
        Client::connect(&url, NoTls)
            .unwrap_or_else(|_| panic!("explicit zone lease test database is unavailable"));
        Some(url)
    }

    fn unique_zone(label: &str) -> ZoneId {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        ZoneId::new(format!("test-zone-{label}-{}-{nanos}", std::process::id()))
    }

    fn cleanup(url: &str, zone: &ZoneId) {
        if let Ok(mut client) = Client::connect(url, NoTls) {
            let _ = client.execute(
                "DELETE FROM zone_owner_leases WHERE zone_id = $1",
                &[&zone.as_str()],
            );
        }
    }

    #[test]
    fn acquire_renew_and_fence_on_steal() {
        let Some(url) = pg_test_url() else {
            eprintln!("skipping zone lease test because Postgres is unavailable");
            return;
        };
        let zone = unique_zone("steal");
        cleanup(&url, &zone);

        let owner_a = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 1000);
        let owner_b = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-B", 1000);

        // A acquires at t=0 with token 1.
        let lease_a = owner_a.acquire_at(&zone, 0).expect("A acquires");
        assert_eq!(lease_a.owner_id(), "instance-A");
        assert_eq!(lease_a.fencing_token(), 1);

        // A renews before expiry, keeping the same token.
        let renewed = owner_a.renew_at(&lease_a, 500).expect("A renews");
        assert_eq!(renewed.fencing_token(), 1);

        // B tries to acquire while A's lease is still live -> observes A, not owner.
        let observed = owner_b.acquire_at(&zone, 800).expect("B observes live A");
        assert_eq!(observed.owner_id(), "instance-A");
        assert_eq!(observed.fencing_token(), 1);

        // After A's lease expires (t >= 1500), B steals with a bumped fencing token.
        let lease_b = owner_b
            .acquire_at(&zone, 2000)
            .expect("B steals expired lease");
        assert_eq!(lease_b.owner_id(), "instance-B");
        assert_eq!(lease_b.fencing_token(), 2, "steal must bump fencing token");

        // A is now fenced: renewing its stale token must fail.
        let stale = owner_a.renew_at(&lease_a, 2100);
        assert!(stale.is_err(), "zombie owner A must be fenced out");

        // B can renew its fresh lease.
        owner_b.renew_at(&lease_b, 2200).expect("B renews");

        cleanup(&url, &zone);
    }

    #[test]
    fn release_lets_peer_take_over_immediately() {
        let Some(url) = pg_test_url() else {
            eprintln!("skipping zone lease release test because Postgres is unavailable");
            return;
        };
        let zone = unique_zone("release");
        cleanup(&url, &zone);

        let owner_a = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 60_000);
        let owner_b = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-B", 60_000);

        let lease_a = owner_a.acquire_at(&zone, 0).expect("A acquires");
        // Before release, B cannot take the still-live lease.
        let blocked = owner_b.acquire_at(&zone, 100).expect("B observes");
        assert_eq!(blocked.owner_id(), "instance-A");

        owner_a.release(&lease_a).expect("A releases");
        let released = owner_b.observe_at(&zone, 100).unwrap().unwrap();
        assert_eq!(released.owner_id, "instance-A");
        assert_eq!(released.fencing_token, lease_a.fencing_token());
        assert!(
            !released.is_live(0),
            "release retains an expired historical row"
        );
        // A released, so B takes a strictly newer epoch without waiting for TTL.
        let lease_b = owner_b
            .acquire_at(&zone, 200)
            .expect("B acquires after release");
        assert_eq!(lease_b.owner_id(), "instance-B");
        assert_eq!(lease_b.fencing_token(), lease_a.fencing_token() + 1);

        let observed = owner_b
            .observe_at(&zone, 250)
            .expect("observe")
            .expect("lease present");
        assert_eq!(observed.owner_id, "instance-B");
        assert!(observed.is_live(250));

        cleanup(&url, &zone);
    }

    #[test]
    fn release_and_reacquire_same_owner_advance_epoch_and_stale_release_is_harmless() {
        let Some(url) = pg_test_url() else { return };
        let zone = unique_zone("same-owner-release");
        let first = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 60_000);
        let restarted = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 60_000);
        let lease = first.acquire_at(&zone, 100).unwrap();
        first.release(&lease).unwrap();
        let released = restarted.observe_at(&zone, 101).unwrap().unwrap();
        first.release(&lease).unwrap();
        assert_eq!(restarted.observe_at(&zone, 101).unwrap().unwrap(), released);

        let next = restarted.acquire_at(&zone, 102).unwrap();
        assert!(next.fencing_token() > lease.fencing_token());
        let before_stale_release = restarted.observe_at(&zone, 103).unwrap().unwrap();
        first.release(&lease).unwrap();
        assert_eq!(
            restarted.observe_at(&zone, 103).unwrap().unwrap(),
            before_stale_release
        );
        assert!(first.renew_at(&lease, 103).is_err());
        cleanup(&url, &zone);
    }

    #[test]
    fn expired_same_owner_reacquisition_starts_a_new_epoch_at_the_expiry_boundary() {
        let Some(url) = pg_test_url() else { return };
        let zone = unique_zone("same-owner-expiry");
        let first = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 1_000);
        let restarted = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 1_000);
        let lease = first.acquire_at(&zone, 100).unwrap();
        let next = restarted.acquire_at(&zone, 1_100).unwrap();
        assert_eq!(next.fencing_token(), lease.fencing_token() + 1);
        assert!(first.renew_at(&lease, 1_101).is_err());
        first.release(&lease).unwrap();
        assert!(restarted
            .observe_at(&zone, 1_101)
            .unwrap()
            .unwrap()
            .is_live(1_101));
        let same_epoch = restarted.acquire_at(&zone, 1_102).unwrap();
        assert_eq!(same_epoch, next, "a live owner may renew its current epoch");
        cleanup(&url, &zone);
    }

    #[test]
    fn independent_authorities_reject_previous_live_owner_and_refresh_peer_observations() {
        let Some(url) = pg_test_url() else { return };
        let zone = unique_zone("strict-current-row");
        let owner_a = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 60_000);
        let owner_b = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-B", 60_000);
        let observer = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "observer", 60_000);
        let first = owner_a.owner_lease(&zone);
        owner_a.validate_owner_lease(&first).unwrap();
        assert_eq!(observer.owner_lease(&zone), first);
        let next = owner_a.handoff_at(&first, "instance-B", now_ms()).unwrap();

        assert!(owner_a.validate_owner_lease(&first).is_err());
        assert!(owner_b.validate_owner_lease(&first).is_err());
        assert!(owner_a.renew_at(&first, now_ms()).is_err());
        assert_eq!(owner_a.owner_lease(&zone), next);
        assert_eq!(observer.owner_lease(&zone), next);
        assert_eq!(owner_a.refresh_owner_lease(&zone), next);
        owner_b.validate_owner_lease(&next).unwrap();
        owner_a.release(&first).unwrap();
        assert_eq!(
            owner_b
                .observe_at(&zone, now_ms())
                .unwrap()
                .unwrap()
                .fencing_token,
            next.fencing_token()
        );

        let mut called = false;
        assert!(owner_a
            .with_durable_npc_purchase_lease(&first, &mut || called = true)
            .is_err());
        assert!(!called, "stale owner must not enter the durable callback");
        owner_b
            .with_durable_npc_purchase_lease(&next, &mut || called = true)
            .unwrap();
        assert!(called);
        cleanup(&url, &zone);
    }

    #[test]
    fn validation_is_read_only_and_cannot_register_or_resurrect_a_lease() {
        let Some(url) = pg_test_url() else { return };
        let zone = unique_zone("validation-no-acquire");
        let authority = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 1_000);
        let invented = ZoneOwnerLease::new(zone.clone(), "instance-A", 1);
        assert!(authority.validate_owner_lease(&invented).is_err());
        assert!(authority.observe_at(&zone, now_ms()).unwrap().is_none());
        let expired = authority.acquire_at(&zone, 0).unwrap();
        let before = authority.observe_at(&zone, now_ms()).unwrap().unwrap();
        assert!(authority.validate_owner_lease(&expired).is_err());
        assert_eq!(
            authority.observe_at(&zone, now_ms()).unwrap().unwrap(),
            before
        );
        authority.release(&expired).unwrap();
        let released = authority.observe_at(&zone, now_ms()).unwrap().unwrap();
        assert!(authority.validate_owner_lease(&expired).is_err());
        assert_eq!(
            authority.observe_at(&zone, now_ms()).unwrap().unwrap(),
            released
        );
        cleanup(&url, &zone);
    }

    #[test]
    fn release_refuses_another_authority_and_preserves_the_exact_row() {
        let Some(url) = pg_test_url() else { return };
        let zone = unique_zone("release-foreign");
        let owner_a = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 1_000);
        let owner_b = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-B", 1_000);
        let lease = owner_a.acquire_at(&zone, 100).unwrap();
        let before = owner_a.observe_at(&zone, 100).unwrap().unwrap();
        assert!(owner_b.release(&lease).is_err());
        assert_eq!(owner_a.observe_at(&zone, 100).unwrap().unwrap(), before);
        cleanup(&url, &zone);
    }

    #[test]
    fn bigint_epoch_exhaustion_refuses_reacquisition_and_handoff_without_row_changes() {
        let Some(url) = pg_test_url() else { return };
        let zone = unique_zone("epoch-exhaustion");
        let owner_a = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 1_000);
        let owner_b = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-B", 1_000);
        owner_a.acquire_at(&zone, 0).unwrap();
        let mut client = Client::connect(&url, NoTls).unwrap();
        assert_eq!(
            client
                .execute(
                    "UPDATE zone_owner_leases SET fencing_token=$2 WHERE zone_id=$1",
                    &[&zone.as_str(), &i64::MAX],
                )
                .unwrap(),
            1
        );
        let max_lease = ZoneOwnerLease::new(zone.clone(), "instance-A", i64::MAX as u64);
        let before = owner_b.observe_at(&zone, 0).unwrap().unwrap();
        assert!(owner_a
            .handoff_at(&max_lease, "instance-B", 500)
            .unwrap_err()
            .contains("exhausted"));
        assert_eq!(owner_b.observe_at(&zone, 500).unwrap().unwrap(), before);
        assert_eq!(owner_a.acquire_at(&zone, 500).unwrap(), max_lease);
        let live_max = owner_a.observe_at(&zone, 500).unwrap().unwrap();
        assert!(owner_a
            .acquire_at(&zone, 1_500)
            .unwrap_err()
            .contains("exhausted"));
        assert!(owner_b
            .acquire_at(&zone, 1_500)
            .unwrap_err()
            .contains("exhausted"));
        assert_eq!(owner_a.observe_at(&zone, 1_500).unwrap().unwrap(), live_max);
        cleanup(&url, &zone);
    }

    #[test]
    fn concurrent_independent_acquisitions_choose_one_live_owner_and_one_epoch() {
        let Some(url) = pg_test_url() else { return };
        let zone = unique_zone("concurrent-acquire");
        let owner_a = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 60_000);
        let owner_b = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-B", 60_000);
        // Initialize both real connections before racing the atomic acquire.
        assert!(owner_a.observe_at(&zone, 100).unwrap().is_none());
        assert!(owner_b.observe_at(&zone, 100).unwrap().is_none());
        let barrier = std::sync::Barrier::new(2);
        let (first, second) = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                owner_a.acquire_at(&zone, 100)
            });
            let second = scope.spawn(|| {
                barrier.wait();
                owner_b.acquire_at(&zone, 100)
            });
            (
                first.join().unwrap().unwrap(),
                second.join().unwrap().unwrap(),
            )
        });
        assert_eq!(first, second);
        assert_eq!(first.fencing_token(), 1);
        assert!(matches!(first.owner_id(), "instance-A" | "instance-B"));
        cleanup(&url, &zone);
    }

    #[test]
    fn invalid_stored_epoch_is_not_repaired_or_rewritten_by_acquisition() {
        let Some(url) = pg_test_url() else { return };
        let zone = unique_zone("invalid-stored-epoch");
        let authority = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-A", 1_000);
        authority.acquire_at(&zone, 0).unwrap();
        let mut client = Client::connect(&url, NoTls).unwrap();
        for invalid in [0_i64, -1] {
            client
                .execute(
                    "UPDATE zone_owner_leases SET fencing_token=$2 WHERE zone_id=$1",
                    &[&zone.as_str(), &invalid],
                )
                .unwrap();
            let raw_row = |client: &mut Client| -> String {
                client.query_one(
                    "SELECT row_to_json(lease)::text FROM zone_owner_leases lease WHERE zone_id=$1",
                    &[&zone.as_str()],
                ).unwrap().get(0)
            };
            let before = raw_row(&mut client);
            assert!(authority
                .acquire_at(&zone, 100)
                .unwrap_err()
                .contains("invalid"));
            assert!(authority
                .acquire_at(&zone, 1_000)
                .unwrap_err()
                .contains("invalid"));
            assert_eq!(
                raw_row(&mut client),
                before,
                "invalid epochs are never repaired implicitly"
            );
            assert!(authority.observe_at(&zone, 1_000).is_err());
        }
        cleanup(&url, &zone);
    }

    #[test]
    fn trait_owner_lease_acquires_and_renews_same_token() {
        let Some(url) = pg_test_url() else {
            eprintln!("skipping zone lease trait test because Postgres is unavailable");
            return;
        };
        let zone = unique_zone("trait");
        cleanup(&url, &zone);

        let authority = PostgresZoneOwnerLeaseAuthority::new(url.clone(), "instance-T", 60_000);
        let lease = authority.owner_lease(&zone);
        assert_eq!(lease.owner_id(), "instance-T");
        assert_eq!(lease.fencing_token(), 1);
        // owner_lease again should renew, not bump the token (same owner).
        let again = authority.owner_lease(&zone);
        assert_eq!(again.fencing_token(), 1);
        // Validation against the held lease should pass.
        authority.validate_owner_lease(&lease).expect("lease valid");

        cleanup(&url, &zone);
    }
}

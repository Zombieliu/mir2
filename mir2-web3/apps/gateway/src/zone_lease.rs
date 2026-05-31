//! Durable, fenced zone-ownership leases backed by Postgres.
//!
//! This is the coordination primitive for distributed gateway failover. A single
//! `zone_owner_leases` row per zone records the current owner, a monotonic fencing
//! token, and an absolute expiry. Ownership semantics:
//!
//! - **acquire**: claim the zone if it is unowned, owned by us, or expired. A steal
//!   from a *different, expired* owner bumps the fencing token; renewing our own
//!   lease keeps it. If a *live* different owner holds it, we observe their lease
//!   and learn we are not the owner.
//! - **renew (heartbeat)**: extend expiry iff we still hold the lease with a
//!   matching fencing token and it has not expired. Otherwise the lease is lost.
//! - **release**: drop the lease on graceful shutdown so a peer can take over
//!   immediately instead of waiting for TTL.
//!
//! A returning "zombie" owner is fenced out: its stale token fails renewal, and any
//! token-checked write is rejected by the new epoch's higher token.
//!
//! `postgres` (sync) creates its own runtime, so every DB call runs on a freshly
//! spawned OS thread — the same pattern the simulation account store uses to avoid
//! nested-Tokio-runtime panics inside the async gateway.
//!
//! NOTE: this makes ownership *coordination* production-grade. Transferring the
//! live in-memory zone runtime to a new owner still depends on the zone-owner RPC
//! transport, which is tracked separately.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use postgres::{Client, NoTls};

use crate::routing::{
    SharedZoneOwnerLeaseAuthority, ZoneId, ZoneOwnerLease, ZoneOwnerLeaseAuthority,
};

const DEFAULT_LEASE_TTL_MS: u64 = 30_000;

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
    client: Arc<Mutex<Option<Client>>>,
    last_known: Mutex<BTreeMap<String, ZoneOwnerLease>>,
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
            client: Arc::new(Mutex::new(None)),
            last_known: Mutex::new(BTreeMap::new()),
        }
    }

    /// Build from the environment. Returns `None` when no lease database URL is
    /// configured, so the caller can fall back to the in-process authority.
    pub fn from_env() -> Option<Self> {
        let database_url = std::env::var("MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())?;
        let owner_id = std::env::var("MIR2_GATEWAY_INSTANCE_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(default_instance_id);
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

    fn remember(&self, lease: &ZoneOwnerLease) {
        if let Ok(mut cache) = self.last_known.lock() {
            cache.insert(lease.zone_id().as_str().to_string(), lease.clone());
        }
    }

    fn cached(&self, zone_id: &ZoneId) -> Option<ZoneOwnerLease> {
        self.last_known
            .lock()
            .ok()
            .and_then(|cache| cache.get(zone_id.as_str()).cloned())
    }

    /// Atomically acquire/renew/steal the lease, returning the resulting stored
    /// lease (whose owner may be a *different* live process).
    pub fn acquire_at(&self, zone_id: &ZoneId, now_ms: u64) -> Result<ZoneOwnerLease, String> {
        let zone = zone_id.as_str().to_string();
        let owner_id = self.owner_id.clone();
        let expires_at = now_ms.saturating_add(self.lease_ttl_ms) as i64;
        let now = now_ms as i64;
        let zone_for_lease = zone_id.clone();
        let lease = self.run(move |client| {
            let row = client
                .query_one(
                    "INSERT INTO zone_owner_leases \
                     (zone_id, owner_id, fencing_token, expires_at_ms, heartbeat_at_ms, acquired_at_ms) \
                     VALUES ($1, $2, 1, $3, $4, $4) \
                     ON CONFLICT (zone_id) DO UPDATE SET \
                       fencing_token = CASE \
                           WHEN zone_owner_leases.owner_id = EXCLUDED.owner_id THEN zone_owner_leases.fencing_token \
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
                           WHEN zone_owner_leases.owner_id <> EXCLUDED.owner_id AND zone_owner_leases.expires_at_ms <= $4 \
                               THEN EXCLUDED.acquired_at_ms ELSE zone_owner_leases.acquired_at_ms END, \
                       updated_at = now() \
                     RETURNING owner_id, fencing_token",
                    &[&zone, &owner_id, &expires_at, &now],
                )
                .map_err(|error| format!("zone lease acquire failed for {zone}: {error}"))?;
            let owner_id: String = row.get("owner_id");
            let fencing_token: i64 = row.get("fencing_token");
            Ok(ZoneOwnerLease::new(
                zone_for_lease,
                owner_id,
                fencing_token.max(1) as u64,
            ))
        })?;
        self.remember(&lease);
        Ok(lease)
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
        let token = lease.fencing_token() as i64;
        let expires_at = now_ms.saturating_add(self.lease_ttl_ms) as i64;
        let now = now_ms as i64;
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
                self.remember(&lease);
                Ok(lease)
            }
            Err(error) => Err(error),
        }
    }

    /// Release the lease on graceful shutdown so a peer can take over immediately.
    pub fn release(&self, lease: &ZoneOwnerLease) -> Result<(), String> {
        let zone = lease.zone_id().as_str().to_string();
        let owner_id = self.owner_id.clone();
        let token = lease.fencing_token() as i64;
        self.run(move |client| {
            client
                .execute(
                    "DELETE FROM zone_owner_leases \
                     WHERE zone_id = $1 AND owner_id = $2 AND fencing_token = $3",
                    &[&zone, &owner_id, &token],
                )
                .map_err(|error| format!("zone lease release failed for {zone}: {error}"))?;
            Ok(())
        })
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
            Ok(row.map(|row| {
                let fencing_token: i64 = row.get("fencing_token");
                let expires_at_ms: i64 = row.get("expires_at_ms");
                let heartbeat_at_ms: i64 = row.get("heartbeat_at_ms");
                ObservedZoneLease {
                    owner_id: row.get("owner_id"),
                    fencing_token: fencing_token.max(0) as u64,
                    expires_at_ms: expires_at_ms.max(0) as u64,
                    heartbeat_at_ms: heartbeat_at_ms.max(0) as u64,
                }
            }))
        })
    }
}

impl ZoneOwnerLeaseAuthority for PostgresZoneOwnerLeaseAuthority {
    fn owner_lease(&self, zone_id: &ZoneId) -> ZoneOwnerLease {
        let now = now_ms();
        match self.acquire_at(zone_id, now) {
            Ok(lease) => lease,
            Err(error) => {
                // Transient DB unavailability must not relinquish ownership: return
                // the last known good lease, or a stable bootstrap lease for this
                // owner. Fencing is reconciled by renew/acquire once the DB heals.
                eprintln!(
                    "zone lease authority degraded for {}: {error}",
                    zone_id.as_str()
                );
                self.cached(zone_id).unwrap_or_else(|| {
                    ZoneOwnerLease::new(zone_id.clone(), self.owner_id.clone(), 1)
                })
            }
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
mod tests {
    use super::*;

    fn pg_test_url() -> Option<String> {
        let url = std::env::var("MIR2_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2".into());
        Client::connect(&url, NoTls).ok().map(|_| url)
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
        // A released, so the row is gone -> B acquires fresh (token resets to 1).
        let lease_b = owner_b
            .acquire_at(&zone, 200)
            .expect("B acquires after release");
        assert_eq!(lease_b.owner_id(), "instance-B");
        assert_eq!(lease_b.fencing_token(), 1);

        let observed = owner_b
            .observe_at(&zone, 250)
            .expect("observe")
            .expect("lease present");
        assert_eq!(observed.owner_id, "instance-B");
        assert!(observed.is_live(250));

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

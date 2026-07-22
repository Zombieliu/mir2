use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ZoneHostHealth, ZoneId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoneHostLifecycle {
    Active,
    Draining,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostRegistration {
    pub host_id: String,
    pub endpoint: String,
    pub failure_domain: String,
    pub max_sessions: usize,
    pub max_zones: usize,
    pub weight: u32,
}

impl ZoneHostRegistration {
    pub fn from_health(
        endpoint: impl Into<String>,
        failure_domain: impl Into<String>,
        weight: u32,
        health: &ZoneHostHealth,
    ) -> Self {
        Self {
            host_id: health.host_id.clone(),
            endpoint: endpoint.into(),
            failure_domain: failure_domain.into(),
            max_sessions: health.session_capacity,
            max_zones: health.zone_capacity,
            weight,
        }
    }

    fn validate(&self) -> Result<(), String> {
        validate_component("Zone Host id", &self.host_id)?;
        validate_component("Zone Host endpoint", &self.endpoint)?;
        validate_component("Zone Host failure domain", &self.failure_domain)?;
        if self.max_sessions == 0 {
            return Err("Zone Host max_sessions must be positive".to_string());
        }
        if self.max_zones == 0 {
            return Err("Zone Host max_zones must be positive".to_string());
        }
        if self.weight == 0 {
            return Err("Zone Host weight must be positive".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneHostHeartbeat {
    pub session_count: usize,
    pub active_connections: usize,
    pub observed_at_ms: u64,
}

impl ZoneHostHeartbeat {
    pub fn from_health(health: &ZoneHostHealth, observed_at_ms: u64) -> Self {
        Self {
            session_count: health.session_count,
            active_connections: health.active_connections,
            observed_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZonePlacementEndpoint {
    pub host_id: String,
    pub endpoint: String,
    pub failure_domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZonePlacementLease {
    pub zone_id: ZoneId,
    pub generation: u64,
    pub primary: ZonePlacementEndpoint,
    pub replicas: Vec<ZonePlacementEndpoint>,
    pub expires_at_ms: u64,
}

impl ZonePlacementLease {
    pub fn endpoints(&self) -> Vec<String> {
        std::iter::once(self.primary.endpoint.clone())
            .chain(self.replicas.iter().map(|host| host.endpoint.clone()))
            .collect()
    }

    pub fn host_ids(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.primary.host_id.as_str())
            .chain(self.replicas.iter().map(|host| host.host_id.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneRebalanceMove {
    pub zone_id: ZoneId,
    pub previous_generation: u64,
    pub next: ZonePlacementLease,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneHostSnapshot {
    pub registration: ZoneHostRegistration,
    pub lifecycle: ZoneHostLifecycle,
    pub heartbeat: ZoneHostHeartbeat,
    pub assigned_zones: usize,
    pub healthy: bool,
}

#[derive(Debug)]
struct ZoneHostRecord {
    registration: ZoneHostRegistration,
    lifecycle: ZoneHostLifecycle,
    heartbeat: ZoneHostHeartbeat,
}

#[derive(Debug, Default)]
struct ZoneHostControlState {
    hosts: BTreeMap<String, ZoneHostRecord>,
    placements: BTreeMap<ZoneId, ZonePlacementLease>,
}

/// In-memory scheduler state machine used by a single control-plane leader.
/// Its methods take an explicit clock so the placement and failover rules are
/// deterministic and can later be driven by the Commonware replicated log.
#[derive(Debug)]
pub struct ZoneHostControlPlane {
    state: Mutex<ZoneHostControlState>,
    heartbeat_ttl_ms: u64,
    placement_lease_ttl_ms: u64,
    replica_count: usize,
}

impl ZoneHostControlPlane {
    pub fn new(heartbeat_ttl_ms: u64, placement_lease_ttl_ms: u64, replica_count: usize) -> Self {
        Self {
            state: Mutex::new(ZoneHostControlState::default()),
            heartbeat_ttl_ms: heartbeat_ttl_ms.max(1),
            placement_lease_ttl_ms: placement_lease_ttl_ms.max(1),
            replica_count,
        }
    }

    pub fn register_host(
        &self,
        registration: ZoneHostRegistration,
        heartbeat: ZoneHostHeartbeat,
    ) -> Result<(), String> {
        registration.validate()?;
        if heartbeat.session_count > registration.max_sessions {
            return Err(format!(
                "Zone Host {} reports {} sessions above capacity {}",
                registration.host_id, heartbeat.session_count, registration.max_sessions
            ));
        }
        let mut state = self.lock_state()?;
        state.hosts.insert(
            registration.host_id.clone(),
            ZoneHostRecord {
                registration,
                lifecycle: ZoneHostLifecycle::Active,
                heartbeat,
            },
        );
        Ok(())
    }

    pub fn heartbeat(&self, host_id: &str, heartbeat: ZoneHostHeartbeat) -> Result<(), String> {
        let mut state = self.lock_state()?;
        let host = state
            .hosts
            .get_mut(host_id)
            .ok_or_else(|| format!("unknown Zone Host {host_id}"))?;
        if heartbeat.observed_at_ms < host.heartbeat.observed_at_ms {
            return Err(format!(
                "stale Zone Host heartbeat for {host_id}: {} < {}",
                heartbeat.observed_at_ms, host.heartbeat.observed_at_ms
            ));
        }
        if heartbeat.session_count > host.registration.max_sessions {
            return Err(format!(
                "Zone Host {host_id} reports {} sessions above capacity {}",
                heartbeat.session_count, host.registration.max_sessions
            ));
        }
        host.heartbeat = heartbeat;
        Ok(())
    }

    pub fn place_zone(&self, zone_id: ZoneId, now_ms: u64) -> Result<ZonePlacementLease, String> {
        let mut state = self.lock_state()?;
        if let Some(current) = state.placements.get(&zone_id).cloned() {
            if self.placement_is_eligible(&state, &current, now_ms) {
                let mut renewed = current;
                renewed.expires_at_ms = now_ms.saturating_add(self.placement_lease_ttl_ms);
                state.placements.insert(zone_id, renewed.clone());
                return Ok(renewed);
            }
        }
        let previous_generation = state
            .placements
            .get(&zone_id)
            .map(|placement| placement.generation)
            .unwrap_or(0);
        let placement =
            self.schedule_locked(&state, zone_id.clone(), previous_generation, now_ms)?;
        state.placements.insert(zone_id, placement.clone());
        Ok(placement)
    }

    pub fn renew_placement(
        &self,
        zone_id: &ZoneId,
        generation: u64,
        now_ms: u64,
    ) -> Result<ZonePlacementLease, String> {
        let mut state = self.lock_state()?;
        let current = state
            .placements
            .get(zone_id)
            .cloned()
            .ok_or_else(|| format!("Zone {} has no placement", zone_id.as_str()))?;
        if current.generation != generation {
            return Err(format!(
                "stale Zone placement generation for {}: current {}, got {}",
                zone_id.as_str(),
                current.generation,
                generation
            ));
        }
        if !self.placement_is_eligible(&state, &current, now_ms) {
            return Err(format!(
                "Zone placement {} generation {} is no longer eligible",
                zone_id.as_str(),
                generation
            ));
        }
        let mut renewed = current;
        renewed.expires_at_ms = now_ms.saturating_add(self.placement_lease_ttl_ms);
        state.placements.insert(zone_id.clone(), renewed.clone());
        Ok(renewed)
    }

    pub fn begin_drain(
        &self,
        host_id: &str,
        now_ms: u64,
    ) -> Result<Vec<ZoneRebalanceMove>, String> {
        let mut state = self.lock_state()?;
        state
            .hosts
            .get_mut(host_id)
            .ok_or_else(|| format!("unknown Zone Host {host_id}"))?
            .lifecycle = ZoneHostLifecycle::Draining;
        self.rebalance_locked(&mut state, now_ms)
    }

    pub fn finish_drain(&self, host_id: &str) -> Result<(), String> {
        let mut state = self.lock_state()?;
        if state
            .placements
            .values()
            .any(|placement| placement.host_ids().any(|assigned| assigned == host_id))
        {
            return Err(format!("Zone Host {host_id} still owns Zone placements"));
        }
        state
            .hosts
            .remove(host_id)
            .ok_or_else(|| format!("unknown Zone Host {host_id}"))?;
        Ok(())
    }

    pub fn rebalance(&self, now_ms: u64) -> Result<Vec<ZoneRebalanceMove>, String> {
        let mut state = self.lock_state()?;
        self.rebalance_locked(&mut state, now_ms)
    }

    pub fn placement(&self, zone_id: &ZoneId) -> Option<ZonePlacementLease> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.placements.get(zone_id).cloned())
    }

    pub fn hosts(&self, now_ms: u64) -> Vec<ZoneHostSnapshot> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };
        state
            .hosts
            .values()
            .map(|host| ZoneHostSnapshot {
                registration: host.registration.clone(),
                lifecycle: host.lifecycle,
                heartbeat: host.heartbeat.clone(),
                assigned_zones: assigned_zone_count(&state, &host.registration.host_id),
                healthy: self.host_is_healthy(host, now_ms),
            })
            .collect()
    }

    fn rebalance_locked(
        &self,
        state: &mut ZoneHostControlState,
        now_ms: u64,
    ) -> Result<Vec<ZoneRebalanceMove>, String> {
        let zones = state
            .placements
            .iter()
            .filter_map(|(zone_id, placement)| {
                (!self.placement_is_eligible(state, placement, now_ms))
                    .then_some((zone_id.clone(), placement.generation))
            })
            .collect::<Vec<_>>();
        let mut moves = Vec::with_capacity(zones.len());
        for (zone_id, previous_generation) in zones {
            let next = self.schedule_locked(state, zone_id.clone(), previous_generation, now_ms)?;
            state.placements.insert(zone_id.clone(), next.clone());
            moves.push(ZoneRebalanceMove {
                zone_id,
                previous_generation,
                next,
            });
        }
        Ok(moves)
    }

    fn schedule_locked(
        &self,
        state: &ZoneHostControlState,
        zone_id: ZoneId,
        previous_generation: u64,
        now_ms: u64,
    ) -> Result<ZonePlacementLease, String> {
        let needed = self.replica_count.saturating_add(1);
        let mut candidates = state
            .hosts
            .values()
            .filter(|host| self.host_is_schedulable(state, host, &zone_id, now_ms))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|host| host_rank(state, host, &zone_id));
        if candidates.len() < needed {
            return Err(format!(
                "Zone {} needs {needed} healthy hosts, only {} have capacity",
                zone_id.as_str(),
                candidates.len()
            ));
        }

        let mut selected = Vec::with_capacity(needed);
        let mut failure_domains = BTreeSet::new();
        while selected.len() < needed {
            let distinct = candidates.iter().position(|host| {
                !selected.contains(&host.registration.host_id)
                    && !failure_domains.contains(&host.registration.failure_domain)
            });
            let index = distinct.or_else(|| {
                candidates
                    .iter()
                    .position(|host| !selected.contains(&host.registration.host_id))
            });
            let Some(index) = index else {
                break;
            };
            let host = candidates[index];
            selected.push(host.registration.host_id.clone());
            failure_domains.insert(host.registration.failure_domain.clone());
        }
        if selected.len() < needed {
            return Err(format!(
                "Zone {} could not satisfy replication factor {}",
                zone_id.as_str(),
                self.replica_count
            ));
        }
        let endpoints = selected
            .iter()
            .filter_map(|host_id| state.hosts.get(host_id))
            .map(|host| ZonePlacementEndpoint {
                host_id: host.registration.host_id.clone(),
                endpoint: host.registration.endpoint.clone(),
                failure_domain: host.registration.failure_domain.clone(),
            })
            .collect::<Vec<_>>();
        Ok(ZonePlacementLease {
            zone_id,
            generation: previous_generation.saturating_add(1).max(1),
            primary: endpoints[0].clone(),
            replicas: endpoints[1..].to_vec(),
            expires_at_ms: now_ms.saturating_add(self.placement_lease_ttl_ms),
        })
    }

    fn placement_is_eligible(
        &self,
        state: &ZoneHostControlState,
        placement: &ZonePlacementLease,
        now_ms: u64,
    ) -> bool {
        placement.expires_at_ms > now_ms
            && placement.host_ids().all(|host_id| {
                state
                    .hosts
                    .get(host_id)
                    .is_some_and(|host| self.host_is_healthy(host, now_ms))
                    && state
                        .hosts
                        .get(host_id)
                        .is_some_and(|host| host.lifecycle == ZoneHostLifecycle::Active)
            })
    }

    fn host_is_schedulable(
        &self,
        state: &ZoneHostControlState,
        host: &ZoneHostRecord,
        zone_id: &ZoneId,
        now_ms: u64,
    ) -> bool {
        self.host_is_healthy(host, now_ms)
            && host.lifecycle == ZoneHostLifecycle::Active
            && host.heartbeat.session_count < host.registration.max_sessions
            && assigned_zone_count_excluding(state, &host.registration.host_id, zone_id)
                < host.registration.max_zones
    }

    fn host_is_healthy(&self, host: &ZoneHostRecord, now_ms: u64) -> bool {
        now_ms.saturating_sub(host.heartbeat.observed_at_ms) <= self.heartbeat_ttl_ms
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, ZoneHostControlState>, String> {
        self.state
            .lock()
            .map_err(|_| "Zone Host control-plane mutex poisoned".to_string())
    }
}

impl Default for ZoneHostControlPlane {
    fn default() -> Self {
        Self::new(15_000, 30_000, 1)
    }
}

fn assigned_zone_count(state: &ZoneHostControlState, host_id: &str) -> usize {
    state
        .placements
        .values()
        .filter(|placement| placement.host_ids().any(|assigned| assigned == host_id))
        .count()
}

fn assigned_zone_count_excluding(
    state: &ZoneHostControlState,
    host_id: &str,
    excluded_zone: &ZoneId,
) -> usize {
    state
        .placements
        .values()
        .filter(|placement| {
            &placement.zone_id != excluded_zone
                && placement.host_ids().any(|assigned| assigned == host_id)
        })
        .count()
}

fn host_rank(
    state: &ZoneHostControlState,
    host: &ZoneHostRecord,
    zone_id: &ZoneId,
) -> (u128, std::cmp::Reverse<u64>, String) {
    let assigned =
        assigned_zone_count_excluding(state, &host.registration.host_id, zone_id) as u128;
    let zone_load = assigned.saturating_mul(1_000_000) / host.registration.max_zones as u128;
    let session_load = (host.heartbeat.session_count as u128).saturating_mul(1_000_000)
        / host.registration.max_sessions as u128;
    let load = zone_load.saturating_add(session_load).saturating_mul(1_000)
        / host.registration.weight as u128;
    (
        load,
        std::cmp::Reverse(rendezvous_hash(zone_id, &host.registration.host_id)),
        host.registration.host_id.clone(),
    )
}

fn rendezvous_hash(zone_id: &ZoneId, host_id: &str) -> u64 {
    let mut hash = Sha256::new();
    hash.update(b"obelisk.mir2.zone-placement.v1\0");
    hash.update(zone_id.as_str().as_bytes());
    hash.update([0]);
    hash.update(host_id.as_bytes());
    let digest = hash.finalize();
    u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("SHA-256 prefix is eight bytes"),
    )
}

fn validate_component(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.len() > 255 {
        return Err(format!("{label} exceeds 255 bytes"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{label} contains control characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register(control: &ZoneHostControlPlane, host_id: &str, domain: &str, now_ms: u64) {
        control
            .register_host(
                ZoneHostRegistration {
                    host_id: host_id.to_string(),
                    endpoint: format!("127.0.0.1:{}", 30_000 + host_id.len()),
                    failure_domain: domain.to_string(),
                    max_sessions: 100,
                    max_zones: 2,
                    weight: 100,
                },
                ZoneHostHeartbeat {
                    session_count: 0,
                    active_connections: 0,
                    observed_at_ms: now_ms,
                },
            )
            .expect("host should register");
    }

    #[test]
    fn scheduler_spreads_replicas_across_failure_domains_and_keeps_a_stable_lease() {
        let control = ZoneHostControlPlane::new(100, 1_000, 1);
        register(&control, "host-a", "az-a", 10);
        register(&control, "host-b", "az-b", 10);
        register(&control, "host-c", "az-a", 10);

        let first = control
            .place_zone(ZoneId::new("map:0"), 10)
            .expect("Zone should place");
        let renewed = control
            .place_zone(ZoneId::new("map:0"), 20)
            .expect("stable placement should renew");

        assert_eq!(first.generation, 1);
        assert_eq!(renewed.generation, 1);
        assert_eq!(first.primary.host_id, renewed.primary.host_id);
        assert_ne!(
            first.primary.failure_domain,
            first.replicas[0].failure_domain
        );
        assert_eq!(renewed.expires_at_ms, 1_020);
    }

    #[test]
    fn drain_rebalances_with_a_new_generation_and_fences_the_old_lease() {
        let control = ZoneHostControlPlane::new(100, 1_000, 1);
        register(&control, "host-a", "az-a", 10);
        register(&control, "host-b", "az-b", 10);
        register(&control, "host-c", "az-c", 10);
        let zone = ZoneId::new("map:0");
        let first = control
            .place_zone(zone.clone(), 10)
            .expect("Zone should place");

        let moves = control
            .begin_drain(&first.primary.host_id, 20)
            .expect("drain should rebalance");

        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].previous_generation, 1);
        assert_eq!(moves[0].next.generation, 2);
        assert!(!moves[0]
            .next
            .host_ids()
            .any(|host_id| host_id == first.primary.host_id));
        let error = control
            .renew_placement(&zone, first.generation, 21)
            .expect_err("old generation must be fenced");
        assert!(error.contains("current 2, got 1"));
        control
            .finish_drain(&first.primary.host_id)
            .expect("empty draining host can leave");
    }

    #[test]
    fn expired_heartbeat_triggers_failover_and_capacity_is_enforced() {
        let control = ZoneHostControlPlane::new(10, 1_000, 0);
        register(&control, "host-a", "az-a", 10);
        register(&control, "host-b", "az-b", 10);
        let first = control
            .place_zone(ZoneId::new("map:0"), 10)
            .expect("Zone should place");
        let survivor = if first.primary.host_id == "host-a" {
            "host-b"
        } else {
            "host-a"
        };
        control
            .heartbeat(
                survivor,
                ZoneHostHeartbeat {
                    session_count: 0,
                    active_connections: 0,
                    observed_at_ms: 25,
                },
            )
            .expect("survivor heartbeat");

        let moves = control
            .rebalance(25)
            .expect("expired host should fail over");
        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].next.primary.host_id, survivor);
        assert_eq!(moves[0].next.generation, 2);
    }
}

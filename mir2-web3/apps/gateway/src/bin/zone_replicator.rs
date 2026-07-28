use std::env;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_game_data::crystal_respawn_manifest_ref;
use mir2_gateway::zone_lease::PostgresZoneOwnerLeaseAuthority;
use mir2_gateway::{
    TcpZoneOwnerRpcTransport, ZoneBaseSnapshotStore, ZoneId, ZoneMutationWal, ZoneMutationWalAck,
    ZoneReplicationHead, ZoneRpcLimits, DEFAULT_ZONE_PROMOTION_MAX_LAG_MS,
    DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES, DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES,
};

const DEFAULT_ACTIVE_INTERVAL_MS: u64 = 250;
const DEFAULT_IDLE_INTERVAL_MS: u64 = 5_000;
const DEFAULT_BASE_SNAPSHOT_INTERVAL_ENTRIES: u64 = 512;
const ACTIVE_SESSION_PROGRESS_INTERVAL_MS: u64 = 2_000;
const DEGRADED_PROGRESS_INTERVAL_MS: u64 = 5_000;
const OWNER_RECONCILIATION_INTERVAL_MS: u64 = 1_000;

struct MutationReplicationState {
    wal: ZoneMutationWal,
    snapshot_store: ZoneBaseSnapshotStore,
    last_snapshot_sequence: Option<u64>,
    last_snapshot_session_count: Option<usize>,
    snapshot_interval_entries: u64,
}

struct ReplicationTarget {
    zone_id: ZoneId,
    active: TcpZoneOwnerRpcTransport,
    standby: TcpZoneOwnerRpcTransport,
    active_detector: TcpZoneOwnerRpcTransport,
    standby_detector: TcpZoneOwnerRpcTransport,
    mutation_replication: Option<MutationReplicationState>,
    installed_checksum: Option<String>,
    failover: Option<TargetFailoverState>,
}

#[derive(Clone)]
struct FailoverConfig {
    database_url: String,
    active_owner: String,
    standby_owner: String,
    lease_ttl_ms: u64,
    failure_threshold: usize,
    maximum_lag_ms: u64,
}

struct TargetFailoverState {
    config: FailoverConfig,
    active_owner: String,
    standby_owner: String,
    consecutive_active_failures: usize,
    last_synchronized: Option<(ZoneReplicationHead, u64)>,
    last_reported_synchronization_at_ms: Option<u64>,
    last_reported_session_count: Option<usize>,
    last_degraded_report_at_ms: Option<u64>,
    last_owner_observation_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct ReplicationCadence {
    active: Duration,
    idle: Duration,
}

impl ReplicationCadence {
    fn from_env() -> Self {
        let active = duration_from_env(
            "MIR2_ZONE_REPLICA_INTERVAL_MS",
            DEFAULT_ACTIVE_INTERVAL_MS,
            50,
            30_000,
        );
        let idle = duration_from_env(
            "MIR2_ZONE_REPLICA_IDLE_INTERVAL_MS",
            DEFAULT_IDLE_INTERVAL_MS,
            50,
            300_000,
        );
        Self::new(active, idle)
    }

    fn new(active: Duration, idle: Duration) -> Self {
        Self {
            active,
            idle: idle.max(active),
        }
    }

    fn delay_after_checkpoint(self, session_count: usize) -> Duration {
        if session_count == 0 {
            self.idle
        } else {
            self.active
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("zone-replicator failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let once = env::args().skip(1).any(|argument| argument == "--once");
    let token = env::var("MIR2_ZONE_HOST_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let limits = ZoneRpcLimits::from_env();
    let wal_root = env::var("MIR2_ZONE_REPLICA_WAL_DIR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let failover = failover_config_from_env()?;
    let mut targets = replication_targets(token, limits, wal_root.as_ref(), failover)?;
    let worker_count =
        optional_positive_usize_env("MIR2_ZONE_REPLICA_WORKERS", 1, 256)?.unwrap_or(16);
    let cadence = ReplicationCadence::from_env();
    let mut was_idle = None::<bool>;

    loop {
        let (session_count, errors) = sync_targets(&mut targets, worker_count);
        if !errors.is_empty() {
            for error in &errors {
                eprintln!("zone-replicator target degraded: {error}");
            }
            if once {
                return Err(format!(
                    "{} Zone replication target(s) failed",
                    errors.len()
                ));
            }
        }
        if once {
            return Ok(());
        }
        let idle = session_count == 0;
        if was_idle != Some(idle) {
            let delay = cadence.delay_after_checkpoint(session_count);
            eprintln!(
                "zone-replicator {} cadence: {}ms (sessions={session_count})",
                if idle { "idle" } else { "active" },
                delay.as_millis()
            );
            was_idle = Some(idle);
        }
        thread::sleep(cadence.delay_after_checkpoint(session_count));
    }
}

fn sync_targets(
    targets: &mut [ReplicationTarget],
    requested_workers: usize,
) -> (usize, Vec<String>) {
    let worker_count = requested_workers.clamp(1, targets.len().max(1));
    let chunk_size = targets.len().div_ceil(worker_count).max(1);
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in targets.chunks_mut(chunk_size) {
            handles.push(scope.spawn(move || {
                let mut maximum_sessions = 0;
                let mut errors = Vec::new();
                for target in chunk {
                    match target.sync_once() {
                        Ok(session_count) => {
                            maximum_sessions = maximum_sessions.max(session_count);
                        }
                        Err(error) => {
                            errors.push(format!("{}: {error}", target.zone_id.as_str()));
                        }
                    }
                }
                (maximum_sessions, errors)
            }));
        }
        let mut maximum_sessions = 0;
        let mut errors = Vec::new();
        for handle in handles {
            match handle.join() {
                Ok((session_count, mut worker_errors)) => {
                    maximum_sessions = maximum_sessions.max(session_count);
                    errors.append(&mut worker_errors);
                }
                Err(_) => errors.push("replication worker panicked".to_string()),
            }
        }
        (maximum_sessions, errors)
    })
}

impl ReplicationTarget {
    fn sync_once(&mut self) -> Result<usize, String> {
        match self.synchronize_once() {
            Ok(session_count) => {
                let active_head = self.active.replication_head()?;
                let standby_head = self.standby.replication_head()?;
                if let Some(failover) = self.failover.as_mut() {
                    failover.consecutive_active_failures = 0;
                    failover.last_degraded_report_at_ms = None;
                    if replication_heads_match(&active_head, &standby_head) {
                        let observed_at_ms = now_ms();
                        let synchronization_advanced =
                            failover.last_synchronized.as_ref().is_none_or(|(head, _)| {
                                head.next_sequence != standby_head.next_sequence
                                    || head.latest_digest != standby_head.latest_digest
                            });
                        let session_population_changed =
                            failover.last_reported_session_count != Some(session_count);
                        let active_progress_due = session_count > 0
                            && failover.last_reported_synchronization_at_ms.is_none_or(
                                |reported_at_ms| {
                                    observed_at_ms.saturating_sub(reported_at_ms)
                                        >= ACTIVE_SESSION_PROGRESS_INTERVAL_MS
                                },
                            );
                        let should_report = synchronization_advanced
                            && (failover.last_synchronized.is_none()
                                || session_population_changed
                                || active_progress_due);
                        failover.last_synchronized = Some((standby_head, observed_at_ms));
                        if should_report {
                            let head = failover
                                .last_synchronized
                                .as_ref()
                                .map(|(head, _)| head)
                                .expect("verified synchronization was just recorded");
                            eprintln!(
                                "ZONE_REPLICATOR_SYNCHRONIZED zone={} cursor={} digest={}",
                                self.zone_id.as_str(),
                                head.next_sequence,
                                head.latest_digest
                            );
                            failover.last_reported_synchronization_at_ms = Some(observed_at_ms);
                            failover.last_reported_session_count = Some(session_count);
                        }
                    }
                }
                Ok(session_count)
            }
            Err(sync_error) => {
                if self.reconcile_external_owner_handoff()? {
                    Ok(0)
                } else {
                    self.handle_sync_failure(sync_error)
                }
            }
        }
    }

    fn reconcile_external_owner_handoff(&mut self) -> Result<bool, String> {
        let observed_at_ms = now_ms();
        let (database_url, active_owner, lease_ttl_ms) = {
            let Some(failover) = self.failover.as_mut() else {
                return Ok(false);
            };
            if failover
                .last_owner_observation_at_ms
                .is_some_and(|last_observed_at_ms| {
                    observed_at_ms.saturating_sub(last_observed_at_ms)
                        < OWNER_RECONCILIATION_INTERVAL_MS
                })
            {
                return Ok(false);
            }
            failover.last_owner_observation_at_ms = Some(observed_at_ms);
            (
                failover.config.database_url.clone(),
                failover.active_owner.clone(),
                failover.config.lease_ttl_ms,
            )
        };
        let observer =
            PostgresZoneOwnerLeaseAuthority::new(database_url, active_owner, lease_ttl_ms);
        let Some(observed) = observer.observe_at(&self.zone_id, observed_at_ms)? else {
            return Ok(false);
        };
        self.apply_observed_owner(&observed.owner_id, observed.fencing_token)
    }

    fn apply_observed_owner(
        &mut self,
        observed_owner: &str,
        observed_generation: u64,
    ) -> Result<bool, String> {
        let Some(failover) = self.failover.as_ref() else {
            return Ok(false);
        };
        if observed_owner == failover.active_owner {
            return Ok(false);
        }
        if observed_owner != failover.standby_owner {
            return Err(format!(
                "Zone {} is owned by unconfigured owner {}",
                self.zone_id.as_str(),
                observed_owner
            ));
        }
        let failover = self
            .failover
            .as_mut()
            .expect("failover configuration was checked above");
        std::mem::swap(&mut self.active, &mut self.standby);
        std::mem::swap(&mut self.active_detector, &mut self.standby_detector);
        std::mem::swap(&mut failover.active_owner, &mut failover.standby_owner);
        failover.consecutive_active_failures = 0;
        failover.last_synchronized = None;
        failover.last_reported_synchronization_at_ms = None;
        failover.last_reported_session_count = None;
        failover.last_degraded_report_at_ms = None;
        failover.last_owner_observation_at_ms = None;
        self.installed_checksum = None;
        if let Some(replication) = self.mutation_replication.as_mut() {
            // The previous active may only be quiesced, not yet installed as
            // a replica. Force a fresh exact base on the new owner so the next
            // cycle explicitly demotes and reconstructs the opposite Host
            // before it can ever be promoted back.
            replication.last_snapshot_session_count = None;
        }
        eprintln!(
            "ZONE_REPLICATOR_ROLE_RECONCILED zone={} owner={} generation={}",
            self.zone_id.as_str(),
            observed_owner,
            observed_generation
        );
        Ok(true)
    }

    fn synchronize_once(&mut self) -> Result<usize, String> {
        if let Some(replication) = self.mutation_replication.as_mut() {
            sync_mutation_wal(&self.active, &mut replication.wal)?;
            maybe_persist_base_snapshot(&self.active, replication)?;
            return sync_incremental_standby(&self.active, &self.standby, replication)
                .map(|session_count| session_count.unwrap_or_default());
        }

        let checkpoint = self.active.export_host_checkpoint()?;
        let session_count = checkpoint.session_count;
        if self.installed_checksum.as_deref() != Some(checkpoint.checksum.as_str()) {
            self.standby.install_host_checkpoint(&checkpoint)?;
            eprintln!(
                "zone-replicator {} installed checkpoint {} (entries={}, sessions={})",
                self.zone_id.as_str(),
                checkpoint.checksum,
                checkpoint.entry_count,
                session_count
            );
            self.installed_checksum = Some(checkpoint.checksum);
        }
        Ok(session_count)
    }

    fn handle_sync_failure(&mut self, sync_error: String) -> Result<usize, String> {
        if let Ok(health) = self.active_detector.health() {
            if let Some(failover) = self.failover.as_mut() {
                failover.consecutive_active_failures = 0;
                failover.last_synchronized = None;
                let observed_at_ms = now_ms();
                if failover
                    .last_degraded_report_at_ms
                    .is_none_or(|reported_at_ms| {
                        observed_at_ms.saturating_sub(reported_at_ms)
                            >= DEGRADED_PROGRESS_INTERVAL_MS
                    })
                {
                    eprintln!(
                        "zone-replicator {} standby synchronization degraded while active remains healthy: {sync_error}",
                        self.zone_id.as_str()
                    );
                    failover.last_degraded_report_at_ms = Some(observed_at_ms);
                }
            }
            return Ok(health.session_count);
        }
        let Some(failover) = self.failover.as_mut() else {
            return Err(sync_error);
        };
        failover.consecutive_active_failures =
            failover.consecutive_active_failures.saturating_add(1);
        eprintln!(
            "zone-replicator {} active failure {}/{}: {sync_error}",
            self.zone_id.as_str(),
            failover.consecutive_active_failures,
            failover.config.failure_threshold
        );
        if failover.consecutive_active_failures < failover.config.failure_threshold {
            return Ok(0);
        }
        self.promote_synchronized_standby()
    }

    fn promote_synchronized_standby(&mut self) -> Result<usize, String> {
        let failover = self
            .failover
            .as_mut()
            .ok_or_else(|| "automatic failover is not configured".to_string())?;
        let standby_head = self.standby.replication_head()?;
        let (last_synchronized, synchronized_at_ms) =
            failover.last_synchronized.clone().ok_or_else(|| {
                format!(
                    "Zone {} has no verified synchronized standby head",
                    self.zone_id.as_str()
                )
            })?;
        if let Some(replication) = self.mutation_replication.as_ref() {
            let durable = replication.wal.ack();
            if durable.next_sequence != standby_head.next_sequence
                || durable.latest_digest != standby_head.latest_digest
            {
                return Err(format!(
                    "Zone {} standby cursor/digest is not the durable WAL head",
                    self.zone_id.as_str()
                ));
            }
        }
        let readiness = self.standby.assess_promotion_readiness(
            last_synchronized,
            synchronized_at_ms,
            failover.config.maximum_lag_ms,
        )?;
        if !readiness.ready {
            return Err(format!(
                "Zone {} standby rejected promotion readiness: {} (activeCursor={}, standbyCursor={}, activeDigest={}, standbyDigest={}, lagMs={})",
                self.zone_id.as_str(),
                readiness.reason.as_deref().unwrap_or("unknown"),
                readiness.active_next_sequence,
                readiness.standby_next_sequence,
                readiness.active_latest_digest,
                readiness.standby_latest_digest,
                readiness.observed_lag_ms
            ));
        }
        let readiness_id = readiness
            .readiness_id
            .ok_or_else(|| "ready standby did not return a readiness ID".to_string())?;
        let authority = PostgresZoneOwnerLeaseAuthority::new(
            failover.config.database_url.clone(),
            failover.active_owner.clone(),
            failover.config.lease_ttl_ms,
        );
        let active_lease = authority.acquire_at(&self.zone_id, now_ms())?;
        if active_lease.owner_id() != failover.active_owner {
            return Err(format!(
                "Zone {} lease is owned by {}, expected {}",
                self.zone_id.as_str(),
                active_lease.owner_id(),
                failover.active_owner
            ));
        }
        let promoted_lease =
            authority.handoff_at(&active_lease, failover.standby_owner.clone(), now_ms())?;
        let receipt = match self.standby.promote_replica(readiness_id, &promoted_lease) {
            Ok(receipt) => receipt,
            Err(promotion_error) => {
                let rollback =
                    authority.handoff_at(&promoted_lease, failover.active_owner.clone(), now_ms());
                return Err(match rollback {
                    Ok(rollback_lease) => format!(
                        "Zone {} standby promotion failed after generation {}; owner rolled back to {} at generation {}: {promotion_error}",
                        self.zone_id.as_str(),
                        promoted_lease.fencing_token(),
                        rollback_lease.owner_id(),
                        rollback_lease.fencing_token()
                    ),
                    Err(rollback_error) => format!(
                        "CRITICAL Zone {} standby promotion failed after owner handoff reached generation {}, and owner rollback also failed: promotion={promotion_error}; rollback={rollback_error}",
                        self.zone_id.as_str(),
                        promoted_lease.fencing_token()
                    ),
                });
            }
        };
        std::mem::swap(&mut self.active, &mut self.standby);
        std::mem::swap(&mut self.active_detector, &mut self.standby_detector);
        std::mem::swap(&mut failover.active_owner, &mut failover.standby_owner);
        failover.consecutive_active_failures = 0;
        failover.last_synchronized = None;
        failover.last_reported_synchronization_at_ms = None;
        failover.last_reported_session_count = None;
        failover.last_degraded_report_at_ms = None;
        failover.last_owner_observation_at_ms = None;
        self.installed_checksum = None;
        if let Some(replication) = self.mutation_replication.as_mut() {
            replication.last_snapshot_session_count = None;
        }
        eprintln!(
            "ZONE_REPLICATOR_PROMOTED zone={} owner={} generation={} cursor={} lag_ms={}",
            self.zone_id.as_str(),
            promoted_lease.owner_id(),
            promoted_lease.fencing_token(),
            receipt.head.next_sequence,
            readiness.observed_lag_ms
        );
        Ok(self.active.health()?.session_count)
    }
}

fn replication_targets(
    token: Option<String>,
    limits: ZoneRpcLimits,
    wal_root: Option<&PathBuf>,
    failover: Option<FailoverConfig>,
) -> Result<Vec<ReplicationTarget>, String> {
    let explicit_zone_ids = env::var("MIR2_ZONE_REPLICA_ZONE_IDS")
        .ok()
        .map(|value| parse_zone_ids(&value))
        .transpose()?;
    let active_map_count =
        optional_positive_usize_env("MIR2_ZONE_REPLICA_ACTIVE_MAP_COUNT", 1, 10_000)?;
    let catalog_zone_ids = active_map_count
        .map(|active_map_count| {
            let hot_map = env::var("MIR2_ZONE_REPLICA_HOT_MAP")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "0".to_string());
            let hot_map_lines =
                optional_positive_usize_env("MIR2_ZONE_REPLICA_HOT_MAP_LINES", 1, 10_000)?
                    .unwrap_or(1);
            regional_zone_ids(active_map_count, &hot_map, hot_map_lines)
        })
        .transpose()?;
    if explicit_zone_ids.is_some() && catalog_zone_ids.is_some() {
        return Err(
            "configure MIR2_ZONE_REPLICA_ZONE_IDS or MIR2_ZONE_REPLICA_ACTIVE_MAP_COUNT, not both"
                .to_string(),
        );
    }
    let configured_zone_ids = explicit_zone_ids.or(catalog_zone_ids);
    let zone_ids = configured_zone_ids.clone().unwrap_or_else(|| {
        vec![ZoneId::new(
            env::var("MIR2_ZONE_REPLICA_ZONE_ID")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "primary".to_string()),
        )]
    });
    if zone_ids.len() > 1 && wal_root.is_none() {
        return Err("MIR2_ZONE_REPLICA_WAL_DIR is required for multi-Zone replication".to_string());
    }

    let legacy_active = env::var("MIR2_ZONE_ACTIVE_ADDR").ok();
    let legacy_standby = env::var("MIR2_ZONE_STANDBY_ADDR").ok();
    let role_observer = failover.as_ref().map(|config| {
        PostgresZoneOwnerLeaseAuthority::new(
            config.database_url.clone(),
            config.active_owner.clone(),
            config.lease_ttl_ms,
        )
    });
    let mut targets = Vec::with_capacity(zone_ids.len());
    for (target_index, zone_id) in zone_ids.into_iter().enumerate() {
        let (active_address, standby_address) = if configured_zone_ids.is_some() {
            let (endpoints, _) = TcpZoneOwnerRpcTransport::configured_endpoints_from_env(&zone_id)
                .ok_or_else(|| {
                    format!("Zone {} has no active/standby endpoints", zone_id.as_str())
                })?;
            let active = endpoints
                .first()
                .cloned()
                .ok_or_else(|| format!("Zone {} active endpoint missing", zone_id.as_str()))?;
            let standby = endpoints
                .get(1)
                .cloned()
                .ok_or_else(|| format!("Zone {} standby endpoint missing", zone_id.as_str()))?;
            (active, standby)
        } else {
            (
                legacy_active
                    .clone()
                    .ok_or_else(|| "MIR2_ZONE_ACTIVE_ADDR is required".to_string())?,
                legacy_standby
                    .clone()
                    .ok_or_else(|| "MIR2_ZONE_STANDBY_ADDR is required".to_string())?,
            )
        };
        if active_address == standby_address {
            return Err(format!(
                "Zone {} active and standby addresses must differ",
                zone_id.as_str()
            ));
        }
        let mut active = TcpZoneOwnerRpcTransport::with_options(
            active_address.clone(),
            zone_id.clone(),
            format!("zone-replicator-active-{target_index}"),
            token.clone(),
            limits.clone(),
        );
        let mut standby = TcpZoneOwnerRpcTransport::with_options(
            standby_address.clone(),
            zone_id.clone(),
            format!("zone-replicator-standby-{target_index}"),
            token.clone(),
            limits.clone(),
        );
        let mut detector_limits = limits.clone();
        detector_limits.io_timeout = Duration::from_millis(positive_u64_env(
            "MIR2_ZONE_FAILOVER_DETECT_TIMEOUT_MS",
            250,
            50,
            2_000,
        ));
        let mut active_detector = TcpZoneOwnerRpcTransport::with_options(
            active_address,
            zone_id.clone(),
            format!("zone-replicator-active-detector-{target_index}"),
            token.clone(),
            detector_limits.clone(),
        );
        let mut standby_detector = TcpZoneOwnerRpcTransport::with_options(
            standby_address,
            zone_id.clone(),
            format!("zone-replicator-standby-detector-{target_index}"),
            token.clone(),
            detector_limits,
        );
        let mut target_failover = failover.clone().map(|config| TargetFailoverState {
            active_owner: config.active_owner.clone(),
            standby_owner: config.standby_owner.clone(),
            config,
            consecutive_active_failures: 0,
            last_synchronized: None,
            last_reported_synchronization_at_ms: None,
            last_reported_session_count: None,
            last_degraded_report_at_ms: None,
            last_owner_observation_at_ms: None,
        });
        if let (Some(observer), Some(target_failover)) =
            (role_observer.as_ref(), target_failover.as_mut())
        {
            if let Some(observed) = observer.observe_at(&zone_id, now_ms())? {
                if observed.owner_id == target_failover.standby_owner {
                    std::mem::swap(&mut active, &mut standby);
                    std::mem::swap(&mut active_detector, &mut standby_detector);
                    std::mem::swap(
                        &mut target_failover.active_owner,
                        &mut target_failover.standby_owner,
                    );
                    eprintln!(
                        "zone-replicator {} resumed promoted owner {} generation {}",
                        zone_id.as_str(),
                        observed.owner_id,
                        observed.fencing_token
                    );
                } else if observed.owner_id != target_failover.active_owner {
                    return Err(format!(
                        "Zone {} is owned by unconfigured owner {}",
                        zone_id.as_str(),
                        observed.owner_id
                    ));
                }
            }
        }
        let wal_directory =
            wal_root.map(|root| root.join(hex_zone_directory_name(zone_id.as_str())));
        let mutation_replication = mutation_replication_from_directory(&active, wal_directory)?;
        targets.push(ReplicationTarget {
            zone_id,
            active,
            standby,
            active_detector,
            standby_detector,
            mutation_replication,
            installed_checksum: None,
            failover: target_failover,
        });
    }
    Ok(targets)
}

fn parse_zone_ids(value: &str) -> Result<Vec<ZoneId>, String> {
    let zone_ids = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ZoneId::new)
        .collect::<Vec<_>>();
    if zone_ids.is_empty() {
        return Err("MIR2_ZONE_REPLICA_ZONE_IDS must contain at least one Zone id".to_string());
    }
    let unique = zone_ids
        .iter()
        .map(ZoneId::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != zone_ids.len() {
        return Err("MIR2_ZONE_REPLICA_ZONE_IDS contains a duplicate Zone id".to_string());
    }
    Ok(zone_ids)
}

fn regional_zone_ids(
    active_map_count: usize,
    hot_map: &str,
    hot_map_lines: usize,
) -> Result<Vec<ZoneId>, String> {
    let active_maps = crystal_respawn_manifest_ref()
        .maps
        .iter()
        .filter(|map| !map.respawns.is_empty())
        .take(active_map_count)
        .map(|map| map.map_file_name.as_str())
        .collect::<Vec<_>>();
    if active_maps.len() != active_map_count {
        return Err(format!(
            "Crystal manifest supplied {} populated maps but {} were requested",
            active_maps.len(),
            active_map_count
        ));
    }
    if !active_maps.contains(&hot_map) {
        return Err(format!(
            "hot map {hot_map} is not present in the first {active_map_count} populated maps"
        ));
    }
    let mut zone_ids = Vec::with_capacity(active_map_count + hot_map_lines.saturating_sub(1));
    for map_file_name in active_maps {
        if map_file_name == hot_map {
            for line in 1..=hot_map_lines {
                zone_ids.push(ZoneId::new(format!("map:{hot_map}:line:{line}")));
            }
        } else {
            zone_ids.push(ZoneId::new(format!("map:{map_file_name}")));
        }
    }
    Ok(zone_ids)
}

fn failover_config_from_env() -> Result<Option<FailoverConfig>, String> {
    let database_url = non_empty_env("MIR2_ZONE_FAILOVER_DATABASE_URL");
    let active_owner = non_empty_env("MIR2_ZONE_FAILOVER_ACTIVE_OWNER_ID");
    let standby_owner = non_empty_env("MIR2_ZONE_FAILOVER_STANDBY_OWNER_ID");
    if database_url.is_none() && active_owner.is_none() && standby_owner.is_none() {
        return Ok(None);
    }
    let database_url = database_url.ok_or_else(|| {
        "MIR2_ZONE_FAILOVER_DATABASE_URL is required when automatic failover is enabled".to_string()
    })?;
    let active_owner = active_owner.ok_or_else(|| {
        "MIR2_ZONE_FAILOVER_ACTIVE_OWNER_ID is required when automatic failover is enabled"
            .to_string()
    })?;
    let standby_owner = standby_owner.ok_or_else(|| {
        "MIR2_ZONE_FAILOVER_STANDBY_OWNER_ID is required when automatic failover is enabled"
            .to_string()
    })?;
    if active_owner == standby_owner {
        return Err("automatic failover active and standby owners must differ".to_string());
    }
    Ok(Some(FailoverConfig {
        database_url,
        active_owner,
        standby_owner,
        lease_ttl_ms: positive_u64_env(
            "MIR2_ZONE_FAILOVER_LEASE_TTL_MS",
            positive_u64_env("MIR2_GATEWAY_ZONE_LEASE_TTL_MS", 30_000, 1_000, 300_000),
            1_000,
            300_000,
        ),
        failure_threshold: optional_positive_usize_env(
            "MIR2_ZONE_FAILOVER_FAILURE_THRESHOLD",
            1,
            10,
        )?
        .unwrap_or(2),
        maximum_lag_ms: positive_u64_env(
            "MIR2_ZONE_PROMOTION_MAX_LAG_MS",
            DEFAULT_ZONE_PROMOTION_MAX_LAG_MS,
            1,
            5_000,
        ),
    }))
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn hex_zone_directory_name(zone_id: &str) -> String {
    zone_id
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn mutation_replication_from_directory(
    active: &TcpZoneOwnerRpcTransport,
    directory: Option<PathBuf>,
) -> Result<Option<MutationReplicationState>, String> {
    let Some(directory) = directory else {
        return Ok(None);
    };
    let head = active.replication_head()?;
    let mut wal = ZoneMutationWal::open(
        directory.join("mutation-batches-v5.jsonl"),
        &head.zone_id,
        &head.build_id,
    )?;
    let snapshot_store = ZoneBaseSnapshotStore::new(
        directory.join("base-snapshot-v5.json"),
        &head.zone_id,
        &head.build_id,
    )?;
    let stored_snapshot = snapshot_store.load()?;
    let needs_current_base = wal.ack().next_sequence < head.oldest_available_sequence
        || stored_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.base_sequence < head.oldest_available_sequence);
    let (last_snapshot_sequence, last_snapshot_session_count) = if needs_current_base {
        let snapshot = active.export_base_snapshot()?;
        snapshot_store.persist(&snapshot)?;
        wal.compact_to_base(&snapshot)?;
        eprintln!(
            "zone-replicator persisted base snapshot {} at cursor {} (compressed={} bytes, raw={} bytes, applyReady={})",
            snapshot.snapshot_id,
            snapshot.base_sequence,
            snapshot.payload.len(),
            snapshot.uncompressed_bytes,
            snapshot.apply_ready
        );
        eprintln!(
            "zone-replicator bootstrapped compacted active history from v5 base {} at cursor {}",
            snapshot.snapshot_id, snapshot.base_sequence
        );
        (
            Some(snapshot.base_sequence),
            Some(active.health()?.session_count),
        )
    } else {
        validate_wal_prefix(active, &wal.ack(), head.next_sequence, &head.latest_digest)?;
        let sequence = stored_snapshot
            .map(|snapshot| {
                validate_snapshot_prefix(
                    active,
                    snapshot.base_sequence,
                    &snapshot.latest_digest,
                    head.next_sequence,
                    &head.latest_digest,
                )?;
                Ok::<u64, String>(snapshot.base_sequence)
            })
            .transpose()?;
        // A persisted base does not carry a cheap population summary. Force
        // one fresh base on the first live synchronization so a process
        // restart cannot retain a stale empty-session base indefinitely.
        (sequence, None)
    };
    eprintln!(
        "zone-replicator mutation WAL ready at {} (cursor={}, base={})",
        wal.path().display(),
        wal.ack().next_sequence,
        last_snapshot_sequence
            .map(|sequence| sequence.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    Ok(Some(MutationReplicationState {
        wal,
        snapshot_store,
        last_snapshot_sequence,
        last_snapshot_session_count,
        snapshot_interval_entries: positive_u64_env(
            "MIR2_ZONE_REPLICA_BASE_SNAPSHOT_INTERVAL_ENTRIES",
            DEFAULT_BASE_SNAPSHOT_INTERVAL_ENTRIES,
            1,
            1_000_000,
        ),
    }))
}

fn sync_mutation_wal(
    active: &TcpZoneOwnerRpcTransport,
    wal: &mut ZoneMutationWal,
) -> Result<ZoneMutationWalAck, String> {
    let head = active.replication_head()?;
    let mut ack = wal.ack();
    validate_wal_prefix(active, &ack, head.next_sequence, &head.latest_digest)?;
    while ack.next_sequence < head.next_sequence {
        let remaining = head.next_sequence.saturating_sub(ack.next_sequence);
        let max_entries = usize::try_from(remaining)
            .unwrap_or(DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES)
            .min(DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES);
        let batch = active.export_mutation_batch(
            ack.next_sequence,
            max_entries,
            DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES,
        )?;
        if batch.entries.is_empty() {
            return Err(format!(
                "active Zone {} reported cursor {} but returned no mutation at durable cursor {}",
                head.zone_id, head.next_sequence, ack.next_sequence
            ));
        }
        ack = wal.append_batch(&batch)?;
    }
    if ack.latest_digest != head.latest_digest {
        return Err(format!(
            "durable mutation WAL digest {} does not match active Head {} at cursor {}",
            ack.latest_digest, head.latest_digest, head.next_sequence
        ));
    }
    Ok(ack)
}

fn maybe_persist_base_snapshot(
    active: &TcpZoneOwnerRpcTransport,
    replication: &mut MutationReplicationState,
) -> Result<(), String> {
    let durable_sequence = replication.wal.ack().next_sequence;
    let active_session_count = active.health()?.session_count;
    let population_changed = replication.last_snapshot_session_count != Some(active_session_count);
    if !population_changed
        && replication.last_snapshot_sequence.is_some_and(|last| {
            durable_sequence.saturating_sub(last) < replication.snapshot_interval_entries
        })
    {
        return Ok(());
    }
    let snapshot = active.export_base_snapshot()?;
    let durable_ack = replication.wal.ack();
    if snapshot.base_sequence < durable_ack.next_sequence {
        return Err(format!(
            "base snapshot cursor {} is behind durable mutation WAL {}",
            snapshot.base_sequence, durable_ack.next_sequence
        ));
    }
    let active_head = active.replication_head()?;
    validate_snapshot_prefix(
        active,
        snapshot.base_sequence,
        &snapshot.latest_digest,
        active_head.next_sequence,
        &active_head.latest_digest,
    )?;
    replication.snapshot_store.persist(&snapshot)?;
    replication.wal.compact_to_base(&snapshot)?;
    replication.last_snapshot_sequence = Some(snapshot.base_sequence);
    replication.last_snapshot_session_count = Some(active_session_count);
    eprintln!(
        "zone-replicator persisted base snapshot {} at cursor {} (compressed={} bytes, raw={} bytes, applyReady={})",
        snapshot.snapshot_id,
        snapshot.base_sequence,
        snapshot.payload.len(),
        snapshot.uncompressed_bytes,
        snapshot.apply_ready
    );
    Ok(())
}

fn sync_incremental_standby(
    active: &TcpZoneOwnerRpcTransport,
    standby: &TcpZoneOwnerRpcTransport,
    replication: &mut MutationReplicationState,
) -> Result<Option<usize>, String> {
    let Some(snapshot) = replication.snapshot_store.load()? else {
        return Ok(None);
    };
    if !snapshot.apply_ready {
        return Ok(None);
    }
    let durable = replication.wal.ack();
    if snapshot.base_sequence > durable.next_sequence {
        return Err(format!(
            "persisted base cursor {} is ahead of durable WAL {}",
            snapshot.base_sequence, durable.next_sequence
        ));
    }
    let mut standby_head = standby.replication_head()?;
    if standby_head.build_id != snapshot.build_id {
        return Err(format!(
            "standby build {} does not match base build {}",
            standby_head.build_id, snapshot.build_id
        ));
    }
    let must_install_base = standby_head.next_sequence < snapshot.base_sequence
        || (standby_head.next_sequence == snapshot.base_sequence
            && (standby_head.latest_digest != snapshot.latest_digest
                || standby_head.base_snapshot_id.as_deref()
                    != Some(snapshot.snapshot_id.as_str())));
    if must_install_base {
        standby.install_base_snapshot(&snapshot)?;
        standby_head = standby.replication_head()?;
        eprintln!(
            "zone-replicator installed v5 base {} at cursor {}",
            snapshot.snapshot_id, snapshot.base_sequence
        );
    }
    let prefix = ZoneMutationWalAck {
        zone_id: standby_head.zone_id.clone(),
        build_id: standby_head.build_id.clone(),
        next_sequence: standby_head.next_sequence,
        latest_digest: standby_head.latest_digest.clone(),
        durable: true,
    };
    validate_wal_prefix(
        active,
        &prefix,
        durable.next_sequence,
        &durable.latest_digest,
    )
    .map_err(|error| format!("standby is not a prefix of durable WAL: {error}"))?;

    while standby_head.next_sequence < durable.next_sequence {
        let remaining = durable
            .next_sequence
            .saturating_sub(standby_head.next_sequence);
        let max_entries = usize::try_from(remaining)
            .unwrap_or(DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES)
            .min(DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES);
        let batch = active.export_mutation_batch(
            standby_head.next_sequence,
            max_entries,
            DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES,
        )?;
        if batch.entries.is_empty() || batch.next_sequence > durable.next_sequence {
            return Err(format!(
                "active returned invalid incremental batch {}..{} for durable cursor {}",
                batch.first_sequence, batch.next_sequence, durable.next_sequence
            ));
        }
        standby.apply_mutation_batch(&batch)?;
        standby_head = standby.replication_head()?;
    }
    if standby_head.latest_digest != durable.latest_digest {
        return Err(format!(
            "standby digest {} does not match durable WAL {} at cursor {}",
            standby_head.latest_digest, durable.latest_digest, durable.next_sequence
        ));
    }
    let compacted_entries = active.compact_mutation_journal(&snapshot)?;
    if compacted_entries > 0 {
        eprintln!(
            "zone-replicator compacted {compacted_entries} active journal entries through durable base {} at cursor {}",
            snapshot.snapshot_id, snapshot.base_sequence
        );
    }
    Ok(Some(active.health()?.session_count))
}

fn validate_snapshot_prefix(
    active: &TcpZoneOwnerRpcTransport,
    snapshot_sequence: u64,
    snapshot_digest: &str,
    active_next_sequence: u64,
    active_latest_digest: &str,
) -> Result<(), String> {
    let ack = ZoneMutationWalAck {
        zone_id: String::new(),
        build_id: String::new(),
        next_sequence: snapshot_sequence,
        latest_digest: snapshot_digest.to_string(),
        durable: true,
    };
    validate_wal_prefix(active, &ack, active_next_sequence, active_latest_digest)
        .map_err(|error| format!("base snapshot is not a prefix of active Head: {error}"))
}

fn validate_wal_prefix(
    active: &TcpZoneOwnerRpcTransport,
    ack: &ZoneMutationWalAck,
    active_next_sequence: u64,
    active_latest_digest: &str,
) -> Result<(), String> {
    if ack.next_sequence > active_next_sequence {
        return Err(format!(
            "durable mutation WAL cursor {} is ahead of active Head {}",
            ack.next_sequence, active_next_sequence
        ));
    }
    if ack.next_sequence == active_next_sequence {
        if ack.latest_digest != active_latest_digest {
            return Err(format!(
                "durable mutation WAL digest {} conflicts with active Head {} at cursor {}",
                ack.latest_digest, active_latest_digest, active_next_sequence
            ));
        }
        return Ok(());
    }
    let continuity = active.export_mutation_batch(
        ack.next_sequence,
        1,
        DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES,
    )?;
    if continuity.previous_digest != ack.latest_digest {
        return Err(format!(
            "durable mutation WAL digest {} is not a prefix of active Head {}",
            ack.latest_digest, active_latest_digest
        ));
    }
    Ok(())
}

fn replication_heads_match(left: &ZoneReplicationHead, right: &ZoneReplicationHead) -> bool {
    left.zone_id == right.zone_id
        && left.build_id == right.build_id
        && left.next_sequence == right.next_sequence
        && left.latest_digest == right.latest_digest
}

fn duration_from_env(name: &str, default_ms: u64, min_ms: u64, max_ms: u64) -> Duration {
    Duration::from_millis(positive_u64_env(name, default_ms, min_ms, max_ms))
}

fn positive_u64_env(name: &str, default: u64, min: u64, max: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

fn optional_positive_usize_env(
    name: &str,
    minimum: usize,
    maximum: usize,
) -> Result<Option<usize>, String> {
    let Some(value) = env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("{name} must be an integer: {error}"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!(
            "{name} must be between {minimum} and {maximum}, got {parsed}"
        ));
    }
    Ok(Some(parsed))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replication_cadence_backs_off_when_no_sessions_are_active() {
        let cadence = ReplicationCadence::new(Duration::from_millis(100), Duration::from_secs(5));

        assert_eq!(
            cadence.delay_after_checkpoint(2),
            Duration::from_millis(100)
        );
        assert_eq!(cadence.delay_after_checkpoint(0), Duration::from_secs(5));
    }

    #[test]
    fn idle_replication_cannot_poll_faster_than_active_replication() {
        let cadence =
            ReplicationCadence::new(Duration::from_millis(500), Duration::from_millis(100));

        assert_eq!(
            cadence.delay_after_checkpoint(0),
            Duration::from_millis(500)
        );
    }

    #[test]
    fn explicit_zone_ids_reject_duplicates() {
        let error = parse_zone_ids("map:0:line:1,map:0:line:1")
            .expect_err("duplicate Zone ids must fail closed");

        assert!(error.contains("duplicate"));
    }

    #[test]
    fn regional_catalog_expands_hot_map_into_exact_lines() {
        let zone_ids = regional_zone_ids(120, "0", 10).expect("regional catalog should resolve");

        assert_eq!(zone_ids.len(), 129);
        assert_eq!(zone_ids[0].as_str(), "map:0:line:1");
        assert_eq!(zone_ids[9].as_str(), "map:0:line:10");
        assert!(!zone_ids.iter().any(|zone| zone.as_str() == "map:0"));
        assert_eq!(
            zone_ids
                .iter()
                .map(ZoneId::as_str)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            129
        );
    }

    #[test]
    fn external_owner_handoff_swaps_roles_and_resets_replication_state() {
        let zone_id = ZoneId::new("map:external-promotion");
        let transport = |address: &str, session: &str| {
            TcpZoneOwnerRpcTransport::with_options(
                address,
                zone_id.clone(),
                session,
                None,
                ZoneRpcLimits::default(),
            )
        };
        let mut target = ReplicationTarget {
            zone_id: zone_id.clone(),
            active: transport("127.0.0.1:17020", "active"),
            standby: transport("127.0.0.1:17021", "standby"),
            active_detector: transport("127.0.0.1:17020", "active-detector"),
            standby_detector: transport("127.0.0.1:17021", "standby-detector"),
            mutation_replication: None,
            installed_checksum: Some("old-checkpoint".to_string()),
            failover: Some(TargetFailoverState {
                config: FailoverConfig {
                    database_url: "postgres://unused".to_string(),
                    active_owner: "owner-a".to_string(),
                    standby_owner: "owner-b".to_string(),
                    lease_ttl_ms: 30_000,
                    failure_threshold: 2,
                    maximum_lag_ms: 500,
                },
                active_owner: "owner-a".to_string(),
                standby_owner: "owner-b".to_string(),
                consecutive_active_failures: 2,
                last_synchronized: None,
                last_reported_synchronization_at_ms: Some(1),
                last_reported_session_count: Some(10),
                last_degraded_report_at_ms: Some(1),
                last_owner_observation_at_ms: Some(1),
            }),
        };

        assert!(target
            .apply_observed_owner("owner-b", 2)
            .expect("configured peer owner should reconcile"));
        let state = target.failover.as_ref().unwrap();
        assert_eq!(state.active_owner, "owner-b");
        assert_eq!(state.standby_owner, "owner-a");
        assert_eq!(state.consecutive_active_failures, 0);
        assert!(state.last_reported_synchronization_at_ms.is_none());
        assert!(state.last_reported_session_count.is_none());
        assert!(state.last_degraded_report_at_ms.is_none());
        assert!(state.last_owner_observation_at_ms.is_none());
        assert!(target.installed_checksum.is_none());
        assert!(!target
            .apply_observed_owner("owner-b", 2)
            .expect("already-current owner should be a no-op"));
        assert!(target
            .apply_observed_owner("owner-c", 3)
            .expect_err("unconfigured owner must fail closed")
            .contains("unconfigured owner"));
        assert!(target
            .apply_observed_owner("owner-a", 4)
            .expect("reverse external handoff should reconcile"));
        assert_eq!(target.failover.as_ref().unwrap().active_owner, "owner-a");
    }
}

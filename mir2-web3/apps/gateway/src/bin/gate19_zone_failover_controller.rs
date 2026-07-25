use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_gateway::zone_lease::PostgresZoneOwnerLeaseAuthority;
use mir2_gateway::{
    TcpZoneOwnerRpcTransport, ZoneBaseSnapshot, ZoneId, ZoneMutationBatch, ZoneReplicationHead,
    ZoneRpcLimits, DEFAULT_ZONE_PROMOTION_MAX_LAG_MS, DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES,
    DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES,
};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "docs/generated/regional/gate19-zone-failover.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate19ZoneFailoverEvidence {
    schema_version: u32,
    generated_at_ms: u64,
    zone_id: String,
    active_endpoint: String,
    standby_endpoint: String,
    active_owner: String,
    standby_owner: String,
    initial_base_snapshot_id: String,
    synchronized_cursor: u64,
    synchronized_digest: String,
    synchronized_at_ms: u64,
    active_failure_detected_at_ms: u64,
    promoted_at_ms: u64,
    failover_rto_ms: f64,
    old_generation: u64,
    new_generation: u64,
    readiness_id: String,
    readiness_lag_ms: u64,
    promoted_cursor: u64,
    promoted_digest: String,
    post_promotion_probe_succeeded: bool,
    assertions: std::collections::BTreeMap<String, bool>,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let active_endpoint = required_env("MIR2_ZONE_ACTIVE_ADDR")?;
    let standby_endpoint = required_env("MIR2_ZONE_STANDBY_ADDR")?;
    if active_endpoint == standby_endpoint {
        return Err("active and standby endpoints must differ".into());
    }
    let zone_id = ZoneId::new(
        env::var("MIR2_ZONE_REPLICA_ZONE_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "map:0".to_string()),
    );
    let active_owner = required_env("MIR2_ZONE_ACTIVE_OWNER_ID")?;
    let standby_owner = required_env("MIR2_ZONE_STANDBY_OWNER_ID")?;
    if active_owner == standby_owner {
        return Err("active and standby owner IDs must differ".into());
    }
    let database_url = required_env("MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL")?;
    let output = PathBuf::from(
        env::var("MIR2_GATE19_ZONE_FAILOVER_OUT").unwrap_or_else(|_| DEFAULT_OUTPUT.to_string()),
    );
    let poll_interval =
        Duration::from_millis(env_u64("MIR2_GATE19_ZONE_FAILOVER_POLL_MS", 20, 5, 500));
    let failure_threshold = env_usize("MIR2_GATE19_ZONE_FAILURE_THRESHOLD", 2, 1, 10);
    let maximum_rto_ms = env_u64("MIR2_GATE19_MAX_ZONE_RTO_MS", 5_000, 1, 60_000);
    let maximum_lag_ms = env_u64(
        "MIR2_ZONE_PROMOTION_MAX_LAG_MS",
        DEFAULT_ZONE_PROMOTION_MAX_LAG_MS,
        1,
        5_000,
    );
    let lease_ttl_ms = env_u64("MIR2_GATEWAY_ZONE_LEASE_TTL_MS", 30_000, 1_000, 300_000);
    let token = env::var("MIR2_ZONE_HOST_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let mut limits = ZoneRpcLimits::from_env();
    limits.io_timeout = Duration::from_millis(env_u64(
        "MIR2_GATE19_ZONE_SYNC_RPC_TIMEOUT_MS",
        1_000,
        100,
        30_000,
    ));
    let active = TcpZoneOwnerRpcTransport::with_options(
        active_endpoint.clone(),
        zone_id.clone(),
        "gate19-controller-active",
        token.clone(),
        limits.clone(),
    )
    .with_connection_reuse();
    let mut detector_limits = limits.clone();
    detector_limits.io_timeout =
        Duration::from_millis(env_u64("MIR2_GATE19_ZONE_RPC_TIMEOUT_MS", 100, 50, 5_000));
    let active_detector = TcpZoneOwnerRpcTransport::with_options(
        active_endpoint.clone(),
        zone_id.clone(),
        "gate19-controller-detector",
        env::var("MIR2_ZONE_HOST_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty()),
        detector_limits,
    )
    .with_connection_reuse();
    let standby = TcpZoneOwnerRpcTransport::with_options(
        standby_endpoint.clone(),
        zone_id.clone(),
        "gate19-controller-standby",
        token,
        limits,
    )
    .with_connection_reuse();
    let authority =
        PostgresZoneOwnerLeaseAuthority::new(database_url, active_owner.clone(), lease_ttl_ms);
    let active_lease = authority.acquire_at(&zone_id, now_ms())?;
    if active_lease.owner_id() != active_owner {
        return Err(format!(
            "Zone {} is owned by {}, expected {}",
            zone_id,
            active_lease.owner_id(),
            active_owner
        )
        .into());
    }

    eprintln!(
        "Gate 19 controller acquired {} generation {} for {}",
        active_lease.owner_id(),
        active_lease.fencing_token(),
        zone_id
    );
    eprintln!("Gate 19 controller exporting initial base snapshot");
    let base = active.export_base_snapshot()?;
    eprintln!(
        "Gate 19 controller installing base {} at cursor {}",
        base.snapshot_id, base.base_sequence
    );
    standby.install_base_snapshot(&base)?;
    eprintln!("Gate 19 controller initial base installed");
    let (last_synchronized, synchronized_at_ms) = wait_for_failure(
        &active,
        &active_detector,
        &standby,
        poll_interval,
        failure_threshold,
        &base,
    )?;
    let active_failure_detected_at_ms = now_ms();
    let failover_started = Instant::now();
    let readiness = standby.assess_promotion_readiness(
        last_synchronized.clone(),
        synchronized_at_ms,
        maximum_lag_ms,
    )?;
    if !readiness.ready {
        return Err(format!(
            "standby rejected failover readiness: {}",
            readiness.reason.as_deref().unwrap_or("unknown")
        )
        .into());
    }
    let readiness_id = readiness
        .readiness_id
        .clone()
        .ok_or("ready standby did not return a readiness ID")?;
    let promoted_lease = authority.handoff_at(&active_lease, standby_owner.clone(), now_ms())?;
    let receipt = standby.promote_replica(readiness_id.clone(), &promoted_lease)?;
    let post_promotion_probe_succeeded = standby.health().is_ok_and(|health| !health.draining)
        && standby.replication_head().is_ok_and(|head| {
            head.next_sequence >= receipt.head.next_sequence
                && head.latest_digest == receipt.head.latest_digest
        });
    let failover_rto_ms = failover_started.elapsed().as_secs_f64() * 1_000.0;
    let assertions = std::collections::BTreeMap::from([
        (
            "standbyMatchedLastObservedCursorAndDigest".to_string(),
            receipt.head.next_sequence == last_synchronized.next_sequence
                && receipt.head.latest_digest == last_synchronized.latest_digest,
        ),
        (
            "promotionAdvancedOwnerGeneration".to_string(),
            promoted_lease.fencing_token() > active_lease.fencing_token(),
        ),
        (
            "zoneFailoverMetRto".to_string(),
            failover_rto_ms <= maximum_rto_ms as f64,
        ),
        (
            "postPromotionProbeSucceeded".to_string(),
            post_promotion_probe_succeeded,
        ),
    ]);
    let success = assertions.values().all(|value| *value);
    let evidence = Gate19ZoneFailoverEvidence {
        schema_version: 1,
        generated_at_ms: now_ms(),
        zone_id: zone_id.as_str().to_string(),
        active_endpoint,
        standby_endpoint,
        active_owner,
        standby_owner,
        initial_base_snapshot_id: base.snapshot_id,
        synchronized_cursor: last_synchronized.next_sequence,
        synchronized_digest: last_synchronized.latest_digest,
        synchronized_at_ms,
        active_failure_detected_at_ms,
        promoted_at_ms: receipt.promoted_at_ms,
        failover_rto_ms,
        old_generation: active_lease.fencing_token(),
        new_generation: promoted_lease.fencing_token(),
        readiness_id,
        readiness_lag_ms: readiness.observed_lag_ms,
        promoted_cursor: receipt.head.next_sequence,
        promoted_digest: receipt.head.latest_digest,
        post_promotion_probe_succeeded,
        assertions,
        success,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&evidence)?)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    if success {
        Ok(())
    } else {
        Err("Gate 19 Zone failover assertions failed".into())
    }
}

fn wait_for_failure(
    active: &TcpZoneOwnerRpcTransport,
    active_detector: &TcpZoneOwnerRpcTransport,
    standby: &TcpZoneOwnerRpcTransport,
    poll_interval: Duration,
    failure_threshold: usize,
    base: &ZoneBaseSnapshot,
) -> Result<(ZoneReplicationHead, u64), String> {
    let mut last_synchronized = standby.replication_head()?;
    if last_synchronized.base_snapshot_id.as_deref() != Some(base.snapshot_id.as_str()) {
        return Err("standby did not retain the installed base snapshot".to_string());
    }
    let mut synchronized_at_ms = now_ms();
    let mut consecutive_failures = 0;
    let mut announced = false;
    let mut unsynchronized_polls = 0_u64;
    loop {
        match active_detector.replication_head() {
            Ok(active_head) => {
                consecutive_failures = 0;
                sync_to_head(active, standby, &active_head)?;
                let confirmed_active = active.replication_head()?;
                let confirmed_standby = standby.replication_head()?;
                if heads_match(&confirmed_active, &confirmed_standby) {
                    last_synchronized = confirmed_standby;
                    synchronized_at_ms = now_ms();
                    if !announced {
                        println!(
                            "GATE19_ZONE_SYNCHRONIZED cursor={} digest={}",
                            last_synchronized.next_sequence, last_synchronized.latest_digest
                        );
                        announced = true;
                    }
                } else {
                    unsynchronized_polls = unsynchronized_polls.saturating_add(1);
                    if unsynchronized_polls == 1 || unsynchronized_polls % 50 == 0 {
                        eprintln!(
                            "Gate 19 controller waiting for exact head: active={}/{} standby={}/{}",
                            confirmed_active.next_sequence,
                            confirmed_active.latest_digest,
                            confirmed_standby.next_sequence,
                            confirmed_standby.latest_digest
                        );
                    }
                }
            }
            Err(error) => {
                consecutive_failures += 1;
                eprintln!(
                    "Gate 19 active health failure {consecutive_failures}/{failure_threshold}: {error}"
                );
                if consecutive_failures >= failure_threshold {
                    return Ok((last_synchronized, synchronized_at_ms));
                }
            }
        }
        thread::sleep(poll_interval);
    }
}

fn sync_to_head(
    active: &TcpZoneOwnerRpcTransport,
    standby: &TcpZoneOwnerRpcTransport,
    target: &ZoneReplicationHead,
) -> Result<(), String> {
    let mut standby_head = standby.replication_head()?;
    if standby_head.next_sequence > target.next_sequence {
        return Err(format!(
            "standby cursor {} is ahead of active {}",
            standby_head.next_sequence, target.next_sequence
        ));
    }
    while standby_head.next_sequence < target.next_sequence {
        let remaining = target
            .next_sequence
            .saturating_sub(standby_head.next_sequence);
        let batch: ZoneMutationBatch = active.export_mutation_batch(
            standby_head.next_sequence,
            usize::try_from(remaining)
                .unwrap_or(DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES)
                .min(DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES),
            DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES,
        )?;
        if batch.entries.is_empty() {
            return Err("active returned an empty mutation batch before target cursor".to_string());
        }
        standby.apply_mutation_batch(&batch)?;
        standby_head = standby.replication_head()?;
    }
    if standby_head.latest_digest != target.latest_digest {
        return Err(format!(
            "standby digest {} does not match active {} at cursor {}",
            standby_head.latest_digest, target.latest_digest, target.next_sequence
        ));
    }
    Ok(())
}

fn heads_match(left: &ZoneReplicationHead, right: &ZoneReplicationHead) -> bool {
    left.zone_id == right.zone_id
        && left.build_id == right.build_id
        && left.mutation_coverage == right.mutation_coverage
        && left.next_sequence == right.next_sequence
        && left.latest_digest == right.latest_digest
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn env_u64(name: &str, default: u64, minimum: u64, maximum: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
        .clamp(minimum, maximum)
}

fn env_usize(name: &str, default: usize, minimum: usize, maximum: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
        .clamp(minimum, maximum)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

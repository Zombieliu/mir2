use std::env;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use mir2_gateway::{
    TcpZoneOwnerRpcTransport, ZoneBaseSnapshotStore, ZoneId, ZoneMutationWal, ZoneMutationWalAck,
    ZoneRpcLimits, DEFAULT_ZONE_REPLICATION_MAX_BATCH_BYTES,
    DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES,
};

const DEFAULT_ACTIVE_INTERVAL_MS: u64 = 250;
const DEFAULT_IDLE_INTERVAL_MS: u64 = 5_000;
const DEFAULT_BASE_SNAPSHOT_INTERVAL_ENTRIES: u64 = 512;

struct MutationReplicationState {
    wal: ZoneMutationWal,
    snapshot_store: ZoneBaseSnapshotStore,
    last_snapshot_sequence: Option<u64>,
    snapshot_interval_entries: u64,
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
    let active_address = required_env("MIR2_ZONE_ACTIVE_ADDR")?;
    let standby_address = required_env("MIR2_ZONE_STANDBY_ADDR")?;
    if active_address == standby_address {
        return Err("active and standby Zone Host addresses must differ".to_string());
    }
    let zone_id = ZoneId::new(
        env::var("MIR2_ZONE_REPLICA_ZONE_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "primary".to_string()),
    );
    let token = env::var("MIR2_ZONE_HOST_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let limits = ZoneRpcLimits::from_env();
    let active = TcpZoneOwnerRpcTransport::with_options(
        active_address,
        zone_id.clone(),
        "zone-replicator-active",
        token.clone(),
        limits.clone(),
    );
    let standby = TcpZoneOwnerRpcTransport::with_options(
        standby_address,
        zone_id,
        "zone-replicator-standby",
        token,
        limits,
    );
    let cadence = ReplicationCadence::from_env();
    let mut mutation_replication = mutation_replication_from_env(&active)?;
    let mut installed_checksum = None::<String>;
    let mut was_idle = None::<bool>;

    loop {
        if let Some(replication) = mutation_replication.as_mut() {
            let before = replication.wal.ack();
            let after = sync_mutation_wal(&active, &mut replication.wal)?;
            if after.next_sequence != before.next_sequence {
                eprintln!(
                    "zone-replicator persisted mutation WAL through cursor {} ({})",
                    after.next_sequence, after.latest_digest
                );
            }
            maybe_persist_base_snapshot(&active, replication)?;
        }
        let checkpoint = active.export_host_checkpoint()?;
        let session_count = checkpoint.session_count;
        if installed_checksum.as_deref() != Some(checkpoint.checksum.as_str()) {
            standby.install_host_checkpoint(&checkpoint)?;
            eprintln!(
                "zone-replicator installed checkpoint {} (entries={}, sessions={})",
                checkpoint.checksum, checkpoint.entry_count, session_count
            );
            installed_checksum = Some(checkpoint.checksum);
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

fn mutation_replication_from_env(
    active: &TcpZoneOwnerRpcTransport,
) -> Result<Option<MutationReplicationState>, String> {
    let Some(directory) = env::var("MIR2_ZONE_REPLICA_WAL_DIR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let head = active.replication_head()?;
    let directory = PathBuf::from(directory);
    let wal = ZoneMutationWal::open(
        directory.join("mutation-batches-v5.jsonl"),
        &head.zone_id,
        &head.build_id,
    )?;
    validate_wal_prefix(active, &wal.ack(), head.next_sequence, &head.latest_digest)?;
    let snapshot_store = ZoneBaseSnapshotStore::new(
        directory.join("base-snapshot-v5.json"),
        &head.zone_id,
        &head.build_id,
    )?;
    let last_snapshot_sequence = snapshot_store
        .load()?
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
        let batch = active.export_mutation_batch(
            ack.next_sequence,
            DEFAULT_ZONE_REPLICATION_MAX_BATCH_ENTRIES,
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
    if durable_sequence == 0 {
        return Ok(());
    }
    if replication.last_snapshot_sequence.is_some_and(|last| {
        durable_sequence.saturating_sub(last) < replication.snapshot_interval_entries
    }) {
        return Ok(());
    }
    let snapshot = active.export_base_snapshot()?;
    let durable_ack = sync_mutation_wal(active, &mut replication.wal)?;
    if snapshot.base_sequence > durable_ack.next_sequence {
        return Err(format!(
            "base snapshot cursor {} is ahead of durable mutation WAL {}",
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
    replication.last_snapshot_sequence = Some(snapshot.base_sequence);
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

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} is required"))
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
}

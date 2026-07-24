use std::env;
use std::thread;
use std::time::Duration;

use mir2_gateway::{TcpZoneOwnerRpcTransport, ZoneId, ZoneRpcLimits};

const DEFAULT_ACTIVE_INTERVAL_MS: u64 = 250;
const DEFAULT_IDLE_INTERVAL_MS: u64 = 5_000;

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
    let mut installed_checksum = None::<String>;
    let mut was_idle = None::<bool>;

    loop {
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

fn duration_from_env(name: &str, default_ms: u64, min_ms: u64, max_ms: u64) -> Duration {
    Duration::from_millis(
        env::var(name)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(default_ms)
            .clamp(min_ms, max_ms),
    )
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

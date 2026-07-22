use std::env;
use std::thread;
use std::time::Duration;

use mir2_gateway::{TcpZoneOwnerRpcTransport, ZoneId, ZoneRpcLimits};

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
    let interval = Duration::from_millis(
        env::var("MIR2_ZONE_REPLICA_INTERVAL_MS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(250)
            .clamp(50, 30_000),
    );
    let mut installed_checksum = None::<String>;

    loop {
        let checkpoint = active.export_host_checkpoint()?;
        if installed_checksum.as_deref() != Some(checkpoint.checksum.as_str()) {
            standby.install_host_checkpoint(&checkpoint)?;
            eprintln!(
                "zone-replicator installed checkpoint {} (entries={}, sessions={})",
                checkpoint.checksum, checkpoint.entry_count, checkpoint.session_count
            );
            installed_checksum = Some(checkpoint.checksum);
        }
        if once {
            return Ok(());
        }
        thread::sleep(interval);
    }
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

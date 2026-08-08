use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    TcpZoneOwnerRpcTransport, ZoneId, ZoneOwnerLease, ZoneRpcLimits,
    DEFAULT_ZONE_PROMOTION_MAX_LAG_MS,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("zone-promoter failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let operation = env::args().nth(1).unwrap_or_else(|| "assess".to_string());
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
    let standby = TcpZoneOwnerRpcTransport::with_options(
        required_env("MIR2_ZONE_STANDBY_ADDR")?,
        zone_id.clone(),
        "zone-promoter-standby",
        token.clone(),
        limits.clone(),
    );

    match operation.as_str() {
        "assess" => {
            let active = TcpZoneOwnerRpcTransport::with_options(
                required_env("MIR2_ZONE_ACTIVE_ADDR")?,
                zone_id,
                "zone-promoter-active",
                token,
                limits,
            );
            let observed_at_ms = unix_now_ms();
            let head = active.replication_head()?;
            let max_lag_ms = env::var("MIR2_ZONE_PROMOTION_MAX_LAG_MS")
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
                .unwrap_or(DEFAULT_ZONE_PROMOTION_MAX_LAG_MS)
                .clamp(1, 5_000);
            let readiness = standby.assess_promotion_readiness(head, observed_at_ms, max_lag_ms)?;
            println!(
                "{}",
                serde_json::to_string(&readiness)
                    .map_err(|error| format!("failed to encode readiness: {error}"))?
            );
            if readiness.ready {
                Ok(())
            } else {
                Err(format!(
                    "standby is not ready: {}",
                    readiness.reason.as_deref().unwrap_or("unknown")
                ))
            }
        }
        "promote" => {
            let readiness_id = required_env("MIR2_ZONE_PROMOTION_READINESS_ID")?;
            let owner_id = required_env("MIR2_ZONE_PROMOTION_OWNER_ID")?;
            let generation = required_env("MIR2_ZONE_PROMOTION_GENERATION")?
                .parse::<u64>()
                .map_err(|error| format!("invalid MIR2_ZONE_PROMOTION_GENERATION: {error}"))?;
            if generation == 0 {
                return Err("MIR2_ZONE_PROMOTION_GENERATION must be positive".to_string());
            }
            let lease = ZoneOwnerLease::new(zone_id, owner_id, generation);
            let receipt = standby.promote_replica(readiness_id, &lease)?;
            println!(
                "{}",
                serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to encode promotion receipt: {error}"))?
            );
            Ok(())
        }
        "quiesce" | "resume" => {
            let active = TcpZoneOwnerRpcTransport::with_options(
                required_env("MIR2_ZONE_ACTIVE_ADDR")?,
                zone_id.clone(),
                "zone-promoter-active-control",
                token,
                limits,
            );
            let owner_id = required_env("MIR2_ZONE_PROMOTION_OWNER_ID")?;
            let generation = required_env("MIR2_ZONE_PROMOTION_GENERATION")?
                .parse::<u64>()
                .map_err(|error| format!("invalid MIR2_ZONE_PROMOTION_GENERATION: {error}"))?;
            let lease = ZoneOwnerLease::new(zone_id, owner_id, generation);
            if operation == "quiesce" {
                let receipt = active.quiesce_for_promotion(&lease)?;
                println!(
                    "{}",
                    serde_json::to_string(&receipt)
                        .map_err(|error| format!("failed to encode quiesce receipt: {error}"))?
                );
            } else {
                active.resume_after_quiesce(&lease)?;
                println!("{{\"resumed\":true}}");
            }
            Ok(())
        }
        other => Err(format!(
            "unsupported operation {other:?}; expected quiesce, assess, promote, or resume"
        )),
    }
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

//! Explicitly invoked internal promotion worker. No background work starts by
//! default. Database credentials belong in the same protected source-PG config
//! as the gateway, never in command arguments, output or browser requests.
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_simulation::promotion::PromotionStore;
use serde_json::json;

fn usage() {
    println!("promotion_worker --once [--limit 1..100] | --poll-ms 1000..60000 [--limit 1..100]");
    println!("Requires MIR2_ACCOUNT_STORE_DATABASE_URL and MIR2_PROMOTION_ENVIRONMENT (development/test/production). No payment, notification, or alert is sent.");
}

fn run() -> Result<(), &'static str> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args == ["--help"] {
        usage();
        return Ok(());
    }
    let mut once = false;
    let mut poll = None;
    let mut limit = 50u32;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--once" if !once && poll.is_none() => once = true,
            "--poll-ms" if !once && poll.is_none() => {
                index += 1;
                let interval = args
                    .get(index)
                    .ok_or("promotionWorkerArgumentsInvalid")?
                    .parse::<u64>()
                    .map_err(|_| "promotionWorkerArgumentsInvalid")?;
                if !(1000..=60000).contains(&interval) {
                    return Err("promotionWorkerArgumentsInvalid");
                }
                poll = Some(interval);
            }
            "--limit" => {
                index += 1;
                limit = args
                    .get(index)
                    .ok_or("promotionWorkerArgumentsInvalid")?
                    .parse()
                    .map_err(|_| "promotionWorkerArgumentsInvalid")?;
                if !(1..=100).contains(&limit) {
                    return Err("promotionWorkerArgumentsInvalid");
                }
            }
            _ => return Err("promotionWorkerArgumentsInvalid"),
        }
        index += 1;
    }
    if !once && poll.is_none() {
        return Err("promotionWorkerExplicitModeRequired");
    }
    let database = std::env::var("MIR2_ACCOUNT_STORE_DATABASE_URL")
        .map_err(|_| "promotionWorkerConfigurationRequired")?;
    let environment = std::env::var("MIR2_PROMOTION_ENVIRONMENT")
        .map_err(|_| "promotionWorkerConfigurationRequired")?;
    let store = PromotionStore::new(database.clone(), &environment)
        .map_err(|_| "promotionWorkerConfigurationInvalid")?;
    loop {
        let now: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "promotionWorkerClockInvalid")?
            .as_millis()
            .try_into()
            .map_err(|_| "promotionWorkerClockInvalid")?;
        let game_sources = mir2_simulation::promotion::game_events::drain_committed_game_events(
            &database,
            &environment,
            limit,
            now,
        )
        .map_err(|_| "promotionWorkerGameSourcesFailed")?;
        let payment_sources = store
            .reconcile_live_monthly_revenue(now)
            .map_err(|_| "promotionWorkerPaymentSourcesFailed")?;
        let recovery = store
            .retry_pending_reversals(limit, now)
            .map_err(|_| "promotionWorkerRecoveryFailed")?;
        let work = store
            .process_pending_work(limit, now)
            .map_err(|_| "promotionWorkerProcessingFailed")?;
        let incomplete = work["failed"].as_u64().unwrap_or(0) > 0
            || game_sources["failures"]
                .as_array()
                .is_some_and(|v| !v.is_empty())
            || recovery["failures"]
                .as_array()
                .is_some_and(|v| !v.is_empty());
        println!(
            "{}",
            json!({"environment":environment,"dataMode":if environment=="production"{"real"}else{"sandbox"},"gameSources":game_sources,"paymentSources":payment_sources,"reversalRecovery":recovery,"work":work,"externalDispatchImplemented":false})
        );
        if once {
            if incomplete {
                return Err("promotionWorkerIncomplete");
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(
            poll.ok_or("promotionWorkerExplicitModeRequired")?,
        ));
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

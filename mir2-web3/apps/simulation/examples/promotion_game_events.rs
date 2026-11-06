//! Native opt-in committed-game-source pump. Never sends funds or starts Docker.
use mir2_simulation::promotion::game_events::drain_committed_game_events;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn run() -> Result<(), &'static str> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args == ["--help"] {
        println!("promotion_game_events --once | --poll-ms 1000..60000 [--limit 1..100]");
        println!("Requires MIR2_ACCOUNT_STORE_DATABASE_URL and MIR2_PROMOTION_ENVIRONMENT. Reads committed sources only.");
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
                let value = args
                    .get(index)
                    .and_then(|v| v.parse::<u64>().ok())
                    .filter(|v| (1000..=60000).contains(v))
                    .ok_or("promotionGameWorkerArgumentsInvalid")?;
                poll = Some(value);
            }
            "--limit" => {
                index += 1;
                limit = args
                    .get(index)
                    .and_then(|v| v.parse().ok())
                    .filter(|v| (1..=100).contains(v))
                    .ok_or("promotionGameWorkerArgumentsInvalid")?;
            }
            _ => return Err("promotionGameWorkerArgumentsInvalid"),
        }
        index += 1;
    }
    if !once && poll.is_none() {
        return Err("promotionGameWorkerArgumentsInvalid");
    }
    let database = std::env::var("MIR2_ACCOUNT_STORE_DATABASE_URL")
        .map_err(|_| "promotionGameWorkerConfigurationRequired")?;
    let environment = std::env::var("MIR2_PROMOTION_ENVIRONMENT")
        .map_err(|_| "promotionGameWorkerConfigurationRequired")?;
    loop {
        let now: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "promotionGameWorkerClockInvalid")?
            .as_millis()
            .try_into()
            .map_err(|_| "promotionGameWorkerClockInvalid")?;
        let result = drain_committed_game_events(&database, &environment, limit, now)
            .map_err(|_| "promotionGameWorkerProcessingFailed")?;
        println!("{result}");
        if once {
            if result["failures"].as_array().is_some_and(|v| !v.is_empty()) {
                return Err("promotionGameWorkerIncomplete");
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(
            poll.ok_or("promotionGameWorkerArgumentsInvalid")?,
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

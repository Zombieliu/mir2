use std::env;
use std::fs;
use std::path::PathBuf;

use mir2_simulation::{gate5_demo_scenario, run_zone_replay_scenario, ZoneReplayScenario};

fn main() {
    if let Err(error) = run() {
        eprintln!("zone-replay failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(mode) = args.next() else {
        return Err(usage());
    };

    let (scenario, checkpoint_path) = match mode.as_str() {
        "demo" => {
            let tick_count = args
                .next()
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|error| format!("invalid tick count {value}: {error}"))
                })
                .transpose()?
                .unwrap_or(10_000);
            let checkpoint_path = args.next().map(PathBuf::from);
            (gate5_demo_scenario(tick_count), checkpoint_path)
        }
        "run" => {
            let scenario_path = args.next().ok_or_else(usage).map(PathBuf::from)?;
            let bytes = fs::read(&scenario_path).map_err(|error| {
                format!(
                    "failed to read scenario {}: {error}",
                    scenario_path.display()
                )
            })?;
            let scenario =
                serde_json::from_slice::<ZoneReplayScenario>(&bytes).map_err(|error| {
                    format!(
                        "failed to decode scenario {}: {error}",
                        scenario_path.display()
                    )
                })?;
            let checkpoint_path = args.next().map(PathBuf::from);
            (scenario, checkpoint_path)
        }
        _ => return Err(usage()),
    };

    if args.next().is_some() {
        return Err(usage());
    }

    let (engine, report) = run_zone_replay_scenario(scenario)?;
    if let Some(path) = checkpoint_path {
        let checkpoint = engine.checkpoint_bytes()?;
        fs::write(&path, checkpoint)
            .map_err(|error| format!("failed to write checkpoint {}: {error}", path.display()))?;
    }
    let output = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to encode replay report: {error}"))?;
    println!("{output}");
    Ok(())
}

fn usage() -> String {
    "usage: zone_replay demo [tick_count] [checkpoint.json] | zone_replay run <scenario.json> [checkpoint.json]".to_string()
}

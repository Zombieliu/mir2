use mir2_simulation::{apply_migrations, MIGRATIONS};
use postgres::{Client, NoTls};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationAcceptanceEvidence {
    schema_version: u32,
    generated_at_ms: u64,
    process_id: u32,
    concurrent_workers: usize,
    successful_workers: usize,
    expected_migration_count: usize,
    applied_migration_count: i64,
    required_relations_present: bool,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("MIR2_ECONOMY_DATABASE_URL")
        .map_err(|_| "MIR2_ECONOMY_DATABASE_URL is required")?;
    let output = PathBuf::from(
        env::var("MIR2_GATE18_MIGRATION_OUT")
            .unwrap_or_else(|_| "docs/generated/regional/gate18-migrations.json".to_string()),
    );
    let workers = env::var("MIR2_GATE18_MIGRATION_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(2, 64);
    let barrier = Arc::new(Barrier::new(workers));
    let handles = (0..workers)
        .map(|_| {
            let database_url = database_url.clone();
            let barrier = barrier.clone();
            thread::spawn(move || -> Result<(), String> {
                let mut client = Client::connect(&database_url, NoTls)
                    .map_err(|error| format!("migration worker connect failed: {error}"))?;
                barrier.wait();
                apply_migrations(&mut client)
            })
        })
        .collect::<Vec<_>>();

    let mut successful_workers = 0;
    let mut failures = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok(())) => successful_workers += 1,
            Ok(Err(error)) => failures.push(error),
            Err(_) => failures.push("migration worker panicked".to_string()),
        }
    }

    let mut verifier = Client::connect(&database_url, NoTls)?;
    let applied_migration_count: i64 = verifier
        .query_one("SELECT COUNT(*)::bigint FROM schema_migrations", &[])?
        .get(0);
    let required_relations_present: bool = verifier
        .query_one(
            "SELECT to_regclass('public.accounts') IS NOT NULL
                    AND to_regclass('public.characters') IS NOT NULL
                    AND to_regclass('public.game_economy_balances') IS NOT NULL
                    AND to_regclass('public.game_economy_trade_projections') IS NOT NULL
                    AND to_regclass('public.game_economy_ground_drop_projections') IS NOT NULL
                    AND to_regclass('public.zone_owner_leases') IS NOT NULL",
            &[],
        )?
        .get(0);
    let success = failures.is_empty()
        && successful_workers == workers
        && applied_migration_count == MIGRATIONS.len() as i64
        && required_relations_present;
    let evidence = MigrationAcceptanceEvidence {
        schema_version: 1,
        generated_at_ms: now_ms(),
        process_id: std::process::id(),
        concurrent_workers: workers,
        successful_workers,
        expected_migration_count: MIGRATIONS.len(),
        applied_migration_count,
        required_relations_present,
        success,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&evidence)?)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    if !success {
        return Err(format!("concurrent migration acceptance failed: {failures:?}").into());
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

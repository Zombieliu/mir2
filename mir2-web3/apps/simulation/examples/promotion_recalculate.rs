//! Offline audit tool. Reads an exported settlement without a database, wallet,
//! network request, or payment capability.
use std::io::Read;

use mir2_simulation::promotion::{
    calculate_commission, calculate_test_usdc_micro_commission, CommissionInput, CommissionResult,
    ALGORITHM_VERSION,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn main() {
    if let Err(error) = verify() {
        eprintln!("promotion settlement verification failed: {error}");
        std::process::exit(1);
    }
}

fn verify() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: promotion_recalculate <export.json>")?;
    if args.next().is_some() {
        return Err("exactly one exported settlement file is required".into());
    }
    const LIMIT: u64 = 16 * 1024 * 1024;
    let file = std::fs::File::open(path).map_err(|_| "cannot read exported settlement")?;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read exported settlement")?;
    if bytes.len() as u64 > LIMIT {
        return Err("export exceeds 16MiB audit limit".into());
    }
    let exported: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid export JSON")?;
    let canonical = exported["snapshotCanonicalJson"]
        .as_str()
        .ok_or("snapshotCanonicalJson is required")?;
    let expected_sha = exported["snapshotSha256"]
        .as_str()
        .ok_or("snapshotSha256 is required")?;
    let sha = Sha256::digest(canonical.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if expected_sha != sha {
        return Err("snapshot SHA-256 does not match original bytes".into());
    }
    let snapshot: Value = serde_json::from_str(canonical).map_err(|_| "invalid snapshot JSON")?;
    let input: CommissionInput = serde_json::from_value(snapshot["input"].clone())
        .map_err(|_| "invalid calculator input")?;
    let expected: CommissionResult = serde_json::from_value(snapshot["result"].clone())
        .map_err(|_| "invalid saved calculator result")?;
    if expected.algorithm_version != ALGORITHM_VERSION {
        return Err("snapshot requires a different algorithm version".into());
    }
    let actual = if expected.payable_micro_usd.is_some() {
        calculate_test_usdc_micro_commission(&input)?
    } else {
        calculate_commission(&input)?
    };
    if actual != expected {
        return Err("recalculated settlement differs from saved result".into());
    }
    println!(
        "{}",
        json!({"verified":true,"algorithmVersion":ALGORITHM_VERSION,"snapshotSha256":sha,
        "memberCount":actual.members.len(),"eligibleRevenueMicroUsd":actual.eligible_revenue_micro_usd.to_string(),
        "commissionMicroUsd":actual.commission_micro_usd.to_string(),"payableCents":actual.payable_cents.to_string(),
        "payableMicroUsd":actual.payable_micro_usd.map(|amount|amount.to_string())})
    );
    Ok(())
}

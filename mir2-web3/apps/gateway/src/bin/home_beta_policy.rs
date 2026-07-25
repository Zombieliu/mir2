use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    reconcile_home_node_reward, verify_home_network_beta_cohort, GameRewardPolicy,
    HomeNetworkBetaRunPayload, NodeSigningIdentity, SignedHomeNetworkBetaRun,
    SignedHomeNodeTelemetry, VerifiedWorkReceipt,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

fn main() {
    if let Err(error) = run() {
        eprintln!("HOME_BETA_POLICY_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.first().map(String::as_str) {
        Some("sign-telemetry") if arguments.len() == 3 => {
            let payload = read_json(&arguments[1])?;
            let identity = identity_from_file_env("MIR2_HOME_NODE_SIGNING_KEY_FILE")?;
            write_json(
                &arguments[2],
                &SignedHomeNodeTelemetry::sign(payload, &identity)?,
            )?;
            println!(
                "HOME_TELEMETRY_SIGNED node={} output={}",
                identity.node_id(),
                arguments[2]
            );
            Ok(())
        }
        Some("verify-telemetry") if arguments.len() == 3 => {
            let report: SignedHomeNodeTelemetry = read_json(&arguments[1])?;
            let maximum_age_ms = parse_positive_u64("maximum age", &arguments[2])?;
            report.verify(now_ms(), maximum_age_ms)?;
            println!(
                "HOME_TELEMETRY_VERIFIED node={} sequence={}",
                report.payload.node_id, report.payload.sequence
            );
            Ok(())
        }
        Some("reconcile") if arguments.len() == 5 => {
            let report: SignedHomeNodeTelemetry = read_json(&arguments[1])?;
            let receipts: Vec<VerifiedWorkReceipt> = read_json(&arguments[2])?;
            let policy: GameRewardPolicy = read_json(&arguments[3])?;
            let maximum_age_ms = env::var("MIR2_HOME_TELEMETRY_MAXIMUM_AGE_MS")
                .ok()
                .map(|value| parse_positive_u64("telemetry maximum age", &value))
                .transpose()?
                .unwrap_or(5 * 60 * 1_000);
            let result = reconcile_home_node_reward(
                &report,
                &receipts,
                &policy,
                now_ms(),
                maximum_age_ms,
            )?;
            if !result.payable {
                return Err(format!(
                    "reward reconciliation is not payable: {}",
                    result.discrepancies.join(",")
                ));
            }
            write_json(&arguments[4], &result)?;
            println!(
                "HOME_REWARD_RECONCILED node={} reward={} output={}",
                result.node_id, result.estimated_reward, arguments[4]
            );
            Ok(())
        }
        Some("sign-run") if arguments.len() == 3 => {
            let payload: HomeNetworkBetaRunPayload = read_json(&arguments[1])?;
            let node = identity_from_file_env("MIR2_HOME_NODE_SIGNING_KEY_FILE")?;
            let operator = identity_from_file_env("MIR2_HOME_BETA_OPERATOR_SIGNING_KEY_FILE")?;
            let run = SignedHomeNetworkBetaRun::sign(payload, &node, &operator)?;
            write_json(&arguments[2], &run)?;
            println!(
                "HOME_BETA_RUN_SIGNED run={} node={} output={}",
                run.payload.run_id, run.payload.node_id, arguments[2]
            );
            Ok(())
        }
        Some("verify-run") if arguments.len() == 4 => {
            let run: SignedHomeNetworkBetaRun = read_json(&arguments[1])?;
            let require_physical = match arguments[2].as_str() {
                "production" => true,
                "non-production" => false,
                _ => {
                    return Err(
                        "run verification mode must be production or non-production".to_string()
                    )
                }
            };
            run.verify(&arguments[3], require_physical)?;
            println!(
                "HOME_BETA_RUN_VERIFIED run={} environment={:?}",
                run.payload.run_id, run.payload.environment
            );
            Ok(())
        }
        Some("verify-cohort") if arguments.len() >= 6 => {
            let trusted_operator = &arguments[1];
            let output = &arguments[2];
            let runs = arguments[3..]
                .iter()
                .map(|path| read_json(path))
                .collect::<Result<Vec<SignedHomeNetworkBetaRun>, String>>()?;
            let acceptance = verify_home_network_beta_cohort(&runs, trusted_operator)?;
            write_json(output, &acceptance)?;
            println!(
                "HOME_BETA_COHORT_ACCEPTED physical_runs={} providers={} output={output}",
                acceptance.physical_run_count, acceptance.distinct_provider_count
            );
            Ok(())
        }
        _ => Err(
            "usage:\n  home_beta_policy sign-telemetry <payload.json> <signed.json>\n  home_beta_policy verify-telemetry <signed.json> <maximum-age-ms>\n  home_beta_policy reconcile <signed-telemetry.json> <receipts.json> <policy.json> <output.json>\n  home_beta_policy sign-run <payload.json> <signed.json>\n  home_beta_policy verify-run <signed.json> <production|non-production> <trusted-operator-public-key>\n  home_beta_policy verify-cohort <trusted-operator-public-key> <output.json> <run1.json> <run2.json> <run3.json> [runN.json]"
                .to_string(),
        ),
    }
}

fn identity_from_file_env(name: &str) -> Result<NodeSigningIdentity, String> {
    let path = env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))?;
    NodeSigningIdentity::from_file(path)
}

fn read_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("read {}: {error}", Path::new(path).display()))?,
    )
    .map_err(|error| format!("decode {}: {error}", Path::new(path).display()))
}

fn write_json<T: Serialize>(path: &str, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode JSON evidence: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", Path::new(path).display()))
}

fn parse_positive_u64(label: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label} must be a positive integer"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

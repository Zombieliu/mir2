use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    HomeAgentKeyring, HomeBetaRunJournal, HomeBetaRunMetadata, NodeSignedHomeNetworkBetaRun,
    SignedHomeBetaTestPlan,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

const DEFAULT_KEYRING_ACCOUNT: &str = "default";

fn main() {
    if let Err(error) = run() {
        eprintln!("HOME_BETA_RUNNER_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.first().map(String::as_str) {
        Some("begin") if arguments.len() == 5 => {
            let plan: SignedHomeBetaTestPlan = read_json(&arguments[1])?;
            let node = node_identity()?;
            let journal = HomeBetaRunJournal::begin(
                plan,
                &arguments[2],
                &node,
                &arguments[3],
                now_ms(),
            )?;
            write_json(&arguments[4], &journal)?;
            println!(
                "HOME_BETA_RUN_STARTED plan={} node={} output={}",
                journal.plan.payload.plan_id, journal.plan.payload.node_id, arguments[4]
            );
            Ok(())
        }
        Some("start-action") if arguments.len() == 3 => {
            let mut journal: HomeBetaRunJournal = read_json(&arguments[1])?;
            let node = node_identity()?;
            let action = journal.start_action(&node, &arguments[2], now_ms())?;
            write_json(&arguments[1], &journal)?;
            println!(
                "HOME_BETA_ACTION_STARTED sequence={} fault={:?} execution={:?} timeoutMs={}",
                action.sequence, action.fault, action.execution, action.timeout_ms
            );
            Ok(())
        }
        Some("complete-action") if arguments.len() == 7 => {
            let mut journal: HomeBetaRunJournal = read_json(&arguments[1])?;
            let sessions_before = parse_positive_u32("sessions-before", &arguments[3])?;
            let sessions_recovered = parse_positive_u32("sessions-recovered", &arguments[4])?;
            let duplicate_count = arguments[5]
                .parse::<u64>()
                .map_err(|_| "duplicate-count must be an unsigned integer".to_string())?;
            let evidence_sha256 = sha256_file(&arguments[6])?;
            let node = node_identity()?;
            journal.complete_action(
                sessions_before,
                sessions_recovered,
                duplicate_count,
                evidence_sha256,
                &node,
                &arguments[2],
                now_ms(),
            )?;
            write_json(&arguments[1], &journal)?;
            println!(
                "HOME_BETA_ACTION_COMPLETED completed={} remaining={}",
                journal.observations.len(),
                journal
                    .plan
                    .payload
                    .actions
                    .len()
                    .saturating_sub(journal.observations.len())
            );
            Ok(())
        }
        Some("finish") if arguments.len() == 10 => {
            let journal: HomeBetaRunJournal = read_json(&arguments[1])?;
            let provider_asn = arguments[4]
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| "provider-asn must be a positive integer".to_string())?;
            let active_session_minutes = arguments[7]
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| "active-session-minutes must be positive".to_string())?;
            let node = node_identity()?;
            let payload = journal.finish(
                &node,
                &arguments[2],
                now_ms(),
                HomeBetaRunMetadata {
                    provider_code: arguments[3].clone(),
                    provider_asn,
                    failure_domain: arguments[5].clone(),
                    coarse_region: arguments[6].clone(),
                    active_session_minutes,
                    machine_attestation_sha256: sha256_file(&arguments[8])?,
                },
            )?;
            let node_signed = NodeSignedHomeNetworkBetaRun::sign(payload, &node)?;
            write_json(&arguments[9], &node_signed)?;
            println!(
                "HOME_BETA_RUN_NODE_SIGNED run={} node={} output={}",
                node_signed.payload.run_id, node_signed.payload.node_id, arguments[9]
            );
            Ok(())
        }
        Some("status") if arguments.len() == 2 => {
            let journal: HomeBetaRunJournal = read_json(&arguments[1])?;
            println!(
                "HOME_BETA_RUN_STATUS plan={} completed={} total={} active={}",
                journal.plan.payload.plan_id,
                journal.observations.len(),
                journal.plan.payload.actions.len(),
                journal.active_action_started_at_ms.is_some()
            );
            Ok(())
        }
        _ => Err(
            "usage:\n  home_beta_runner begin <signed-plan.json> <trusted-operator-public-key> <build-commit> <journal.json>\n  home_beta_runner start-action <journal.json> <build-commit>\n  home_beta_runner complete-action <journal.json> <build-commit> <sessions-before> <sessions-recovered> <duplicate-count> <evidence-file>\n  home_beta_runner finish <journal.json> <build-commit> <provider-code> <provider-asn> <failure-domain> <coarse-region> <active-session-minutes> <machine-attestation-file> <node-signed-run.json>\n  home_beta_runner status <journal.json>"
                .to_string(),
        ),
    }
}

fn node_identity() -> Result<mir2_gateway::NodeSigningIdentity, String> {
    let account = env::var("MIR2_HOME_AGENT_KEYRING_ACCOUNT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_KEYRING_ACCOUNT.to_string());
    HomeAgentKeyring::new(account)?.load_identity()
}

fn sha256_file(path: &str) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("read {}: {error}", Path::new(path).display()))?;
    if bytes.is_empty() {
        return Err("Home Beta evidence file must not be empty".to_string());
    }
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn parse_positive_u32(label: &str, value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label} must be a positive integer"))
}

fn read_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("read {}: {error}", Path::new(path).display()))?,
    )
    .map_err(|error| format!("decode {}: {error}", Path::new(path).display()))
}

fn write_json<T: Serialize>(path: &str, value: &T) -> Result<(), String> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("encode JSON: {error}"))?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", Path::new(path).display()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

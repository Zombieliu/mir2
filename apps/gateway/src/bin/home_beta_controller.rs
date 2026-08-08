use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    HomeBetaActionExecution, HomeBetaFaultKind, HomeBetaPlanAction, HomeBetaTestPlanPayload,
    NodeSigningIdentity, SignedHomeBetaTestPlan, HOME_BETA_MAXIMUM_FAILOVER_RTO_MS,
    HOME_BETA_MINIMUM_DURATION_MS, HOME_BETA_PLAN_MAXIMUM_LIFETIME_MS, HOME_BETA_PLAN_SCHEMA,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

fn main() {
    if let Err(error) = run() {
        eprintln!("HOME_BETA_CONTROLLER_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.first().map(String::as_str) {
        Some("template") if arguments.len() == 5 => {
            let node_public_key = &arguments[1];
            let build_commit = &arguments[2];
            let plan_id = &arguments[3];
            let issued_at_ms = now_ms();
            let payload = HomeBetaTestPlanPayload {
                schema: HOME_BETA_PLAN_SCHEMA.to_string(),
                plan_id: plan_id.to_string(),
                node_id: mir2_gateway::node_id_from_public_key(node_public_key)?,
                node_public_key: node_public_key.to_string(),
                key_generation: 1,
                build_commit: build_commit.to_string(),
                issued_at_ms,
                not_before_ms: issued_at_ms,
                expires_at_ms: issued_at_ms + HOME_BETA_PLAN_MAXIMUM_LIFETIME_MS,
                minimum_run_duration_ms: HOME_BETA_MINIMUM_DURATION_MS,
                maximum_failover_rto_ms: HOME_BETA_MAXIMUM_FAILOVER_RTO_MS,
                actions: default_actions(),
            };
            write_json(&arguments[4], &payload)?;
            println!(
                "HOME_BETA_PLAN_TEMPLATE plan={} node={} output={}",
                payload.plan_id, payload.node_id, arguments[4]
            );
            Ok(())
        }
        Some("issue-plan") if arguments.len() == 3 => {
            let payload: HomeBetaTestPlanPayload = read_json(&arguments[1])?;
            let operator = identity_from_file_env("MIR2_HOME_BETA_OPERATOR_SIGNING_KEY_FILE")?;
            let signed = SignedHomeBetaTestPlan::sign(payload, &operator)?;
            write_json(&arguments[2], &signed)?;
            println!(
                "HOME_BETA_PLAN_ISSUED plan={} node={} operator={} output={}",
                signed.payload.plan_id,
                signed.payload.node_id,
                signed.operator_public_key,
                arguments[2]
            );
            Ok(())
        }
        Some("verify-plan") if arguments.len() == 7 => {
            let plan: SignedHomeBetaTestPlan = read_json(&arguments[1])?;
            let now = arguments[6]
                .parse::<u64>()
                .map_err(|_| "now-ms must be an unsigned integer".to_string())?;
            plan.verify(
                &arguments[2],
                &arguments[3],
                &arguments[4],
                &arguments[5],
                now,
            )?;
            println!(
                "HOME_BETA_PLAN_VERIFIED plan={} node={}",
                plan.payload.plan_id, plan.payload.node_id
            );
            Ok(())
        }
        _ => Err(
            "usage:\n  home_beta_controller template <node-public-key> <build-commit> <plan-id> <payload.json>\n  home_beta_controller issue-plan <payload.json> <signed-plan.json>\n  home_beta_controller verify-plan <signed-plan.json> <trusted-operator-public-key> <node-id> <node-public-key> <build-commit> <now-ms>"
                .to_string(),
        ),
    }
}

fn default_actions() -> Vec<HomeBetaPlanAction> {
    HomeBetaFaultKind::required()
        .into_iter()
        .enumerate()
        .map(|(index, fault)| HomeBetaPlanAction {
            sequence: (index + 1) as u16,
            fault,
            execution: match fault {
                HomeBetaFaultKind::CgnatBaseline => HomeBetaActionExecution::PassiveObservation,
                HomeBetaFaultKind::DynamicIpChange
                | HomeBetaFaultKind::RouterRestart
                | HomeBetaFaultKind::HostSleepWake => {
                    HomeBetaActionExecution::LocalUserConfirmation
                }
                HomeBetaFaultKind::PacketLoss | HomeBetaFaultKind::BandwidthCongestion => {
                    HomeBetaActionExecution::BoundedNetworkProbe
                }
                HomeBetaFaultKind::ActiveFailureStandbyTakeover => {
                    HomeBetaActionExecution::StandbyVerification
                }
            },
            timeout_ms: 30_000,
            minimum_observation_ms: 1_000,
        })
        .collect()
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

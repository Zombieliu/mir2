use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    aggregate_public_telemetry, reconcile_home_node_reward, verify_home_network_beta_cohort,
    GameRewardPolicy, HomeAgentWorkMode, HomeBetaEnvironment, HomeBetaFaultKind,
    HomeBetaFaultObservation, HomeNetworkBetaRunPayload, HomeNodeTelemetryPayload,
    NodeSigningIdentity, SignedHomeNetworkBetaRun, SignedHomeNodeTelemetry, VerifiedWorkReceipt,
    HOME_BETA_MINIMUM_DURATION_MS, HOME_BETA_RUN_SCHEMA, HOME_TELEMETRY_SCHEMA,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

fn main() {
    if let Err(error) = run() {
        eprintln!("GATE25_LOCAL_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: home_beta_local_acceptance <output-directory>".to_string())?;
    fs::create_dir_all(&output).map_err(|error| format!("create {}: {error}", output.display()))?;
    let now = now_ms();
    let node = NodeSigningIdentity::from_seed([71; 32]);
    let peer = NodeSigningIdentity::from_seed([72; 32]);
    let operator = NodeSigningIdentity::from_seed([73; 32]);
    let window_start = now.saturating_sub(60_000);
    let telemetry = SignedHomeNodeTelemetry::sign(
        HomeNodeTelemetryPayload {
            schema: HOME_TELEMETRY_SCHEMA.to_string(),
            node_id: node.node_id().to_string(),
            public_key: node.public_key().to_string(),
            key_generation: 1,
            agent_instance_id: "gate25-local-agent".to_string(),
            sequence: 1,
            window_started_at_ms: window_start,
            observed_at_ms: now,
            coarse_region: "local-lab".to_string(),
            provider_code: "simulated-provider".to_string(),
            relay_rtt_ms: 12,
            packet_loss_bps: 0,
            measured_upstream_kbps: 100_000,
            active_sessions: 10,
            active_zones: 1,
            zone_ids: vec!["mir2/map/0".to_string()],
            checkpoint_lag_ms: 8,
            cpu_usage_bps: 2_500,
            memory_usage_bps: 2_000,
            work_mode: HomeAgentWorkMode::Serving,
            capacity_certificate_id: "gate25-local-capacity".to_string(),
            capacity_certificate_expires_at_ms: now.saturating_add(60 * 60 * 1_000),
            capacity_max_sessions: 32,
            capacity_max_zones: 4,
            finalized_control_height: 101,
            placement_generation: 7,
            game_id: "mir2".to_string(),
            reward_epoch: 25,
            verified_work_units: 20,
            session_milliseconds: 10 * 60 * 1_000,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        &node,
    )?;
    telemetry.verify(now, 5_000)?;
    let receipt = VerifiedWorkReceipt {
        receipt_id: "gate25-local-receipt".to_string(),
        game_id: "mir2".to_string(),
        epoch: 25,
        zone_id: "mir2/map/0".to_string(),
        control_height: 101,
        placement_generation: 7,
        work_units: 20,
        availability_bps: 9_950,
        quorum_node_ids: vec![node.node_id().to_string(), peer.node_id().to_string()],
        execution_commitment: sha256_hex(b"gate25-local-execution"),
        observed_at_ms: now.saturating_sub(1_000),
    };
    let policy = GameRewardPolicy {
        game_id: "mir2".to_string(),
        epoch: 25,
        reward_budget: 1_000_000,
        reward_per_work_unit: 10,
        max_reward_per_node: 10_000,
        minimum_availability_bps: 9_000,
        minimum_quorum: 2,
        settlement_coin_type: "0x2::sui::SUI".to_string(),
    };
    let reconciliation = reconcile_home_node_reward(&telemetry, &[receipt], &policy, now, 5_000)?;
    if !reconciliation.payable {
        return Err("local signed reward reconciliation unexpectedly rejected".to_string());
    }
    let public = aggregate_public_telemetry(std::slice::from_ref(&telemetry), 1)?;
    let simulated =
        SignedHomeNetworkBetaRun::sign(simulated_beta_payload(&node, now), &node, &operator)?;
    simulated.verify(operator.public_key(), false)?;
    let production_rejection = simulated.verify(operator.public_key(), true).unwrap_err();
    let cohort_rejection =
        verify_home_network_beta_cohort(&[simulated.clone()], operator.public_key()).unwrap_err();

    write_json(output.join("signed-telemetry.json"), &telemetry)?;
    write_json(output.join("reward-reconciliation.json"), &reconciliation)?;
    write_json(output.join("public-telemetry.json"), &public)?;
    write_json(output.join("simulated-beta-run.json"), &simulated)?;
    let evidence = json!({
        "schema": "obelisk.gate25-local-acceptance.v1",
        "accepted": true,
        "scope": "local-cryptographic-and-policy-only",
        "productionHomeBetaAccepted": false,
        "signedTelemetryVerified": true,
        "rawIpPersisted": false,
        "rewardReconciliationPayable": reconciliation.payable,
        "rewardEstimated": reconciliation.estimated_reward,
        "publicViewContainsNodeId": serde_json::to_string(&public).unwrap_or_default().contains(node.node_id()),
        "simulatedRunNonProductionVerified": true,
        "simulatedRunProductionRejected": true,
        "productionRejection": production_rejection,
        "insufficientCohortRejected": true,
        "cohortRejection": cohort_rejection,
        "externalThreeIspEvidenceProvided": false,
        "cloudDdosEvidenceProvided": false,
        "thirdPartyAuditProvided": false,
        "operatorPublicKey": operator.public_key(),
        "observedAtMs": now,
    });
    write_json(output.join("gate25-local-acceptance.json"), &evidence)?;
    println!(
        "GATE25_LOCAL_ACCEPTED production=false output={}",
        output.display()
    );
    Ok(())
}

fn simulated_beta_payload(node: &NodeSigningIdentity, now: u64) -> HomeNetworkBetaRunPayload {
    let started = now.saturating_sub(HOME_BETA_MINIMUM_DURATION_MS);
    let faults = [
        HomeBetaFaultKind::CgnatBaseline,
        HomeBetaFaultKind::DynamicIpChange,
        HomeBetaFaultKind::RouterRestart,
        HomeBetaFaultKind::HostSleepWake,
        HomeBetaFaultKind::PacketLoss,
        HomeBetaFaultKind::BandwidthCongestion,
        HomeBetaFaultKind::ActiveFailureStandbyTakeover,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, kind)| {
        let injected = started.saturating_add(index as u64 * 10_000);
        HomeBetaFaultObservation {
            kind,
            injected_at_ms: injected,
            recovered_at_ms: injected + 1_500,
            recovery_rto_ms: 1_500,
            sessions_before: 10,
            sessions_recovered: 10,
            economy_duplicate_count: 0,
            passed: true,
            evidence_sha256: sha256_hex(format!("simulated-fault-{index}").as_bytes()),
        }
    })
    .collect();
    HomeNetworkBetaRunPayload {
        schema: HOME_BETA_RUN_SCHEMA.to_string(),
        run_id: "gate25-local-simulated".to_string(),
        environment: HomeBetaEnvironment::SimulatedNetwork,
        node_id: node.node_id().to_string(),
        node_public_key: node.public_key().to_string(),
        key_generation: 1,
        provider_code: "simulated-provider".to_string(),
        provider_asn: 64_500,
        failure_domain: "local-container".to_string(),
        coarse_region: "local-lab".to_string(),
        cgnat_observed: true,
        inbound_port_opened: false,
        relay_ip_hidden: true,
        started_at_ms: started,
        finished_at_ms: now,
        active_session_minutes: 150,
        maximum_failover_rto_ms: 1_500,
        economy_duplicate_count: 0,
        faults,
        build_commit: "local-uncommitted".to_string(),
        machine_attestation_sha256: sha256_hex(b"gate25-local-machine"),
    }
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("encode JSON: {error}"))?;
    fs::write(&path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn sha256_hex(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

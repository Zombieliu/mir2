use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::{
    CapacityChallenge, CapacityChallengeResponse, CapacityWorkload, CommonwareControlLog,
    FinalizedControlBlock, FinalizedControlProjector, FinalizedGuildNodeRegistration,
    GameRewardPolicy, GuildNodeCapability, GuildNodeSecurityRegistry, MultiGameRewardLedger,
    NodeCapacityCertificate, NodeSigningIdentity, ProjectedControlEffect, ReplicatedControlCommand,
    VerifiedWorkReceipt, ZoneHostControlPlane,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;

const DEFAULT_OPERATOR_ADDR: &str = "127.0.0.1:29100";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate13AcceptanceEvidence {
    gate: &'static str,
    accepted: bool,
    generated_at_ms: u64,
    node_id: String,
    operator_address: String,
    sui_network: String,
    sui_package_id: String,
    sui_registration_transaction: String,
    sui_registration_checkpoint: u64,
    capacity_challenge_id: String,
    capacity_completed_commands: u64,
    capacity_p95_latency_ms: u64,
    capacity_certificate_id: String,
    capacity_certificate_issuer: String,
    capacity_certificate_expires_at_ms: u64,
    commonware_quorum: usize,
    commonware_finalized_height: u64,
    membership_eligible: bool,
    reward_batch_id: String,
    reward_merkle_root: String,
    reward_total: u64,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("gate13 acceptance failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let registration_path = required_env("GATE13_REGISTRATION_JSON")?;
    let issuer_path = required_env("GATE13_CAPACITY_ISSUER_KEY_FILE")?;
    let operator_address =
        env::var("GATE13_OPERATOR_ADDR").unwrap_or_else(|_| DEFAULT_OPERATOR_ADDR.to_string());
    let evidence_dir =
        PathBuf::from(env::var("GATE13_EVIDENCE_DIR").unwrap_or_else(|_| ".".to_string()));
    let registration: FinalizedGuildNodeRegistration = serde_json::from_slice(
        &fs::read(&registration_path)
            .map_err(|error| format!("failed to read {registration_path}: {error}"))?,
    )
    .map_err(|error| format!("invalid finalized registration JSON: {error}"))?;
    registration.validate()?;
    let issuer = NodeSigningIdentity::from_file(&issuer_path)?;

    let issued_at_ms = now_ms();
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let challenge = CapacityChallenge {
        challenge_id: format!(
            "gate13-testnet-{}-{issued_at_ms}",
            registration.finality.checkpoint
        ),
        node_id: registration.node_id.clone(),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        issued_at_ms,
        expires_at_ms: issued_at_ms.saturating_add(30_000),
        workload: CapacityWorkload {
            concurrent_sessions: registration.max_sessions.min(16),
            zone_count: registration.max_zones.min(4),
            command_count: 2_000,
            maximum_p95_latency_ms: 100,
            minimum_success_bps: 10_000,
        },
    };
    let response = post_capacity_challenge(&operator_address, &challenge)?;
    response.verify(&registration, now_ms())?;
    let certificate = NodeCapacityCertificate::issue(
        &response,
        &registration,
        &issuer,
        now_ms(),
        60 * 60 * 1_000,
        1,
    )?;

    let committee = ["validator-a", "validator-b", "validator-c", "validator-d"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let log = CommonwareControlLog::new(committee)?;
    let security = Arc::new(GuildNodeSecurityRegistry::with_trusted_capacity_issuers(
        3,
        60_000,
        [issuer.public_key().to_string()],
    )?);
    let rewards = Arc::new(Mutex::new(MultiGameRewardLedger::default()));
    let projector = FinalizedControlProjector::new(
        Arc::new(ZoneHostControlPlane::new(100, 1_000, 0)),
        security.clone(),
    )
    .with_reward_ledger(rewards.clone());

    let sync = log.propose(
        "validator-a",
        vec![ReplicatedControlCommand::SyncFinalizedGuildNode {
            registration: registration.clone(),
            capacity_certificate: certificate.clone(),
            now_ms: now_ms(),
        }
        .envelope(registration.finality.idempotency_key())?],
    )?;
    assert_partial_finality(&log, &sync, &registration.node_id, &security)?;
    let sync = finalize(&log, &sync)?;
    projector.apply(&sync)?;
    if !security.is_eligible(
        &registration.node_id,
        GuildNodeCapability::ExecuteZone,
        now_ms(),
    ) {
        return Err("finalized node did not enter deterministic membership".to_string());
    }

    let policy = GameRewardPolicy {
        game_id: "mir2-gate13".to_string(),
        epoch: 1,
        reward_budget: 1_000_000,
        reward_per_work_unit: 1,
        max_reward_per_node: 1_000_000,
        minimum_availability_bps: 10_000,
        minimum_quorum: 1,
        settlement_coin_type: "0x2::sui::SUI".to_string(),
    };
    let policy_block = log.propose(
        "validator-b",
        vec![
            ReplicatedControlCommand::RegisterGameRewardPolicy { policy }
                .envelope("gate13-reward-policy-1")?,
        ],
    )?;
    let policy_block = finalize(&log, &policy_block)?;
    projector.apply(&policy_block)?;

    rewards
        .lock()
        .map_err(|_| "reward ledger mutex poisoned".to_string())?
        .ingest_verified(VerifiedWorkReceipt {
            receipt_id: format!("gate13-work-{}", response.challenge.challenge_id),
            game_id: "mir2-gate13".to_string(),
            epoch: 1,
            zone_id: "mir2/map/0".to_string(),
            control_height: 1,
            placement_generation: 1,
            work_units: response.completed_commands,
            availability_bps: 10_000,
            quorum_node_ids: vec![registration.node_id.clone()],
            execution_commitment: response.transcript_commitment.clone(),
            observed_at_ms: now_ms(),
        })?;
    let close = log.propose(
        "validator-c",
        vec![ReplicatedControlCommand::FinalizeGameRewardEpoch {
            game_id: "mir2-gate13".to_string(),
            epoch: 1,
        }
        .envelope("gate13-reward-close-1")?],
    )?;
    let close = finalize(&log, &close)?;
    let effects = projector.apply(&close)?;
    let batch = effects
        .into_iter()
        .find_map(|effect| match effect {
            ProjectedControlEffect::RewardEpochFinalized(batch) => Some(batch),
            _ => None,
        })
        .ok_or_else(|| "reward close did not produce a settlement batch".to_string())?;

    let evidence = Gate13AcceptanceEvidence {
        gate: "13-permissionless-guild-node-foundation",
        accepted: true,
        generated_at_ms: now_ms(),
        node_id: registration.node_id,
        operator_address,
        sui_network: registration.finality.network,
        sui_package_id: registration.finality.package_id,
        sui_registration_transaction: registration.finality.transaction_digest,
        sui_registration_checkpoint: registration.finality.checkpoint,
        capacity_challenge_id: response.challenge.challenge_id,
        capacity_completed_commands: response.completed_commands,
        capacity_p95_latency_ms: response.p95_latency_ms,
        capacity_certificate_id: certificate.certificate_id,
        capacity_certificate_issuer: certificate.issuer_public_key,
        capacity_certificate_expires_at_ms: certificate.expires_at_ms,
        commonware_quorum: log.quorum(),
        commonware_finalized_height: projector.last_height(),
        membership_eligible: true,
        reward_batch_id: batch.batch_id,
        reward_merkle_root: batch.merkle_root,
        reward_total: batch.total_reward,
    };
    fs::create_dir_all(&evidence_dir)
        .map_err(|error| format!("failed to create {}: {error}", evidence_dir.display()))?;
    let bytes = serde_json::to_vec_pretty(&evidence)
        .map_err(|error| format!("failed to encode acceptance evidence: {error}"))?;
    let output = evidence_dir.join("gate13-acceptance.json");
    fs::write(&output, [&bytes[..], b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    println!("{}", String::from_utf8_lossy(&bytes));
    Ok(())
}

fn assert_partial_finality(
    log: &CommonwareControlLog,
    block: &mir2_gateway::ControlBlock,
    node_id: &str,
    security: &GuildNodeSecurityRegistry,
) -> Result<(), String> {
    for validator in ["validator-a", "validator-b"] {
        if log.vote(validator, &block.digest)?.is_some() {
            return Err("Commonware finalized before Byzantine quorum".to_string());
        }
    }
    if security.is_eligible(node_id, GuildNodeCapability::ExecuteZone, now_ms()) {
        return Err("node entered membership before Commonware finality".to_string());
    }
    Ok(())
}

fn finalize(
    log: &CommonwareControlLog,
    block: &mir2_gateway::ControlBlock,
) -> Result<FinalizedControlBlock, String> {
    for validator in ["validator-a", "validator-b", "validator-c"] {
        if let Some(finalized) = log.vote(validator, &block.digest)? {
            return Ok(finalized);
        }
    }
    Err("Commonware quorum did not finalize block".to_string())
}

fn post_capacity_challenge(
    address: &str,
    challenge: &CapacityChallenge,
) -> Result<CapacityChallengeResponse, String> {
    let body = serde_json::to_vec(challenge)
        .map_err(|error| format!("failed to encode capacity challenge: {error}"))?;
    let mut stream = TcpStream::connect(address)
        .map_err(|error| format!("failed to connect capacity endpoint {address}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|error| format!("failed to configure capacity endpoint: {error}"))?;
    write!(
        stream,
        "POST /v1/capacity-challenge HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .map_err(|error| format!("failed to write capacity request: {error}"))?;
    stream
        .write_all(&body)
        .map_err(|error| format!("failed to write capacity request body: {error}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("failed to read capacity response: {error}"))?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "capacity endpoint returned malformed HTTP".to_string())?;
    if !head
        .lines()
        .next()
        .is_some_and(|line| line.contains(" 200 "))
    {
        return Err(format!("capacity endpoint rejected challenge: {body}"));
    }
    serde_json::from_str(body).map_err(|error| format!("invalid signed capacity response: {error}"))
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

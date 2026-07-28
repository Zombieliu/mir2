use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::{
    CapacityChallenge, CapacityChallengeResponse, CapacityWorkload, FinalizedGuildNodeRegistration,
    GuildNodeStatus, HomeTunnelPlacement, NodeCapacityCertificate, NodeSigningIdentity,
    SuiFinalityProof,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("HOME_TUNNEL_FIXTURE_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: home_tunnel_fixture <output-directory>".to_string())?;
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("create fixture directory {}: {error}", output_dir.display()))?;
    let node_seed = [31_u8; 32];
    let relay_seed = [32_u8; 32];
    let control_seed = [33_u8; 32];
    let capacity_seed = [34_u8; 32];
    let node = NodeSigningIdentity::from_seed(node_seed);
    let relay = NodeSigningIdentity::from_seed(relay_seed);
    let control = NodeSigningIdentity::from_seed(control_seed);
    let capacity_issuer = NodeSigningIdentity::from_seed(capacity_seed);
    let now = now_ms();
    let registration = FinalizedGuildNodeRegistration {
        node_id: node.node_id().to_string(),
        operator_sui_address: format!("0x{}", "11".repeat(32)),
        public_key: node.public_key().to_string(),
        endpoint: "outbound-only".to_string(),
        failure_domain: "gate22-home-network".to_string(),
        stake_mist: 1_000_000,
        max_sessions: 32,
        max_zones: 4,
        key_generation: 1,
        status: GuildNodeStatus::Active,
        finality: SuiFinalityProof {
            network: "testnet".to_string(),
            package_id: format!("0x{}", "22".repeat(32)),
            transaction_digest: "gate22-local-fixture".to_string(),
            event_sequence: 0,
            checkpoint: 42,
        },
    };
    let challenge = CapacityChallenge {
        challenge_id: "gate22-local-capacity".to_string(),
        node_id: node.node_id().to_string(),
        nonce: URL_SAFE_NO_PAD.encode([41_u8; 32]),
        issued_at_ms: now.saturating_sub(1_000),
        expires_at_ms: now.saturating_add(60_000),
        workload: CapacityWorkload {
            concurrent_sessions: 32,
            max_sessions_per_zone: 16,
            zone_count: 4,
            command_count: 1_000,
            maximum_p95_latency_ms: 200,
            minimum_success_bps: 9_990,
        },
    };
    let response =
        CapacityChallengeResponse::sign(challenge, &node, 1, 1_000, 0, 50, "ab".repeat(32), now)?;
    let certificate = NodeCapacityCertificate::issue(
        &response,
        &registration,
        &capacity_issuer,
        now,
        24 * 60 * 60 * 1_000,
        7,
    )?;
    let placement = HomeTunnelPlacement::issue(
        "gate22-primary-placement",
        "gate22-relay",
        "primary",
        node.node_id(),
        1,
        1,
        16,
        101,
        now.saturating_sub(100),
        now.saturating_add(24 * 60 * 60 * 1_000),
        &control,
    )?;
    write_json(output_dir.join("capacity-certificate.json"), &certificate)?;
    write_json(output_dir.join("placements.json"), &vec![placement])?;
    fs::write(
        output_dir.join("node-signing.key"),
        format!("{}\n", URL_SAFE_NO_PAD.encode(node_seed)),
    )
    .map_err(|error| format!("write node signing fixture: {error}"))?;
    fs::write(
        output_dir.join("relay-signing.key"),
        format!("{}\n", URL_SAFE_NO_PAD.encode(relay_seed)),
    )
    .map_err(|error| format!("write Relay signing fixture: {error}"))?;
    let environment = format!(
        "MIR2_HOME_AGENT_SIGNING_KEY_FILE=/run/gate22/node-signing.key\n\
         MIR2_HOME_RELAY_SIGNING_KEY_FILE=/run/gate22/relay-signing.key\n\
         MIR2_HOME_RELAY_PUBLIC_KEY={}\n\
         MIR2_HOME_CONTROL_ISSUER_PUBLIC_KEY={}\n\
         MIR2_HOME_CAPACITY_ISSUER_PUBLIC_KEY={}\n",
        relay.public_key(),
        control.public_key(),
        capacity_issuer.public_key(),
    );
    fs::write(output_dir.join("fixture.env"), environment)
        .map_err(|error| format!("write fixture.env: {error}"))?;
    println!(
        "HOME_TUNNEL_FIXTURE_READY node_id={} output={}",
        node.node_id(),
        output_dir.display()
    );
    Ok(())
}

fn write_json(path: PathBuf, value: &impl serde::Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode fixture {}: {error}", path.display()))?;
    fs::write(&path, bytes).map_err(|error| format!("write fixture {}: {error}", path.display()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

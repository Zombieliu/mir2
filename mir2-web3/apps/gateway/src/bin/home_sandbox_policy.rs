use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    HomeSandboxManifest, HomeSandboxManifestPayload, HomeSandboxRuntimeLimits, NodeSigningIdentity,
    HOME_SANDBOX_SCHEMA,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("HOME_SANDBOX_POLICY_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.first().map(String::as_str) {
        Some("sign") if arguments.len() == 5 => sign(
            &arguments[1],
            &arguments[2],
            &arguments[3],
            &arguments[4],
        ),
        Some("attest") if arguments.len() == 3 => attest(&arguments[1], &arguments[2]),
        _ => Err(
            "usage: home_sandbox_policy sign <image-digest> <node-id> <network> <output> | attest <manifest> <docker-inspect-json>"
                .to_string(),
        ),
    }
}

fn sign(image_digest: &str, node_id: &str, network: &str, output: &str) -> Result<(), String> {
    let issuer = identity_from_env()?;
    let now = now_ms();
    let manifest = HomeSandboxManifest::sign(
        HomeSandboxManifestPayload {
            schema: HOME_SANDBOX_SCHEMA.to_string(),
            workload_id: required_env("MIR2_HOME_SANDBOX_WORKLOAD_ID")
                .unwrap_or_else(|_| "home-zone-primary".to_string()),
            image_digest: image_digest.to_string(),
            node_id: node_id.to_string(),
            placement_generation: positive_u64_env("MIR2_HOME_SANDBOX_GENERATION", 1)?,
            issued_at_ms: now.saturating_sub(1_000),
            expires_at_ms: now.saturating_add(24 * 60 * 60 * 1_000),
            run_as_uid: 65_534,
            run_as_gid: 65_534,
            read_only_root_filesystem: true,
            no_new_privileges: true,
            drop_all_capabilities: true,
            seccomp_profile_sha256: required_env("MIR2_HOME_SANDBOX_SECCOMP_SHA256")
                .unwrap_or_else(|_| "00".repeat(32)),
            allowed_networks: BTreeSet::from([network.to_string()]),
            writable_paths: BTreeSet::new(),
            runtime_limits: HomeSandboxRuntimeLimits {
                memory_bytes: positive_u64_env(
                    "MIR2_HOME_SANDBOX_MEMORY_BYTES",
                    1024 * 1024 * 1024,
                )?,
                nano_cpus: positive_u64_env("MIR2_HOME_SANDBOX_NANO_CPUS", 2_000_000_000)?,
                pids_limit: positive_u64_env("MIR2_HOME_SANDBOX_PIDS_LIMIT", 128)? as i64,
                maximum_open_files: positive_u64_env("MIR2_HOME_SANDBOX_MAX_OPEN_FILES", 1024)?,
            },
            allowed_environment_names: BTreeSet::from([
                "HOME".to_string(),
                "HOSTNAME".to_string(),
                "LANG".to_string(),
                "MIR2_ACCOUNT_STORE_PATH".to_string(),
                "MIR2_ZONE_HOST_ADDR".to_string(),
                "MIR2_ZONE_HOST_CRYSTAL_WORLD".to_string(),
                "MIR2_ZONE_HOST_MANAGEMENT_TOKEN".to_string(),
                "MIR2_ZONE_HOST_MAX_CONNECTIONS".to_string(),
                "MIR2_ZONE_HOST_MAX_SESSIONS".to_string(),
                "MIR2_ZONE_HOST_MAX_SESSIONS_PER_ZONE".to_string(),
                "MIR2_ZONE_HOST_TOKEN".to_string(),
                "PATH".to_string(),
                "RUST_BACKTRACE".to_string(),
            ]),
        },
        &issuer,
    )?;
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("encode Home Sandbox manifest: {error}"))?;
    fs::write(output, bytes).map_err(|error| format!("write Home Sandbox manifest: {error}"))?;
    println!(
        "HOME_SANDBOX_POLICY_SIGNED issuer={} output={output}",
        issuer.public_key()
    );
    Ok(())
}

fn attest(manifest_path: &str, inspect_path: &str) -> Result<(), String> {
    let manifest: HomeSandboxManifest = read_json(manifest_path)?;
    let inspect: serde_json::Value = read_json(inspect_path)?;
    let issuer = required_env("MIR2_HOME_SANDBOX_ISSUER_PUBLIC_KEY")?;
    let expected_node_id = required_env("MIR2_HOME_SANDBOX_EXPECTED_NODE_ID")?;
    let generation = positive_u64_env("MIR2_HOME_SANDBOX_GENERATION", 1)?;
    manifest.verify(&issuer, &expected_node_id, generation, now_ms())?;
    let attestation = manifest.attest_docker_inspect(&inspect)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&attestation)
            .map_err(|error| format!("encode Home Sandbox attestation: {error}"))?
    );
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("read {}: {error}", Path::new(path).display()))?,
    )
    .map_err(|error| format!("decode {}: {error}", Path::new(path).display()))
}

fn identity_from_env() -> Result<NodeSigningIdentity, String> {
    let file = required_env("MIR2_HOME_SANDBOX_SIGNING_KEY_FILE")?;
    NodeSigningIdentity::from_file(file)
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn positive_u64_env(name: &str, default: u64) -> Result<u64, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("{name} must be a positive integer"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    HomeAgentArtifact, HomeAgentReleaseManifest, HomeAgentReleaseManifestPayload,
    NodeSigningIdentity,
};
use sha2::{Digest, Sha256};

fn main() {
    if let Err(error) = run() {
        eprintln!("HOME_AGENT_RELEASE_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().map(String::as_str) != Some("sign") || arguments.len() != 7 {
        return Err(
            "usage: home_agent_release sign <version> <target> <artifact> <https-url> <channel> <output>"
                .to_string(),
        );
    }
    let version = arguments[1].clone();
    let target = arguments[2].clone();
    let artifact_path = PathBuf::from(&arguments[3]);
    let artifact_url = arguments[4].clone();
    let channel = arguments[5].clone();
    let output = PathBuf::from(&arguments[6]);
    let bytes = fs::read(&artifact_path).map_err(|error| {
        format!(
            "read Home Agent release artifact {}: {error}",
            artifact_path.display()
        )
    })?;
    if bytes.is_empty() {
        return Err("Home Agent release artifact must not be empty".to_string());
    }
    let issuer = release_identity_from_env()?;
    let now = now_ms();
    let manifest = HomeAgentReleaseManifest::sign(
        HomeAgentReleaseManifestPayload {
            schema: mir2_gateway::home_agent_runtime::HOME_AGENT_RELEASE_SCHEMA.to_string(),
            channel,
            version: version.clone(),
            published_at_ms: now.saturating_sub(1_000),
            expires_at_ms: now.saturating_add(7 * 24 * 60 * 60 * 1_000),
            minimum_agent_version: env::var("MIR2_HOME_RELEASE_MINIMUM_VERSION")
                .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
            rollout_id: env::var("MIR2_HOME_RELEASE_ROLLOUT_ID")
                .unwrap_or_else(|_| format!("home-agent-{version}-{now}")),
            artifacts: vec![HomeAgentArtifact {
                target,
                url: artifact_url,
                sha256: hex_digest(&Sha256::digest(&bytes)),
                size_bytes: bytes.len() as u64,
            }],
        },
        &issuer,
    )?;
    let encoded = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("encode Home Agent release manifest: {error}"))?;
    atomic_write(&output, &encoded)?;
    println!(
        "HOME_AGENT_RELEASE_SIGNED version={} issuer={} output={}",
        version,
        issuer.public_key(),
        output.display()
    );
    Ok(())
}

fn release_identity_from_env() -> Result<NodeSigningIdentity, String> {
    let inline = env::var("MIR2_HOME_RELEASE_SIGNING_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let file = env::var("MIR2_HOME_RELEASE_SIGNING_KEY_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (inline, file) {
        (Some(_), Some(_)) => Err(
            "configure only one of MIR2_HOME_RELEASE_SIGNING_KEY or MIR2_HOME_RELEASE_SIGNING_KEY_FILE"
                .to_string(),
        ),
        (Some(value), None) => NodeSigningIdentity::from_base64_seed(&value),
        (None, Some(path)) => NodeSigningIdentity::from_file(path),
        (None, None) => Err(
            "MIR2_HOME_RELEASE_SIGNING_KEY_FILE is required; production signing should use an offline/HSM-backed release step"
                .to_string(),
        ),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Home Agent release output has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create release output directory: {error}"))?;
    let temporary = parent.join(format!(".release-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("create temporary release manifest: {error}"))?;
    file.write_all(bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("persist Home Agent release manifest: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("replace Home Agent release manifest: {error}"))
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

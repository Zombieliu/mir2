use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use mir2_gateway::{
    HomeTunnelPlacement, HomeTunnelRelay, HomeTunnelRelayConfig, HomeTunnelTlsMaterial,
    NodeSigningIdentity,
};
use tokio::sync::watch;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("HOME_RELAY_FATAL {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let relay_id = required_env("MIR2_HOME_RELAY_ID")?;
    let quic_bind = socket_env("MIR2_HOME_RELAY_QUIC_BIND")?;
    let gateway_bind = socket_env("MIR2_HOME_RELAY_GATEWAY_BIND")?;
    let tls = tls_from_env("MIR2_HOME_RELAY")?;
    let relay_identity = signing_identity_from_env(
        "MIR2_HOME_RELAY_SIGNING_KEY",
        "MIR2_HOME_RELAY_SIGNING_KEY_FILE",
    )?;
    let trusted_capacity_issuer = required_env("MIR2_HOME_CAPACITY_ISSUER_PUBLIC_KEY")?;
    let trusted_control_issuer = required_env("MIR2_HOME_CONTROL_ISSUER_PUBLIC_KEY")?;
    let placements_path = PathBuf::from(required_env("MIR2_HOME_PLACEMENTS_FILE")?);
    let placements: Vec<HomeTunnelPlacement> =
        serde_json::from_slice(&std::fs::read(&placements_path).map_err(|error| {
            format!(
                "read Home Tunnel placements {}: {error}",
                placements_path.display()
            )
        })?)
        .map_err(|error| {
            format!(
                "decode Home Tunnel placements {}: {error}",
                placements_path.display()
            )
        })?;
    let mut config = HomeTunnelRelayConfig::with_defaults(
        relay_id.clone(),
        quic_bind,
        gateway_bind,
        tls,
        relay_identity,
        trusted_capacity_issuer,
        trusted_control_issuer,
        placements,
    );
    config.placements_file = Some(placements_path);
    config.gateway_auth_token = env::var("MIR2_HOME_RELAY_GATEWAY_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    if let Some(value) = optional_positive_usize_env("MIR2_HOME_RELAY_MAX_AGENT_CONNECTIONS")? {
        config.max_agent_connections = value;
    }
    if let Some(value) = optional_positive_usize_env("MIR2_HOME_RELAY_MAX_GATEWAY_CONNECTIONS")? {
        config.max_gateway_connections = value;
    }
    if let Some(value) = optional_positive_usize_env("MIR2_HOME_RELAY_MAX_STREAMS_PER_NODE")? {
        config.max_streams_per_node = value;
    }
    let relay = HomeTunnelRelay::bind(config).await?;
    let bound_quic = relay.quic_addr()?;
    let bound_gateway = relay.gateway_addr()?;
    println!("HOME_RELAY_READY relay_id={relay_id} quic={bound_quic} gateway={bound_gateway}");
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });
    relay.serve(shutdown_rx).await
}

fn tls_from_env(prefix: &str) -> Result<HomeTunnelTlsMaterial, String> {
    let ca = required_env(&format!("{prefix}_TLS_CA_DER"))?;
    let chain = required_env(&format!("{prefix}_TLS_CERT_CHAIN_DER"))?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if chain.is_empty() {
        return Err(format!("{prefix}_TLS_CERT_CHAIN_DER must not be empty"));
    }
    let key = required_env(&format!("{prefix}_TLS_KEY_DER"))?;
    HomeTunnelTlsMaterial::from_der_files(ca, &chain, key)
}

fn signing_identity_from_env(
    inline_name: &str,
    file_name: &str,
) -> Result<NodeSigningIdentity, String> {
    let inline = env::var(inline_name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let file = env::var(file_name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (inline, file) {
        (Some(_), Some(_)) => Err(format!(
            "configure only one of {inline_name} or {file_name}"
        )),
        (Some(value), None) => NodeSigningIdentity::from_base64_seed(&value),
        (None, Some(path)) => NodeSigningIdentity::from_file(path),
        (None, None) => Err(format!("{inline_name} or {file_name} is required")),
    }
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn socket_env(name: &str) -> Result<SocketAddr, String> {
    SocketAddr::from_str(&required_env(name)?).map_err(|error| format!("invalid {name}: {error}"))
}

fn optional_positive_usize_env(name: &str) -> Result<Option<usize>, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<usize>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("{name} must be a positive integer"))
        })
        .transpose()
}

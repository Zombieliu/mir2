use std::env;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;

use mir2_gateway::zone_lease::default_zone_owner_lease_authority_from_env;
use mir2_gateway::{validate_zone_host_bind, GatewayConfig, ZoneHostServer};

const DEFAULT_ZONE_HOST_ADDR: &str = "127.0.0.1:7020";

fn main() -> io::Result<()> {
    let address = env::var("MIR2_ZONE_HOST_ADDR")
        .unwrap_or_else(|_| DEFAULT_ZONE_HOST_ADDR.to_string())
        .parse::<SocketAddr>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let auth_token = env::var("MIR2_ZONE_HOST_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    validate_zone_host_bind(address, auth_token.as_deref())?;

    let mut config = GatewayConfig::default();
    if env_flag("MIR2_ZONE_HOST_CRYSTAL_WORLD", true) {
        config = config.with_crystal_world_runtime();
    }
    if let Ok(account_store_path) = env::var("MIR2_ACCOUNT_STORE_PATH") {
        config = config
            .with_account_store_environment(PathBuf::from(account_store_path))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    }

    let listener = TcpListener::bind(address)?;
    let bound_address = listener.local_addr()?;
    eprintln!(
        "mir2-zone-host listening on {bound_address} pid={} authenticated={}",
        std::process::id(),
        auth_token.is_some()
    );
    Arc::new(ZoneHostServer::new(
        config,
        default_zone_owner_lease_authority_from_env(),
    ))
    .serve(listener)
}

fn env_flag(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| !matches!(value.trim(), "0" | "false" | "FALSE" | "no" | "NO"))
        .unwrap_or(default)
}

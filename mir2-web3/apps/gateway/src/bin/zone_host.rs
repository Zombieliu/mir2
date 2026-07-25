use std::env;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;

use mir2_gateway::zone_lease::default_zone_owner_lease_authority_from_env;
use mir2_gateway::{
    serve_zone_host_operator, validate_zone_host_bind, GatewayConfig, NodeSigningIdentity,
    PostgresEconomyAccountInventoryService, SharedAccountInventoryServiceHandle,
    ZoneHostOperatorConfig, ZoneHostServer, ZoneRpcLimits, ZoneTopology,
};

const DEFAULT_ZONE_HOST_ADDR: &str = "127.0.0.1:7020";
const DEFAULT_ACCOUNT_STORE_PATH: &str = ".mir2-data/accounts.json";

fn main() -> io::Result<()> {
    mir2_gateway::gate15::initialize_from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    use_signing_identity_as_default_host_id()?;
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
        // A standalone Zone Host is the authoritative simulation process. It
        // must activate the same complete Crystal collision catalog as the
        // TCP/Web Gateway in embedded mode; otherwise valid remote movement,
        // ground drops and pickups are evaluated against the starter map only.
        mir2_simulation::set_crystal_full_world_zone_collision(true);
        config = config.with_crystal_world_runtime();
    }
    let account_store_path = env::var("MIR2_ACCOUNT_STORE_PATH")
        .unwrap_or_else(|_| DEFAULT_ACCOUNT_STORE_PATH.to_string());
    config = config
        .with_account_store_environment(PathBuf::from(account_store_path))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let listener = TcpListener::bind(address)?;
    let bound_address = listener.local_addr()?;
    let topology = ZoneTopology::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let runtime_factory = match env::var("MIR2_ECONOMY_DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        Some(database_url) => {
            let service = Arc::new(PostgresEconomyAccountInventoryService::new(database_url));
            service
                .ensure_migrated()
                .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
            topology.runtime_factory_with_account_inventory_service(
                service as SharedAccountInventoryServiceHandle,
            )
        }
        None => topology.runtime_factory(),
    };
    let server = Arc::new(ZoneHostServer::with_options_and_factory(
        config,
        default_zone_owner_lease_authority_from_env(),
        auth_token,
        ZoneRpcLimits::from_env(),
        runtime_factory,
    ));
    server.configure_zone_map_catalog(topology.zone_map_catalog(), topology.all_maps_zone_ids());
    let operator_config = ZoneHostOperatorConfig::from_env(bound_address, &server.health().host_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let operator_listener = TcpListener::bind(operator_config.address)?;
    let operator_address = operator_listener.local_addr()?;
    let operator_server = Arc::clone(&server);
    std::thread::Builder::new()
        .name("mir2-zone-host-operator".to_string())
        .spawn(move || {
            if let Err(error) =
                serve_zone_host_operator(operator_listener, operator_server, operator_config)
            {
                eprintln!("zone host operator server stopped: {error}");
            }
        })?;
    eprintln!(
        "mir2-zone-host listening on {bound_address} metrics=http://{operator_address}/metrics pid={} authenticated={}",
        std::process::id(),
        env::var("MIR2_ZONE_HOST_TOKEN").is_ok()
    );
    server.serve(listener)
}

fn use_signing_identity_as_default_host_id() -> io::Result<()> {
    if env::var("MIR2_ZONE_HOST_ID")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(());
    }
    if let Some(identity) = NodeSigningIdentity::from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?
    {
        // This runs before the server or operator threads start. Both surfaces
        // therefore bind to the same stable key-derived identity.
        env::set_var("MIR2_ZONE_HOST_ID", identity.node_id());
    }
    Ok(())
}

fn env_flag(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| !matches!(value.trim(), "0" | "false" | "FALSE" | "no" | "NO"))
        .unwrap_or(default)
}

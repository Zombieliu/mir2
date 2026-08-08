use std::env;
use std::thread;
use std::time::{Duration, Instant};

use mir2_gateway::{
    TcpZoneOwnerRpcTransport, ZoneId, ZoneOwnerCommandRequest, ZoneOwnerLease,
    ZoneOwnerRpcTransport, ZoneRpcLimits,
};
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::WorldCommand;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HomeTunnelAcceptanceEvidence {
    accepted: bool,
    endpoint: String,
    zone_id: String,
    session_id: String,
    login_success: bool,
    start_game_success: bool,
    keep_alive_success: bool,
    player_account_id: String,
    map_file_name: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("HOME_TUNNEL_ACCEPTANCE_FATAL {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let endpoint = env::var("MIR2_HOME_ACCEPTANCE_RELAY_ADDR")
        .unwrap_or_else(|_| "home-relay:7444".to_string());
    let zone_id =
        env::var("MIR2_HOME_ACCEPTANCE_ZONE_ID").unwrap_or_else(|_| "primary".to_string());
    let session_id = env::var("MIR2_HOME_ACCEPTANCE_SESSION_ID")
        .unwrap_or_else(|_| "gate22-docker-session".to_string());
    let zone_host_token = env::var("MIR2_ZONE_HOST_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let transport = TcpZoneOwnerRpcTransport::with_options(
        endpoint.clone(),
        ZoneId::new(zone_id.clone()),
        session_id.clone(),
        zone_host_token,
        ZoneRpcLimits {
            io_timeout: Duration::from_secs(5),
            ..ZoneRpcLimits::default()
        },
    );
    wait_for_health(&transport)?;
    let lease = ZoneOwnerLease::in_process(&ZoneId::new(zone_id.clone()));
    let login = transport.execute(ZoneOwnerCommandRequest::direct(
        lease.clone(),
        WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        }),
    ))?;
    let login_success = login
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. }));
    let start = transport.execute(ZoneOwnerCommandRequest::direct(
        lease.clone(),
        WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
    ))?;
    let start_game_success = start.outcome.active_identity.is_some();
    let keep_alive = transport.execute(ZoneOwnerCommandRequest::direct(
        lease,
        WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 4242 }),
    ))?;
    let keep_alive_success = keep_alive
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::KeepAlive { time: 4242 }));
    let identity = transport
        .active_identity()?
        .ok_or_else(|| "Home Tunnel acceptance has no active identity".to_string())?;
    let snapshot = transport.world_snapshot()?;
    let evidence = HomeTunnelAcceptanceEvidence {
        accepted: login_success && start_game_success && keep_alive_success,
        endpoint,
        zone_id,
        session_id,
        login_success,
        start_game_success,
        keep_alive_success,
        player_account_id: identity.account_id,
        map_file_name: snapshot.map_file_name,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence)
            .map_err(|error| format!("encode Home Tunnel acceptance evidence: {error}"))?
    );
    if !evidence.accepted {
        return Err("Home Tunnel acceptance invariants failed".to_string());
    }
    Ok(())
}

fn wait_for_health(transport: &TcpZoneOwnerRpcTransport) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match transport.health() {
            Ok(_) => return Ok(()),
            Err(error) if Instant::now() >= deadline => {
                return Err(format!(
                    "Home Tunnel Relay did not become ready within 30s: {error}"
                ))
            }
            Err(_) => {}
        }
        thread::sleep(Duration::from_millis(100));
    }
}

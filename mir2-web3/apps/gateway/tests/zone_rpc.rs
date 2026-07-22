use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use mir2_gateway::{
    validate_zone_host_bind, GatewayConfig, GatewaySession, InMemoryZoneOwnerLeaseAuthority,
    SharedZoneOwnerLeaseAuthority, TcpZoneOwnerRpcTransport, ZoneHostServer, ZoneId,
    ZoneOwnerCommandRequest, ZoneOwnerLeaseAuthority, ZoneOwnerRpcTransport, ZoneRpcLimits,
    ZONE_RPC_PROTOCOL_VERSION,
};
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::WorldCommand;

#[test]
fn tcp_zone_rpc_round_trips_packets_snapshots_and_isolates_sessions() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address, server, stop, handle) = start_server(authority.clone());
    let zone_id = ZoneId::primary();
    let first = test_transport(address, zone_id.clone(), "session-a");
    let second = test_transport(address, zone_id.clone(), "session-b");

    let health = first.health().expect("zone host health should respond");
    assert_eq!(health.protocol_version, ZONE_RPC_PROTOCOL_VERSION);
    assert_eq!(health.process_id, std::process::id());
    assert_eq!(health.session_count, 0);

    let connect_packets = first.on_connect().expect("on_connect should use RPC");
    assert!(!connect_packets.is_empty());
    assert_eq!(server.session_count(), 1);

    let lease = authority.owner_lease(&zone_id);
    let execution = first
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 4242 }),
        ))
        .expect("Crystal client packet should execute remotely");
    assert!(execution
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::KeepAlive { time: 4242 })));

    start_demo_character(&first, lease);
    assert_eq!(
        first
            .active_identity()
            .expect("first identity should round trip")
            .expect("first session should be in world")
            .account_id,
        "demo"
    );
    assert_eq!(
        second
            .active_identity()
            .expect("second identity should round trip"),
        None
    );
    let _ = first
        .world_snapshot()
        .expect("first snapshot should round trip");
    let _ = second
        .world_snapshot()
        .expect("second snapshot should round trip");
    assert_eq!(server.session_count(), 2);

    stop_server(address, stop, handle);
}

#[test]
fn tcp_zone_rpc_rejects_stale_fencing_token_at_host_boundary() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address, _server, stop, handle) = start_server(authority.clone());
    let zone_id = ZoneId::primary();
    let transport = test_transport(address, zone_id.clone(), "fencing-session");
    let stale = authority.owner_lease(&zone_id);
    let current = authority.handoff_zone_owner(&zone_id, "guild-node-2");

    let error = transport
        .execute(ZoneOwnerCommandRequest::direct(stale, WorldCommand::Tick))
        .expect_err("stale lease must be fenced at the remote owner");
    assert!(error.contains("stale_lease"), "unexpected error: {error}");

    transport
        .execute(ZoneOwnerCommandRequest::direct(current, WorldCommand::Tick))
        .expect("current fencing token should execute");

    stop_server(address, stop, handle);
}

#[test]
fn tcp_zone_rpc_client_reconnects_after_host_becomes_available() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve address");
    let address = reservation.local_addr().expect("reserved address");
    drop(reservation);
    let transport = test_transport(address, ZoneId::primary(), "reconnect-session");

    assert!(
        transport.health().is_err(),
        "offline host must be observable"
    );

    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let listener = TcpListener::bind(address).expect("bind reserved address");
    let stop = Arc::new(AtomicBool::new(false));
    let server = test_server(authority);
    let server_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        server
            .serve_until(listener, server_stop)
            .expect("zone host should run");
    });

    let health = wait_for_health(&transport, Duration::from_secs(3));
    assert_eq!(health.protocol_version, ZONE_RPC_PROTOCOL_VERSION);
    stop_server(address, stop, handle);
}

#[test]
fn zone_rpc_enforces_frame_bound_before_network_io() {
    let mut limits = test_limits();
    limits.max_frame_bytes = 8;
    let transport = TcpZoneOwnerRpcTransport::with_options(
        "127.0.0.1:9",
        ZoneId::primary(),
        "bounded-session",
        None,
        limits,
    );
    let error = transport
        .health()
        .expect_err("oversized request should be rejected locally");
    assert!(
        error.contains("exceeds 8 bytes"),
        "unexpected error: {error}"
    );
}

#[test]
fn zone_rpc_authenticates_remote_bind_and_requests() {
    assert!(validate_zone_host_bind("0.0.0.0:7020".parse().unwrap(), None).is_err());
    assert!(validate_zone_host_bind("0.0.0.0:7020".parse().unwrap(), Some("secret")).is_ok());

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind authenticated host");
    let address = listener.local_addr().expect("authenticated host address");
    let authority: SharedZoneOwnerLeaseAuthority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let server = Arc::new(ZoneHostServer::with_options(
        GatewayConfig::default(),
        authority,
        Some("secret".to_string()),
        test_limits(),
    ));
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        server
            .serve_until(listener, server_stop)
            .expect("authenticated host should run");
    });

    let denied = TcpZoneOwnerRpcTransport::with_options(
        address.to_string(),
        ZoneId::primary(),
        "auth-session",
        Some("wrong".to_string()),
        test_limits(),
    );
    assert!(denied.health().unwrap_err().contains("unauthorized"));
    let allowed = TcpZoneOwnerRpcTransport::with_options(
        address.to_string(),
        ZoneId::primary(),
        "auth-session",
        Some("secret".to_string()),
        test_limits(),
    );
    assert_eq!(
        allowed.health().unwrap().protocol_version,
        ZONE_RPC_PROTOCOL_VERSION
    );

    stop_server(address, stop, handle);
}

#[test]
fn zone_host_binary_is_a_separate_authoritative_process() {
    let reservation = TcpListener::bind("127.0.0.1:0").expect("reserve process address");
    let address = reservation.local_addr().expect("process address");
    drop(reservation);

    let mut child = Command::new(env!("CARGO_BIN_EXE_zone_host"))
        .env("MIR2_ZONE_HOST_ADDR", address.to_string())
        .env("MIR2_ZONE_HOST_CRYSTAL_WORLD", "0")
        .env_remove("MIR2_ZONE_HOST_TOKEN")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("zone host process should spawn");
    let _guard = ChildGuard(&mut child);
    let transport = test_transport(address, ZoneId::primary(), "process-session");
    let health = wait_for_health(&transport, Duration::from_secs(5));
    assert_ne!(health.process_id, std::process::id());

    let lease = mir2_gateway::ZoneOwnerLease::in_process(&ZoneId::primary());
    start_demo_character(&transport, lease);
    assert_eq!(
        transport
            .active_identity()
            .expect("remote identity")
            .expect("separate process should own active character")
            .account_id,
        "demo"
    );
}

#[test]
fn gateway_session_uses_zone_host_from_environment() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address, server, stop, handle) = start_server(authority);
    let _environment = EnvironmentGuard::set("MIR2_ZONE_HOST_ADDR", &address.to_string());
    let _token_environment = EnvironmentGuard::set("MIR2_ZONE_HOST_TOKEN", "");

    let mut session = GatewaySession::new(GatewayConfig::default());
    assert!(!session.on_connect().is_empty());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(
        session
            .active_identity()
            .expect("gateway should read identity from remote owner")
            .account_id,
        "demo"
    );
    assert_eq!(server.session_count(), 1);

    stop_server(address, stop, handle);
}

fn start_server(
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
) -> (
    SocketAddr,
    Arc<ZoneHostServer>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test zone host");
    let address = listener.local_addr().expect("test zone host address");
    let stop = Arc::new(AtomicBool::new(false));
    let server = test_server(authority);
    let running_server = Arc::clone(&server);
    let server_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        running_server
            .serve_until(listener, server_stop)
            .expect("test zone host should run");
    });
    (address, server, stop, handle)
}

fn test_server(authority: Arc<InMemoryZoneOwnerLeaseAuthority>) -> Arc<ZoneHostServer> {
    let shared: SharedZoneOwnerLeaseAuthority = authority;
    Arc::new(ZoneHostServer::with_options(
        GatewayConfig::default(),
        shared,
        None,
        test_limits(),
    ))
}

fn test_transport(
    address: SocketAddr,
    zone_id: ZoneId,
    session_id: &str,
) -> TcpZoneOwnerRpcTransport {
    TcpZoneOwnerRpcTransport::with_options(
        address.to_string(),
        zone_id,
        session_id,
        None,
        test_limits(),
    )
}

fn test_limits() -> ZoneRpcLimits {
    ZoneRpcLimits {
        io_timeout: Duration::from_secs(5),
        ..ZoneRpcLimits::default()
    }
}

fn start_demo_character(transport: &TcpZoneOwnerRpcTransport, lease: mir2_gateway::ZoneOwnerLease) {
    transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }),
        ))
        .expect("demo login should execute remotely");
    transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease,
            WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
        ))
        .expect("demo start game should execute remotely");
}

fn wait_for_health(
    transport: &TcpZoneOwnerRpcTransport,
    timeout: Duration,
) -> mir2_gateway::ZoneHostHealth {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(health) = transport.health() {
            return health;
        }
        assert!(
            Instant::now() < deadline,
            "zone host did not become healthy"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn stop_server(address: SocketAddr, stop: Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
    stop.store(true, Ordering::Release);
    let _ = TcpStream::connect(address);
    handle.join().expect("zone host thread should join");
}

struct ChildGuard<'a>(&'a mut Child);

impl Drop for ChildGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct EnvironmentGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvironmentGuard {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }
}

impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var(self.name, previous);
        } else {
            std::env::remove_var(self.name);
        }
    }
}

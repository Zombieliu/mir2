use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mir2_gateway::routing::PerMapSessionRouter;
use mir2_gateway::{
    validate_zone_host_bind, GatewayConfig, GatewaySession, InMemoryZoneOwnerLeaseAuthority,
    SharedInProcessZoneRuntimeFactory, SharedSessionRouter, SharedZoneOwnerLeaseAuthority,
    SharedZoneRuntimeFactory, TcpZoneOwnerRpcTransport, ZoneHostControlPlane, ZoneHostHeartbeat,
    ZoneHostRegistration, ZoneHostServer, ZoneId, ZoneOwnerCommandRequest, ZoneOwnerLeaseAuthority,
    ZoneOwnerRpcTransport, ZoneRegistry, ZoneRpcLimits, ZONE_RPC_PROTOCOL_VERSION,
};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
use mir2_simulation::WorldCommand;

static ENVIRONMENT_TEST_LOCK: Mutex<()> = Mutex::new(());

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
    assert_eq!(health.session_capacity, test_limits().max_sessions);
    assert!(!health.draining);
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
fn scheduler_places_across_hosts_and_draining_primary_rejects_new_sessions() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address_a, server_a, stop_a, handle_a) =
        start_named_server(authority.clone(), "host-a", 8);
    let (address_b, server_b, stop_b, handle_b) =
        start_named_server(authority.clone(), "host-b", 8);
    let (address_c, server_c, stop_c, handle_c) =
        start_named_server(authority.clone(), "host-c", 8);
    let control = ZoneHostControlPlane::new(1_000, 5_000, 1);

    for (host_id, domain, address) in [
        ("host-a", "az-a", address_a),
        ("host-b", "az-b", address_b),
        ("host-c", "az-c", address_c),
    ] {
        let probe = test_transport(address, ZoneId::primary(), &format!("probe-{host_id}"));
        let health = probe.health().expect("named Zone Host should be healthy");
        assert_eq!(health.host_id, host_id);
        control
            .register_host(
                ZoneHostRegistration::from_health(address.to_string(), domain, 100, &health),
                ZoneHostHeartbeat::from_health(&health, 10),
            )
            .expect("healthy Zone Host should register");
    }

    let zone_id = ZoneId::new("map:0");
    let placement = control
        .place_zone(zone_id.clone(), 10)
        .expect("Zone should receive primary and replica");
    assert_ne!(
        placement.primary.failure_domain,
        placement.replicas[0].failure_domain
    );
    let server_for = |host_id: &str| match host_id {
        "host-a" => &server_a,
        "host-b" => &server_b,
        "host-c" => &server_c,
        other => panic!("unexpected scheduled host {other}"),
    };
    let active_server = server_for(&placement.primary.host_id);
    let replica_server = server_for(&placement.replicas[0].host_id);

    let first = TcpZoneOwnerRpcTransport::with_placement(
        &placement,
        "scheduled-first",
        None,
        test_limits(),
    )
    .expect("placement should produce a transport");
    let lease = authority.owner_lease(&zone_id);
    first
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 1 }),
        ))
        .expect("scheduled primary should execute");
    assert_eq!(active_server.session_count(), 1);

    active_server.set_draining(true);
    let second = TcpZoneOwnerRpcTransport::with_placement(
        &placement,
        "scheduled-during-drain",
        None,
        test_limits(),
    )
    .expect("placement should preserve the replica endpoint");
    second
        .execute(ZoneOwnerCommandRequest::direct(
            lease,
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 2 }),
        ))
        .expect("new session should skip the draining primary and use the replica");
    assert_eq!(active_server.session_count(), 1);
    assert_eq!(replica_server.session_count(), 1);

    let moves = control
        .begin_drain(&placement.primary.host_id, 20)
        .expect("scheduler should move placement away from draining primary");
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].next.generation, placement.generation + 1);
    assert!(!moves[0]
        .next
        .host_ids()
        .any(|host_id| host_id == placement.primary.host_id));

    stop_server(address_a, stop_a, handle_a);
    stop_server(address_b, stop_b, handle_b);
    stop_server(address_c, stop_c, handle_c);
}

#[test]
fn tcp_zone_rpc_close_session_is_fenced_and_replays_as_a_checkpoint_tombstone() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (active_address, active_server, active_stop, active_handle) =
        start_server(authority.clone());
    let zone_id = ZoneId::primary();
    let active = test_transport(active_address, zone_id.clone(), "closed-session");
    let lease = authority.owner_lease(&zone_id);
    active
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 1 }),
        ))
        .expect("session should exist before close");
    assert_eq!(active_server.session_count(), 1);

    ZoneOwnerRpcTransport::close_session(&active, &lease)
        .expect("current owner fence should close the remote session");
    assert_eq!(active_server.session_count(), 0);
    let checkpoint = active
        .export_host_checkpoint()
        .expect("closed-session checkpoint should export");

    let (standby_address, standby_server, standby_stop, standby_handle) =
        start_server(authority.clone());
    let standby = test_transport(standby_address, zone_id, "checkpoint-installer");
    standby
        .install_host_checkpoint(&checkpoint)
        .expect("close tombstone should replay on a fresh host");
    assert_eq!(standby_server.session_count(), 0);

    stop_server(active_address, active_stop, active_handle);
    stop_server(standby_address, standby_stop, standby_handle);
}

#[test]
fn tcp_zone_rpc_reliably_resumes_live_cross_session_outbounds() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address, _server, stop, handle) = start_server(authority.clone());
    let zone_id = ZoneId::primary();
    let owner = test_transport(address, zone_id.clone(), "live-owner");
    let observer = test_transport(address, zone_id.clone(), "live-observer");
    let lease = authority.owner_lease(&zone_id);

    start_new_character(&owner, lease.clone(), "live-owner", "LiveOwner");
    start_new_character(&observer, lease.clone(), "live-observer", "LiveObserver");
    owner
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::TransferMap {
                key: "crystal:0:330:270".to_string(),
            },
        ))
        .expect("owner should enter live outbound fixture");
    observer
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::TransferMap {
                key: "crystal:0:340:270".to_string(),
            },
        ))
        .expect("observer should enter live outbound fixture");

    // Acknowledge setup broadcasts so the movement assertion starts from a
    // stable cursor.
    let setup = observer
        .poll_outbounds(0, 128)
        .expect("observer setup outbounds should poll");
    let setup_ack = setup
        .items
        .last()
        .map(|item| item.sequence)
        .unwrap_or_default();
    observer
        .poll_outbounds(setup_ack, 128)
        .expect("observer setup outbounds should acknowledge");

    owner
        .execute(ZoneOwnerCommandRequest::direct(
            lease,
            WorldCommand::ClientPacket(ClientPacket::Walk {
                direction: MirDirection::Right,
            }),
        ))
        .expect("owner movement should execute remotely");

    let first = wait_for_outbounds(&observer, setup_ack, Duration::from_secs(2));
    assert!(first.items.iter().any(|item| {
        matches!(
            item.packet,
            ServerPacket::ObjectWalk { ref movement }
                if movement.direction == MirDirection::Right
        )
    }));
    let redelivered = observer
        .poll_outbounds(setup_ack, 128)
        .expect("unacknowledged outbounds should be replayable");
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.sequence)
            .collect::<Vec<_>>(),
        redelivered
            .items
            .iter()
            .map(|item| item.sequence)
            .collect::<Vec<_>>()
    );

    let acknowledged = first.items.last().expect("movement outbound").sequence;
    let drained = observer
        .poll_outbounds(acknowledged, 128)
        .expect("acknowledged outbounds should drain");
    assert!(drained.items.is_empty());

    stop_server(address, stop, handle);
}

#[test]
fn tcp_zone_rpc_registration_bridges_live_outbounds_to_gateway_channel() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address, _server, stop, handle) = start_server(authority.clone());
    let zone_id = ZoneId::primary();
    let owner = test_transport(address, zone_id.clone(), "bridge-owner");
    let observer = test_transport(address, zone_id.clone(), "bridge-observer");
    let lease = authority.owner_lease(&zone_id);

    start_new_character(&owner, lease.clone(), "bridge-owner", "BridgeOwner");
    start_new_character(
        &observer,
        lease.clone(),
        "bridge-observer",
        "BridgeObserver",
    );
    owner
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::TransferMap {
                key: "crystal:0:330:270".to_string(),
            },
        ))
        .expect("owner should enter bridge fixture");
    observer
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::TransferMap {
                key: "crystal:0:340:270".to_string(),
            },
        ))
        .expect("observer should enter bridge fixture");

    let setup = observer.poll_outbounds(0, 128).expect("setup poll");
    let setup_ack = setup
        .items
        .last()
        .map(|item| item.sequence)
        .unwrap_or_default();
    observer
        .poll_outbounds(setup_ack, 128)
        .expect("setup acknowledge");

    let (sender, mut receiver) = tokio::sync::mpsc::channel(16);
    let registration = ZoneOwnerRpcTransport::register_live_outbound(&observer, sender)
        .expect("remote live outbound registration should succeed")
        .expect("TCP transport should provide a live registration");
    registration.activate();

    owner
        .execute(ZoneOwnerCommandRequest::direct(
            lease,
            WorldCommand::ClientPacket(ClientPacket::Walk {
                direction: MirDirection::Right,
            }),
        ))
        .expect("owner movement should execute remotely");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(outbound) = receiver.try_recv() {
            assert_eq!(outbound.registration_id(), registration.registration_id());
            if matches!(
                outbound.into_packet(),
                ServerPacket::ObjectWalk { ref movement }
                    if movement.direction == MirDirection::Right
            ) {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "registered gateway outbound did not arrive"
        );
        thread::sleep(Duration::from_millis(10));
    }

    drop(registration);
    stop_server(address, stop, handle);
}

#[test]
fn zone_host_checkpoint_replays_two_sessions_and_promotes_under_a_new_fence() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (active_address, _active_server, active_stop, active_handle) =
        start_server(authority.clone());
    let (standby_address, _standby_server, standby_stop, standby_handle) =
        start_server(authority.clone());
    let zone_id = ZoneId::primary();
    let active_owner = test_transport(active_address, zone_id.clone(), "checkpoint-owner");
    let active_observer = test_transport(active_address, zone_id.clone(), "checkpoint-observer");
    let standby_owner = test_transport(standby_address, zone_id.clone(), "checkpoint-owner");
    let standby_observer = test_transport(standby_address, zone_id.clone(), "checkpoint-observer");
    let lease = authority.owner_lease(&zone_id);

    start_new_character(
        &active_owner,
        lease.clone(),
        "checkpoint-owner",
        "CheckpointOwner",
    );
    start_new_character(
        &active_observer,
        lease.clone(),
        "checkpoint-observer",
        "CheckpointObserver",
    );
    active_owner
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::TransferMap {
                key: "crystal:0:330:270".to_string(),
            },
        ))
        .expect("active owner should transfer");
    active_observer
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::TransferMap {
                key: "crystal:0:340:270".to_string(),
            },
        ))
        .expect("active observer should transfer");
    active_owner
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::Walk {
                direction: MirDirection::Right,
            }),
        ))
        .expect("active movement should execute");

    let owner_snapshot = active_owner
        .world_snapshot()
        .expect("active owner snapshot");
    let observer_snapshot = active_observer
        .world_snapshot()
        .expect("active observer snapshot");
    let checkpoint = active_owner
        .export_host_checkpoint()
        .expect("active host checkpoint should export");
    assert!(checkpoint.entry_count >= 11);
    assert_eq!(checkpoint.session_count, 2);
    standby_owner
        .install_host_checkpoint(&checkpoint)
        .expect("standby host should install checkpoint");

    assert_eq!(
        standby_owner
            .active_identity()
            .expect("standby owner identity"),
        active_owner
            .active_identity()
            .expect("active owner identity")
    );
    assert_eq!(
        standby_observer
            .active_identity()
            .expect("standby observer identity"),
        active_observer
            .active_identity()
            .expect("active observer identity")
    );
    assert_eq!(
        standby_owner
            .world_snapshot()
            .expect("standby owner snapshot"),
        owner_snapshot
    );
    assert_eq!(
        standby_observer
            .world_snapshot()
            .expect("standby observer snapshot"),
        observer_snapshot
    );
    let standby_checkpoint = standby_owner
        .export_host_checkpoint()
        .expect("standby checkpoint should export");
    assert_eq!(standby_checkpoint.entry_count, checkpoint.entry_count);
    assert_eq!(standby_checkpoint.session_count, checkpoint.session_count);
    assert_eq!(standby_checkpoint.zone_count, checkpoint.zone_count);
    assert!(checkpoint.zone_state_bytes > 0);
    assert!(standby_checkpoint.zone_state_bytes > 0);

    let promoted = authority.handoff_zone_owner(&zone_id, "standby-owner");
    standby_owner
        .execute(ZoneOwnerCommandRequest::direct(
            promoted,
            WorldCommand::Tick,
        ))
        .expect("promoted standby should execute under the new fence");
    assert!(active_owner
        .execute(ZoneOwnerCommandRequest::direct(lease, WorldCommand::Tick))
        .expect_err("old active must be fenced")
        .contains("stale_lease"));

    stop_server(active_address, active_stop, active_handle);
    stop_server(standby_address, standby_stop, standby_handle);
}

#[test]
fn multi_endpoint_transport_reroutes_to_replicated_standby_after_active_stops() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (active_address, _active_server, active_stop, active_handle) =
        start_server(authority.clone());
    let (standby_address, _standby_server, standby_stop, standby_handle) =
        start_server(authority.clone());
    let zone_id = ZoneId::primary();
    let active = test_transport(active_address, zone_id.clone(), "failover-session");
    let standby = test_transport(standby_address, zone_id.clone(), "failover-session");
    let lease = authority.owner_lease(&zone_id);
    start_new_character(&active, lease.clone(), "failover", "FailoverPlayer");
    let checkpoint = active
        .export_host_checkpoint()
        .expect("active checkpoint should export");
    standby
        .install_host_checkpoint(&checkpoint)
        .expect("standby checkpoint should install");

    let failover = TcpZoneOwnerRpcTransport::with_endpoints(
        vec![active_address.to_string(), standby_address.to_string()],
        zone_id.clone(),
        "failover-session",
        None,
        test_limits(),
    )
    .expect("failover transport should accept two endpoints");
    assert_eq!(
        failover
            .active_identity()
            .expect("active endpoint identity")
            .expect("active endpoint character")
            .account_id,
        "failover"
    );

    stop_server(active_address, active_stop, active_handle);
    assert_eq!(
        failover
            .active_identity()
            .expect("standby endpoint identity")
            .expect("standby endpoint character")
            .account_id,
        "failover"
    );
    let promoted = authority.handoff_zone_owner(&zone_id, "standby-owner");
    failover
        .execute(ZoneOwnerCommandRequest::direct(
            promoted,
            WorldCommand::Tick,
        ))
        .expect("rerouted standby should execute with promoted fence");
    assert!(failover
        .execute(ZoneOwnerCommandRequest::direct(lease, WorldCommand::Tick))
        .expect_err("stale lease must remain fenced after reroute")
        .contains("stale_lease"));

    stop_server(standby_address, standby_stop, standby_handle);
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
fn zone_replicator_copies_checkpoint_between_separate_host_processes() {
    let active_reservation = TcpListener::bind("127.0.0.1:0").expect("reserve active address");
    let active_address = active_reservation.local_addr().expect("active address");
    drop(active_reservation);
    let standby_reservation = TcpListener::bind("127.0.0.1:0").expect("reserve standby address");
    let standby_address = standby_reservation.local_addr().expect("standby address");
    drop(standby_reservation);

    let mut active_child = spawn_zone_host_process(active_address);
    let _active_guard = ChildGuard(&mut active_child);
    let mut standby_child = spawn_zone_host_process(standby_address);
    let _standby_guard = ChildGuard(&mut standby_child);
    let zone_id = ZoneId::primary();
    let active = test_transport(active_address, zone_id.clone(), "process-replica-session");
    let standby = test_transport(standby_address, zone_id.clone(), "process-replica-session");
    wait_for_health(&active, Duration::from_secs(5));
    wait_for_health(&standby, Duration::from_secs(5));
    start_demo_character(&active, mir2_gateway::ZoneOwnerLease::in_process(&zone_id));

    let output = Command::new(env!("CARGO_BIN_EXE_zone_replicator"))
        .arg("--once")
        .env("MIR2_ZONE_ACTIVE_ADDR", active_address.to_string())
        .env("MIR2_ZONE_STANDBY_ADDR", standby_address.to_string())
        .env_remove("MIR2_ZONE_HOST_TOKEN")
        .output()
        .expect("zone replicator process should spawn");
    assert!(
        output.status.success(),
        "zone replicator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        standby
            .active_identity()
            .expect("standby identity should respond")
            .expect("replicated process should restore active character")
            .account_id,
        "demo"
    );
}

#[test]
fn gateway_session_uses_zone_host_from_environment() {
    let _environment_lock = ENVIRONMENT_TEST_LOCK.lock().expect("environment test lock");
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

    drop(session);
    stop_server(address, stop, handle);
}

#[test]
fn gateway_session_handoffs_between_remote_map_zones_without_leaking_host_sessions() {
    let _environment_lock = ENVIRONMENT_TEST_LOCK.lock().expect("environment test lock");
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address, server, stop, handle) = start_server(authority);
    let _environment = EnvironmentGuard::set("MIR2_ZONE_HOST_ADDR", &address.to_string());
    let _token_environment = EnvironmentGuard::set("MIR2_ZONE_HOST_TOKEN", "");
    let registry = ZoneRegistry::with_router(
        ZoneId::primary(),
        Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
        Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
    );

    let mut session = GatewaySession::new_with_zone_registry(GatewayConfig::default(), &registry);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(session.zone_id(), &ZoneId::new("map:0"));
    assert_eq!(session.handoff_generation(), 1);
    assert_eq!(server.session_count(), 1);

    session.transfer_map("crystal:1:100:100");
    assert_eq!(session.zone_id(), &ZoneId::new("map:1"));
    assert_eq!(session.handoff_generation(), 2);
    assert_eq!(server.session_count(), 1);

    drop(session);
    assert_eq!(server.session_count(), 0);
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

fn start_named_server(
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
    host_id: &str,
    zone_capacity: usize,
) -> (
    SocketAddr,
    Arc<ZoneHostServer>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind named test Zone Host");
    let address = listener.local_addr().expect("named Zone Host address");
    let stop = Arc::new(AtomicBool::new(false));
    let shared: SharedZoneOwnerLeaseAuthority = authority;
    let server = Arc::new(ZoneHostServer::with_identity_and_factory(
        host_id,
        zone_capacity,
        GatewayConfig::default(),
        shared,
        None,
        test_limits(),
        Arc::new(SharedInProcessZoneRuntimeFactory::new()),
    ));
    let running_server = Arc::clone(&server);
    let server_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        running_server
            .serve_until(listener, server_stop)
            .expect("named test Zone Host should run");
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

fn start_new_character(
    transport: &TcpZoneOwnerRpcTransport,
    lease: mir2_gateway::ZoneOwnerLease,
    account_id: &str,
    character_name: &str,
) {
    transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::NewAccount {
                account_id: account_id.to_string(),
                password: account_id.to_string(),
                birth_date_binary: 0,
                user_name: String::new(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            }),
        ))
        .expect("new account should execute remotely");
    transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: account_id.to_string(),
                password: account_id.to_string(),
            }),
        ))
        .expect("new account should login remotely");
    let character_index = transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::NewCharacter {
                name: character_name.to_string(),
                gender: MirGender::Male,
                class: MirClass::Warrior,
            }),
        ))
        .expect("new character should execute remotely")
        .packets
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("new character should return an index");
    transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease,
            WorldCommand::ClientPacket(ClientPacket::StartGame { character_index }),
        ))
        .expect("new character should enter the world remotely");
}

fn wait_for_outbounds(
    transport: &TcpZoneOwnerRpcTransport,
    acknowledged_sequence: u64,
    timeout: Duration,
) -> mir2_gateway::zone_rpc::ZoneHostOutboundBatch {
    let deadline = Instant::now() + timeout;
    loop {
        let batch = transport
            .poll_outbounds(acknowledged_sequence, 128)
            .expect("live outbounds should poll");
        if !batch.items.is_empty() {
            return batch;
        }
        assert!(Instant::now() < deadline, "live outbound did not arrive");
        thread::sleep(Duration::from_millis(10));
    }
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

fn spawn_zone_host_process(address: SocketAddr) -> Child {
    Command::new(env!("CARGO_BIN_EXE_zone_host"))
        .env("MIR2_ZONE_HOST_ADDR", address.to_string())
        .env("MIR2_ZONE_HOST_CRYSTAL_WORLD", "0")
        .env_remove("MIR2_ZONE_HOST_TOKEN")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("zone host process should spawn")
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

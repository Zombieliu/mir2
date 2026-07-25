use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_gateway::routing::PerMapSessionRouter;
use mir2_gateway::{
    validate_zone_host_bind, GatewayConfig, GatewaySession, InMemoryZoneOwnerLeaseAuthority,
    SharedInProcessZoneRuntimeFactory, SharedSessionRouter, SharedZoneOwnerLeaseAuthority,
    SharedZoneRuntimeFactory, TcpZoneOwnerRpcTransport, ZoneBaseSnapshotStore,
    ZoneHostControlPlane, ZoneHostHeartbeat, ZoneHostRegistration, ZoneHostServer, ZoneId,
    ZoneMapScope, ZoneMutationWal, ZoneOwnerCommandRequest, ZoneOwnerLeaseAuthority,
    ZoneOwnerRpcTransport, ZoneRegistry, ZoneReplicationCoverage, ZoneRpcLimits,
    ZONE_REPLICATION_HEAD_VERSION, ZONE_RPC_PROTOCOL_VERSION,
};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
use mir2_simulation::WorldCommand;

static ENVIRONMENT_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn tcp_zone_rpc_round_trips_packets_snapshots_and_isolates_sessions() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address, server, stop, handle) = start_server(authority.clone());
    server.configure_zone_map_catalog(BTreeMap::new(), BTreeSet::from(["primary".to_string()]));
    let zone_id = ZoneId::primary();
    let first = test_transport(address, zone_id.clone(), "session-a");
    let second = test_transport(address, zone_id.clone(), "session-b");

    let health = first.health().expect("zone host health should respond");
    assert_eq!(health.protocol_version, ZONE_RPC_PROTOCOL_VERSION);
    assert_eq!(health.process_id, std::process::id());
    assert_eq!(health.session_capacity, test_limits().max_sessions);
    assert_eq!(
        health.session_capacity_per_zone,
        test_limits().max_sessions_per_zone
    );
    assert_eq!(health.busiest_zone_session_count, 0);
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
    assert_eq!(server.busiest_zone_session_count(), 2);
    let telemetry = server.telemetry_snapshot();
    assert_eq!(telemetry.zones.len(), 1);
    assert_eq!(telemetry.zones[0].zone_id, "primary");
    assert_eq!(telemetry.zones[0].map_scope, ZoneMapScope::All);
    assert!(telemetry.zones[0].map_file_names.is_empty());
    assert_eq!(telemetry.zones[0].session_count, 2);

    stop_server(address, stop, handle);
}

#[test]
fn zone_host_enforces_per_zone_session_capacity_and_releases_it_on_close() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let limits = ZoneRpcLimits {
        max_sessions: 4,
        max_sessions_per_zone: 2,
        ..test_limits()
    };
    let (address, server, stop, handle) = start_server_with_limits(authority.clone(), limits);
    let crowded_zone = ZoneId::new("map:crowded");
    let other_zone = ZoneId::new("map:other");
    let first = test_transport(address, crowded_zone.clone(), "crowded-a");
    let second = test_transport(address, crowded_zone.clone(), "crowded-b");
    let rejected = test_transport(address, crowded_zone.clone(), "crowded-c");
    let distributed = test_transport(address, other_zone, "other-a");

    first.on_connect().expect("first crowded session");
    second.on_connect().expect("second crowded session");
    let error = rejected
        .on_connect()
        .expect_err("third session in one Zone must be rejected");
    assert!(error.contains("session capacity 2 reached"), "{error}");
    distributed
        .on_connect()
        .expect("another Zone should still have capacity");

    let health = first.health().expect("capacity health");
    assert_eq!(health.session_count, 3);
    assert_eq!(health.session_capacity, 4);
    assert_eq!(health.session_capacity_per_zone, 2);
    assert_eq!(health.busiest_zone_session_count, 2);

    let lease = authority.owner_lease(&crowded_zone);
    ZoneOwnerRpcTransport::close_session(&first, &lease)
        .expect("closing a session should release per-Zone capacity");
    rejected
        .on_connect()
        .expect("released per-Zone capacity should be reusable");
    assert_eq!(server.busiest_zone_session_count(), 2);
    let active_zones = server.active_zones();
    assert_eq!(
        active_zones
            .iter()
            .map(|zone| (
                zone.zone_id.as_str(),
                zone.map_file_names.as_slice(),
                zone.session_count,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("map:crowded", &["crowded".to_string()][..], 2),
            ("map:other", &["other".to_string()][..], 1),
        ]
    );

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
    let (active_address, active_server, active_stop, active_handle) =
        start_server(authority.clone());
    let (standby_address, standby_server, standby_stop, standby_handle) =
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
    standby_owner
        .install_host_checkpoint(&checkpoint)
        .expect("repeated full-journal install should replay from a clean account baseline");
    let active_telemetry = active_server.telemetry_snapshot();
    assert_eq!(active_telemetry.checkpoint.exports_total, 1);
    assert_eq!(
        active_telemetry.checkpoint.journal_entries,
        checkpoint.entry_count as u64
    );
    assert_eq!(
        active_telemetry.checkpoint.export_last_bytes,
        checkpoint.as_bytes().len() as u64
    );
    let standby_telemetry = standby_server.telemetry_snapshot();
    assert_eq!(standby_telemetry.checkpoint.installs_total, 2);
    assert_eq!(
        standby_telemetry.checkpoint.replay_entries_total,
        (checkpoint.entry_count * 2) as u64
    );
    assert_eq!(
        standby_telemetry.checkpoint.replay_last_entries,
        checkpoint.entry_count as u64
    );
    assert_eq!(
        standby_telemetry.checkpoint.install_last_bytes,
        checkpoint.as_bytes().len() as u64
    );

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
fn zone_replication_head_is_per_zone_bounded_and_survives_v4_restore() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (active_address, active_server, active_stop, active_handle) =
        start_server(authority.clone());
    let (standby_address, _standby_server, standby_stop, standby_handle) =
        start_server(authority.clone());
    let zone_a = ZoneId::new("map:0");
    let zone_b = ZoneId::new("map:1");
    let active_a = test_transport(active_address, zone_a.clone(), "head-a");
    let active_b = test_transport(active_address, zone_b.clone(), "head-b");
    let standby_a = test_transport(standby_address, zone_a.clone(), "head-a");
    let standby_b = test_transport(standby_address, zone_b.clone(), "head-b");

    let empty = active_a
        .replication_head()
        .expect("empty replication head should respond");
    assert_eq!(empty.version, ZONE_REPLICATION_HEAD_VERSION);
    assert_eq!(empty.zone_id, "map:0");
    assert_eq!(
        empty.mutation_coverage,
        ZoneReplicationCoverage::CommandJournal
    );
    assert!(!empty.promotion_ready);
    assert_eq!(empty.base_snapshot_id, None);
    assert_eq!(empty.base_sequence, 0);
    assert_eq!(empty.oldest_available_sequence, 0);
    assert_eq!(empty.entry_count, 0);
    assert_eq!(empty.next_sequence, 0);
    assert_eq!(empty.last_sequence, None);
    assert_eq!(empty.latest_digest, "0".repeat(64));
    assert!(
        serde_json::to_vec(&empty).unwrap().len() < 1_024,
        "100ms replication head must remain smaller than 1 KiB"
    );

    let lease_a = authority.owner_lease(&zone_a);
    let lease_b = authority.owner_lease(&zone_b);
    for time in [1, 2] {
        active_a
            .execute(ZoneOwnerCommandRequest::direct(
                lease_a.clone(),
                WorldCommand::ClientPacket(ClientPacket::KeepAlive { time }),
            ))
            .expect("zone A command should execute");
    }
    active_b
        .execute(ZoneOwnerCommandRequest::direct(
            lease_b,
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 3 }),
        ))
        .expect("zone B command should execute");

    let active_a_head = active_a
        .replication_head()
        .expect("zone A replication head");
    let active_b_head = active_b
        .replication_head()
        .expect("zone B replication head");
    assert_eq!(active_a_head.entry_count, 2);
    assert_eq!(active_a_head.next_sequence, 2);
    assert_eq!(active_a_head.last_sequence, Some(1));
    assert_eq!(active_b_head.entry_count, 1);
    assert_eq!(active_b_head.next_sequence, 1);
    assert_eq!(active_b_head.last_sequence, Some(0));
    assert_ne!(active_a_head.latest_digest, active_b_head.latest_digest);
    assert_eq!(
        active_server.replication_head(&zone_a).unwrap(),
        active_a_head
    );
    assert!(
        serde_json::to_vec(&active_a_head).unwrap().len() < 1_024,
        "non-empty replication head must remain smaller than 1 KiB"
    );

    let first_batch = active_a
        .export_mutation_batch(0, 1, 1024 * 1024)
        .expect("first Zone A mutation batch");
    assert_eq!(first_batch.version, ZONE_REPLICATION_HEAD_VERSION);
    assert_eq!(first_batch.zone_id, "map:0");
    assert_eq!(
        first_batch.mutation_coverage,
        ZoneReplicationCoverage::CommandJournal
    );
    assert_eq!(first_batch.first_sequence, 0);
    assert_eq!(first_batch.next_sequence, 1);
    assert_eq!(first_batch.previous_digest, "0".repeat(64));
    assert_eq!(first_batch.entries.len(), 1);
    assert_eq!(first_batch.entries[0].sequence, 0);
    assert!(!first_batch.entries[0].payload.is_empty());
    assert_eq!(first_batch.latest_digest, first_batch.entries[0].digest);
    assert!(first_batch.has_more);
    first_batch
        .verify()
        .expect("untampered mutation batch should verify");
    let mut tampered_batch = first_batch.clone();
    tampered_batch.entries[0].payload.push(0);
    assert!(tampered_batch.verify().is_err());
    let mut tampered_digest = first_batch.clone();
    tampered_digest.entries[0].digest = "f".repeat(64);
    assert!(tampered_digest.verify().is_err());

    let second_batch = active_a
        .export_mutation_batch(1, 8, 1024 * 1024)
        .expect("second Zone A mutation batch");
    assert_eq!(second_batch.first_sequence, 1);
    assert_eq!(second_batch.next_sequence, 2);
    assert_eq!(second_batch.previous_digest, first_batch.latest_digest);
    assert_eq!(second_batch.entries.len(), 1);
    assert_eq!(second_batch.entries[0].sequence, 1);
    assert_eq!(second_batch.latest_digest, active_a_head.latest_digest);
    assert!(!second_batch.has_more);

    let base_snapshot = active_a
        .export_base_snapshot()
        .expect("Zone A base snapshot should export");
    assert_eq!(base_snapshot.version, ZONE_REPLICATION_HEAD_VERSION);
    assert_eq!(base_snapshot.zone_id, "map:0");
    assert_eq!(base_snapshot.build_id, active_a_head.build_id);
    assert_eq!(
        base_snapshot.mutation_coverage,
        ZoneReplicationCoverage::CommandJournal
    );
    assert!(base_snapshot.apply_ready);
    assert_eq!(base_snapshot.base_sequence, 2);
    assert_eq!(base_snapshot.latest_digest, active_a_head.latest_digest);
    assert_eq!(base_snapshot.session_count, 1);
    assert!(base_snapshot.uncompressed_bytes > 0);
    assert!(!base_snapshot.payload.is_empty());
    base_snapshot
        .verify()
        .expect("untampered base snapshot should verify");
    let mut tampered_snapshot = base_snapshot.clone();
    tampered_snapshot.payload[0] ^= 0xff;
    assert!(tampered_snapshot.verify().is_err());

    let wal_path = std::env::temp_dir().join(format!(
        "mir2-gate16-wal-{}-{}.jsonl",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut wal = ZoneMutationWal::open(&wal_path, "map:0", &active_a_head.build_id)
        .expect("new mutation WAL should open");
    assert_eq!(wal.ack().next_sequence, 0);
    let first_ack = wal
        .append_batch(&first_batch)
        .expect("first mutation batch should fsync");
    assert!(first_ack.durable);
    assert_eq!(first_ack.next_sequence, 1);
    let second_ack = wal
        .append_batch(&second_batch)
        .expect("second mutation batch should fsync");
    assert_eq!(second_ack.next_sequence, 2);
    assert_eq!(second_ack.latest_digest, active_a_head.latest_digest);
    assert!(wal.append_batch(&first_batch).is_err());
    drop(wal);

    OpenOptions::new()
        .append(true)
        .open(&wal_path)
        .unwrap()
        .write_all(b"{partial")
        .unwrap();
    let mut repaired_wal = ZoneMutationWal::open(&wal_path, "map:0", &active_a_head.build_id)
        .expect("reopen should discard only a partial final WAL record");
    assert_eq!(repaired_wal.ack(), second_ack);
    let pre_compaction_bytes = fs::metadata(&wal_path).unwrap().len();
    let compacted_ack = repaired_wal
        .compact_to_base(&base_snapshot)
        .expect("durable base should atomically compact the WAL");
    assert_eq!(compacted_ack, second_ack);
    assert!(fs::metadata(&wal_path).unwrap().len() < pre_compaction_bytes);
    drop(repaired_wal);
    let reopened_compacted = ZoneMutationWal::open(&wal_path, "map:0", &active_a_head.build_id)
        .expect("compacted WAL base anchor should survive restart");
    assert_eq!(reopened_compacted.ack(), second_ack);
    drop(reopened_compacted);
    OpenOptions::new()
        .append(true)
        .open(&wal_path)
        .unwrap()
        .write_all(b"{corrupt-complete-record}\n")
        .unwrap();
    assert!(
        ZoneMutationWal::open(&wal_path, "map:0", &active_a_head.build_id)
            .expect_err("a corrupt complete WAL record must fail closed")
            .contains("corrupt complete record")
    );
    fs::remove_file(&wal_path).unwrap();

    let snapshot_path = wal_path.with_extension("base.json");
    let snapshot_store =
        ZoneBaseSnapshotStore::new(&snapshot_path, "map:0", &active_a_head.build_id)
            .expect("base snapshot store should open");
    snapshot_store
        .persist(&base_snapshot)
        .expect("base snapshot should atomically persist");
    assert_eq!(
        snapshot_store
            .load()
            .expect("persisted base snapshot should load"),
        Some(base_snapshot.clone())
    );
    let wrong_identity_store =
        ZoneBaseSnapshotStore::new(&snapshot_path, "map:0", "different-build")
            .expect("identity-bound base snapshot store should open");
    assert!(wrong_identity_store.load().is_err());
    OpenOptions::new()
        .append(true)
        .open(&snapshot_path)
        .unwrap()
        .write_all(b"x")
        .unwrap();
    assert!(snapshot_store.load().is_err());
    fs::remove_file(&snapshot_path).unwrap();

    let caught_up = active_a
        .export_mutation_batch(2, 8, 1024 * 1024)
        .expect("caught-up Zone A mutation batch");
    assert!(caught_up.entries.is_empty());
    assert_eq!(caught_up.first_sequence, 2);
    assert_eq!(caught_up.next_sequence, 2);
    assert_eq!(caught_up.previous_digest, active_a_head.latest_digest);
    assert_eq!(caught_up.latest_digest, active_a_head.latest_digest);
    assert!(!caught_up.has_more);

    let zone_b_batch = active_b
        .export_mutation_batch(0, 8, 1024 * 1024)
        .expect("Zone B mutation batch");
    assert_eq!(zone_b_batch.entries.len(), 1);
    assert_eq!(zone_b_batch.next_sequence, 1);
    assert_eq!(zone_b_batch.latest_digest, active_b_head.latest_digest);
    assert!(active_a
        .export_mutation_batch(3, 8, 1024 * 1024)
        .expect_err("cursor ahead of the Zone head must fail")
        .contains("replication_cursor_ahead"));
    assert!(active_a
        .export_mutation_batch(0, 8, 1)
        .expect_err("an entry larger than the batch byte bound must fail")
        .contains("replication_entry_too_large"));

    let checkpoint = active_a
        .export_host_checkpoint()
        .expect("v4 checkpoint should export");
    standby_a
        .install_host_checkpoint(&checkpoint)
        .expect("v4 checkpoint should rebuild v5 heads");
    assert_eq!(standby_a.replication_head().unwrap(), active_a_head);
    assert_eq!(standby_b.replication_head().unwrap(), active_b_head);

    stop_server(active_address, active_stop, active_handle);
    stop_server(standby_address, standby_stop, standby_handle);
}

#[test]
fn v5_base_snapshot_restores_active_sessions_without_journal_replay_and_compacts_history() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (active_address, _active_server, active_stop, active_handle) =
        start_server(authority.clone());
    let (standby_address, standby_server, standby_stop, standby_handle) =
        start_server(authority.clone());
    let zone_id = ZoneId::new("map:0");
    let other_zone_id = ZoneId::new("map:other");
    let active = test_transport(active_address, zone_id.clone(), "base-session");
    let standby = test_transport(standby_address, zone_id.clone(), "base-session");
    let standby_other = test_transport(standby_address, other_zone_id.clone(), "other-session");
    let lease = authority.owner_lease(&zone_id);
    let other_lease = authority.owner_lease(&other_zone_id);

    standby_other
        .execute(ZoneOwnerCommandRequest::direct(
            other_lease,
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 99 }),
        ))
        .expect("unrelated Zone should have independent history");
    let other_head_before = standby_other.replication_head().unwrap();

    start_new_character(&active, lease.clone(), "base-account", "BasePlayer");
    active
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::TransferMap {
                key: "crystal:0:330:270".to_string(),
            },
        ))
        .expect("active player should move into the Crystal map");
    active
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::Walk {
                direction: MirDirection::Right,
            }),
        ))
        .expect("active player movement should execute");

    let active_identity = active.active_identity().unwrap();
    let active_snapshot = active.world_snapshot().unwrap();
    let active_head = active.replication_head().unwrap();
    let base = active.export_base_snapshot().unwrap();
    assert!(base.apply_ready);
    assert_eq!(base.base_sequence, active_head.next_sequence);
    assert!(base.base_sequence > 0);

    standby
        .install_base_snapshot(&base)
        .expect("standby should install the complete Session base image");
    assert_eq!(standby.active_identity().unwrap(), active_identity);
    let standby_snapshot = standby.world_snapshot().unwrap();
    assert_eq!(
        standby_snapshot.map_file_name,
        active_snapshot.map_file_name
    );
    assert_eq!(standby_snapshot.gold, active_snapshot.gold);
    assert_eq!(
        standby_snapshot.inventory_items,
        active_snapshot.inventory_items
    );
    assert_eq!(
        standby_snapshot.equipment_items,
        active_snapshot.equipment_items
    );
    assert_eq!(standby_snapshot.quest_log, active_snapshot.quest_log);
    assert_eq!(standby_snapshot.known_skills, active_snapshot.known_skills);

    let installed_head = standby.replication_head().unwrap();
    assert_eq!(
        installed_head.base_snapshot_id,
        Some(base.snapshot_id.clone())
    );
    assert_eq!(installed_head.base_sequence, base.base_sequence);
    assert_eq!(installed_head.oldest_available_sequence, base.base_sequence);
    assert_eq!(installed_head.next_sequence, base.base_sequence);
    assert_eq!(installed_head.latest_digest, base.latest_digest);
    assert!(!installed_head.promotion_ready);
    assert!(standby
        .export_mutation_batch(0, 8, 1024 * 1024)
        .expect_err("pre-base history must be compacted")
        .contains("replication_cursor_compacted"));
    let caught_up = standby
        .export_mutation_batch(base.base_sequence, 8, 1024 * 1024)
        .expect("base cursor should be caught up");
    assert!(caught_up.entries.is_empty());
    assert_eq!(caught_up.previous_digest, base.latest_digest);
    assert!(standby
        .export_host_checkpoint()
        .expect_err("v4 export must fail after v5 compaction")
        .contains("checkpoint_history_compacted"));

    assert_eq!(standby_other.replication_head().unwrap(), other_head_before);
    assert_eq!(standby_server.session_count(), 2);

    active
        .execute(ZoneOwnerCommandRequest::direct(
            lease,
            WorldCommand::ClientPacket(ClientPacket::Walk {
                direction: MirDirection::Left,
            }),
        ))
        .expect("active post-base mutation should execute");
    let post_base_head = active.replication_head().unwrap();
    assert_eq!(post_base_head.next_sequence, base.base_sequence + 1);
    let post_base_batch = active
        .export_mutation_batch(base.base_sequence, 8, 1024 * 1024)
        .expect("post-base delta should export");
    assert_eq!(post_base_batch.entries.len(), 1);
    assert_eq!(post_base_batch.entries[0].sequence, base.base_sequence);
    assert_eq!(post_base_batch.previous_digest, base.latest_digest);
    assert_eq!(post_base_batch.latest_digest, post_base_head.latest_digest);
    standby
        .apply_mutation_batch(&post_base_batch)
        .expect("standby should incrementally apply the post-base delta");
    let standby_post_base_head = standby.replication_head().unwrap();
    assert_eq!(
        standby_post_base_head.next_sequence,
        post_base_head.next_sequence
    );
    assert_eq!(
        standby_post_base_head.latest_digest,
        post_base_head.latest_digest
    );
    assert_eq!(
        standby_post_base_head.base_snapshot_id,
        Some(base.snapshot_id)
    );
    assert_eq!(
        standby.world_snapshot().unwrap().entities,
        active.world_snapshot().unwrap().entities
    );

    stop_server(active_address, active_stop, active_handle);
    stop_server(standby_address, standby_stop, standby_handle);
}

#[test]
fn autonomous_zone_ticks_are_ordered_and_incrementally_applied_after_the_base() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (active_address, _active_server, active_stop, active_handle) =
        start_server_with_tick_cadence(authority.clone(), Duration::from_millis(20));
    let (standby_address, _standby_server, standby_stop, standby_handle) =
        start_server(authority.clone());
    let zone_id = ZoneId::new("map:tick");
    let active = test_transport(active_address, zone_id.clone(), "tick-session");
    let standby = test_transport(standby_address, zone_id.clone(), "tick-session");
    let lease = authority.owner_lease(&zone_id);

    let empty_base = active.export_base_snapshot().unwrap();
    assert_eq!(empty_base.base_sequence, 0);
    assert!(empty_base.apply_ready);
    standby.install_base_snapshot(&empty_base).unwrap();
    start_new_character(&active, lease, "tick-account", "TickPlayer");

    let first_tick_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if active.replication_head().unwrap().next_sequence > 4 {
            break;
        }
        assert!(
            Instant::now() < first_tick_deadline,
            "active Zone did not capture an autonomous cadence tick"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let initial_batch = active.export_mutation_batch(0, 256, 1024 * 1024).unwrap();
    standby.apply_mutation_batch(&initial_batch).unwrap();
    let empty_base_catchup_head = standby.replication_head().unwrap();
    thread::sleep(Duration::from_millis(80));
    assert_eq!(
        standby.replication_head().unwrap(),
        empty_base_catchup_head,
        "a Zone created after an empty base install must remain tick-disabled"
    );

    let base = active.export_base_snapshot().unwrap();
    assert!(base.apply_ready);
    standby.install_base_snapshot(&base).unwrap();

    let next_tick_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let head = active.replication_head().unwrap();
        if head.next_sequence > base.base_sequence {
            break;
        }
        assert!(
            Instant::now() < next_tick_deadline,
            "active Zone did not capture a post-base cadence tick"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let batch = active
        .export_mutation_batch(base.base_sequence, 32, 1024 * 1024)
        .unwrap();
    assert!(!batch.entries.is_empty());
    assert!(batch.entries.iter().all(|entry| {
        std::str::from_utf8(&entry.payload).is_ok_and(|payload| payload.contains("\"zoneTickMs\""))
    }));
    standby.apply_mutation_batch(&batch).unwrap();
    let standby_head = standby.replication_head().unwrap();
    assert_eq!(standby_head.next_sequence, batch.next_sequence);
    assert_eq!(standby_head.latest_digest, batch.latest_digest);
    let active_head_after_apply = active.replication_head().unwrap();
    assert!(standby_head.next_sequence <= active_head_after_apply.next_sequence);
    thread::sleep(Duration::from_millis(80));
    assert_eq!(
        standby.replication_head().unwrap(),
        standby_head,
        "replica autonomous ticks must remain disabled between applied batches"
    );

    stop_server(active_address, active_stop, active_handle);
    stop_server(standby_address, standby_stop, standby_handle);
}

#[test]
fn standby_readiness_requires_exact_replica_and_commonware_fence_before_promotion() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let zone_id = ZoneId::new("map:safe-promotion");
    let active_lease = authority.handoff_zone_owner(&zone_id, "active-host");
    let (active_address, active_server, active_stop, active_handle) =
        start_named_server_with_tick_cadence(
            authority.clone(),
            "active-host",
            8,
            Duration::from_millis(20),
        );
    let (standby_address, standby_server, standby_stop, standby_handle) =
        start_named_server_with_tick_cadence(
            authority.clone(),
            "standby-host",
            8,
            Duration::from_millis(20),
        );
    let active = test_transport(active_address, zone_id.clone(), "promotion-session");
    let standby = test_transport(standby_address, zone_id.clone(), "promotion-session");

    start_new_character(
        &active,
        active_lease.clone(),
        "promotion-account",
        "PromotionPlayer",
    );
    let base = active.export_base_snapshot().expect("active base snapshot");
    standby
        .install_base_snapshot(&base)
        .expect("standby installs a restorable base");
    active
        .execute(ZoneOwnerCommandRequest::direct(
            active_lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 16_500 }),
        ))
        .expect("active advances after the standby base");
    let first_quiesce = active
        .quiesce_for_promotion(&active_lease)
        .expect("current fenced owner may quiesce");
    assert_eq!(first_quiesce.owner_id, "active-host");
    assert!(active
        .execute(ZoneOwnerCommandRequest::direct(
            active_lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 16_501 }),
        ))
        .expect_err("quiesced active must reject new player mutations")
        .contains("zone_quiesced"));
    active
        .resume_after_quiesce(&active_lease)
        .expect("handoff may be aborted under the unchanged fence");
    active
        .execute(ZoneOwnerCommandRequest::direct(
            active_lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 16_502 }),
        ))
        .expect("resumed active accepts mutations");
    let quiesce = active
        .quiesce_for_promotion(&active_lease)
        .expect("active quiesces at a final immutable head");
    thread::sleep(Duration::from_millis(60));
    assert_eq!(
        active.replication_head().unwrap().next_sequence,
        quiesce.head.next_sequence,
        "quiesce must stop both player commands and autonomous cadence"
    );

    let now_ms = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    };
    let initial_active_head = active.replication_head().unwrap();
    let initial_readiness = standby
        .assess_promotion_readiness(initial_active_head, now_ms(), 250)
        .expect("behind standby should return a report");
    assert!(!initial_readiness.ready);
    assert_eq!(
        initial_readiness.reason.as_deref(),
        Some("replica_cursor_behind")
    );

    let deadline = Instant::now() + Duration::from_secs(3);
    let readiness = loop {
        let standby_head = standby.replication_head().unwrap();
        let active_head = active.replication_head().unwrap();
        if standby_head.next_sequence < active_head.next_sequence {
            let batch = active
                .export_mutation_batch(standby_head.next_sequence, 512, 1024 * 1024)
                .unwrap();
            standby.apply_mutation_batch(&batch).unwrap();
        }
        let observed_at_ms = now_ms();
        let active_head = active.replication_head().unwrap();
        let report = standby
            .assess_promotion_readiness(active_head, observed_at_ms, 250)
            .unwrap();
        if report.ready {
            break report;
        }
        assert!(
            Instant::now() < deadline,
            "standby did not become promotion-ready: {report:?}"
        );
        thread::sleep(Duration::from_millis(2));
    };
    assert!(readiness.replica_clock_disabled);
    assert!(readiness.capacity_available);
    assert!(readiness.readiness_id.is_some());
    assert!(standby.replication_head().unwrap().promotion_ready);
    assert!(standby
        .execute(ZoneOwnerCommandRequest::direct(
            active_lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 16_503 }),
        ))
        .expect_err("ready standby must freeze its exact promotion image")
        .contains("zone_quiesced"));

    let readiness_id = readiness.readiness_id.clone().unwrap();
    let before_fence = standby
        .promote_replica(readiness_id.clone(), &active_lease)
        .expect_err("readiness alone must not grant ownership");
    assert!(
        before_fence.contains("promotion_owner_mismatch")
            || before_fence.contains("promotion_fence_rejected"),
        "{before_fence}"
    );

    let promoted_lease = authority.handoff_zone_owner(&zone_id, "standby-host");
    let receipt = standby
        .promote_replica(readiness_id.clone(), &promoted_lease)
        .expect("new finalized generation should promote the ready standby");
    assert_eq!(receipt.owner_id, "standby-host");
    assert_eq!(receipt.generation, promoted_lease.fencing_token());
    assert!(!standby.replication_head().unwrap().promotion_ready);
    standby
        .execute(ZoneOwnerCommandRequest::direct(
            promoted_lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 16_504 }),
        ))
        .expect("promotion releases the standby mutation barrier");
    assert!(standby
        .promote_replica(readiness_id, &promoted_lease)
        .expect_err("readiness receipt must be single-use")
        .contains("promotion_receipt_unknown"));

    thread::sleep(Duration::from_millis(80));
    let frozen_active_head = active.replication_head().unwrap();
    let promoted_head = standby.replication_head().unwrap();
    thread::sleep(Duration::from_millis(80));
    assert_eq!(
        active.replication_head().unwrap(),
        frozen_active_head,
        "old active must stop autonomous ticks after losing the fence"
    );
    assert!(
        standby.replication_head().unwrap().next_sequence > promoted_head.next_sequence,
        "promoted standby must begin autonomous Zone cadence"
    );
    assert!(
        standby_server
            .replication_head(&zone_id)
            .unwrap()
            .next_sequence
            > receipt.head.next_sequence
    );
    assert!(
        !active_server
            .replication_head(&zone_id)
            .unwrap()
            .promotion_ready
    );

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
fn multi_endpoint_transport_reroutes_after_old_active_is_logically_fenced() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (active_address, _active_server, active_stop, active_handle) =
        start_named_server(authority.clone(), "active-owner", 8);
    let (standby_address, _standby_server, standby_stop, standby_handle) =
        start_named_server(authority.clone(), "standby-owner", 8);
    let zone_id = ZoneId::new("map:logical-failover");
    let active_lease = authority.handoff_zone_owner(&zone_id, "active-owner");
    let active = test_transport(active_address, zone_id.clone(), "logical-failover-session");
    let standby = test_transport(standby_address, zone_id.clone(), "logical-failover-session");
    start_new_character(
        &active,
        active_lease.clone(),
        "logical-failover",
        "LogicalFailover",
    );
    let checkpoint = active
        .export_host_checkpoint()
        .expect("active checkpoint should export");
    standby
        .install_host_checkpoint(&checkpoint)
        .expect("standby checkpoint should install");

    let failover = TcpZoneOwnerRpcTransport::with_endpoints(
        vec![active_address.to_string(), standby_address.to_string()],
        zone_id.clone(),
        "logical-failover-session",
        None,
        test_limits(),
    )
    .expect("failover transport should accept two endpoints");
    assert!(failover
        .active_identity()
        .expect("active endpoint identity")
        .is_some());

    let promoted = authority.handoff_zone_owner(&zone_id, "standby-owner");
    failover
        .execute(ZoneOwnerCommandRequest::direct(
            promoted,
            WorldCommand::Tick,
        ))
        .expect("stale active response should reroute to the promoted standby");

    stop_server(active_address, active_stop, active_handle);
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
fn tcp_zone_rpc_reuses_one_framed_connection_for_multiple_requests() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (address, server, stop, handle) = start_named_server_with_tick_cadence(
        authority,
        "persistent-rpc-host",
        8,
        Duration::from_secs(60),
    );
    let transport = test_transport(
        address,
        ZoneId::new("map:persistent-rpc"),
        "persistent-rpc-session",
    )
    .with_connection_reuse();

    assert_eq!(
        transport.health().unwrap().protocol_version,
        ZONE_RPC_PROTOCOL_VERSION
    );
    assert_eq!(
        transport.health().unwrap().protocol_version,
        ZONE_RPC_PROTOCOL_VERSION
    );
    let telemetry = server.telemetry_snapshot();
    assert_eq!(telemetry.accepted_connections_total, 1);
    assert_eq!(telemetry.rpc_requests_total, 2);

    drop(transport);
    stop_server(address, stop, handle);
}

#[test]
fn tcp_zone_rpc_keeps_an_idle_reused_connection_alive_past_the_io_timeout() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let limits = ZoneRpcLimits {
        io_timeout: Duration::from_millis(50),
        ..test_limits()
    };
    let (address, server, stop, handle) = start_server_with_limits(authority, limits.clone());
    let transport = TcpZoneOwnerRpcTransport::with_options(
        address.to_string(),
        ZoneId::new("map:idle-persistent-rpc"),
        "idle-persistent-rpc-session",
        None,
        limits,
    )
    .with_connection_reuse();

    assert_eq!(
        transport.health().unwrap().protocol_version,
        ZONE_RPC_PROTOCOL_VERSION
    );
    thread::sleep(Duration::from_millis(200));
    assert_eq!(
        transport.health().unwrap().protocol_version,
        ZONE_RPC_PROTOCOL_VERSION
    );
    let telemetry = server.telemetry_snapshot();
    assert_eq!(telemetry.accepted_connections_total, 1);
    assert_eq!(telemetry.rpc_requests_total, 2);

    drop(transport);
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
    let wal_dir = std::env::temp_dir().join(format!(
        "mir2-zone-replicator-v5-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&wal_dir).unwrap();

    let empty_output = Command::new(env!("CARGO_BIN_EXE_zone_replicator"))
        .arg("--once")
        .env("MIR2_ZONE_ACTIVE_ADDR", active_address.to_string())
        .env("MIR2_ZONE_STANDBY_ADDR", standby_address.to_string())
        .env("MIR2_ZONE_REPLICA_WAL_DIR", &wal_dir)
        .env("MIR2_ZONE_REPLICA_BASE_SNAPSHOT_INTERVAL_ENTRIES", "1")
        .env_remove("MIR2_ZONE_HOST_TOKEN")
        .output()
        .expect("empty-base zone replicator process should spawn");
    assert!(
        empty_output.status.success(),
        "empty-base zone replicator failed: {}",
        String::from_utf8_lossy(&empty_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&empty_output.stderr).contains("installed v5 base"),
        "empty cursor must still install the v5 base identity: {}",
        String::from_utf8_lossy(&empty_output.stderr)
    );
    assert_eq!(standby.replication_head().unwrap().next_sequence, 0);
    assert!(standby
        .replication_head()
        .unwrap()
        .base_snapshot_id
        .is_some());

    start_demo_character(&active, mir2_gateway::ZoneOwnerLease::in_process(&zone_id));
    let output = Command::new(env!("CARGO_BIN_EXE_zone_replicator"))
        .arg("--once")
        .env("MIR2_ZONE_ACTIVE_ADDR", active_address.to_string())
        .env("MIR2_ZONE_STANDBY_ADDR", standby_address.to_string())
        .env("MIR2_ZONE_REPLICA_WAL_DIR", &wal_dir)
        .env("MIR2_ZONE_REPLICA_BASE_SNAPSHOT_INTERVAL_ENTRIES", "1")
        .env_remove("MIR2_ZONE_HOST_TOKEN")
        .output()
        .expect("zone replicator process should spawn");
    assert!(
        output.status.success(),
        "zone replicator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("installed v5 base"),
        "zone replicator did not use the v5 base path: {}",
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

    let recovery_reservation = TcpListener::bind("127.0.0.1:0").expect("reserve recovery address");
    let recovery_address = recovery_reservation.local_addr().expect("recovery address");
    drop(recovery_reservation);
    let mut recovery_child = spawn_zone_host_process(recovery_address);
    let _recovery_guard = ChildGuard(&mut recovery_child);
    let recovery = test_transport(recovery_address, zone_id.clone(), "process-replica-session");
    wait_for_health(&recovery, Duration::from_secs(5));
    let reverse_wal_dir = wal_dir.with_extension("reverse");
    fs::create_dir_all(&reverse_wal_dir).unwrap();
    let reverse_output = Command::new(env!("CARGO_BIN_EXE_zone_replicator"))
        .arg("--once")
        .env("MIR2_ZONE_ACTIVE_ADDR", standby_address.to_string())
        .env("MIR2_ZONE_STANDBY_ADDR", recovery_address.to_string())
        .env("MIR2_ZONE_REPLICA_WAL_DIR", &reverse_wal_dir)
        .env("MIR2_ZONE_REPLICA_BASE_SNAPSHOT_INTERVAL_ENTRIES", "1")
        .env_remove("MIR2_ZONE_HOST_TOKEN")
        .output()
        .expect("reverse zone replicator process should spawn");
    let reverse_stderr = String::from_utf8_lossy(&reverse_output.stderr);
    assert!(
        reverse_output.status.success(),
        "reverse zone replicator failed: {reverse_stderr}"
    );
    assert!(
        reverse_stderr.contains("bootstrapped compacted active history from v5 base"),
        "reverse replicator did not bridge the active compacted prefix: {reverse_stderr}"
    );
    assert!(
        reverse_stderr.contains("installed v5 base"),
        "reverse replicator did not install the v5 base: {reverse_stderr}"
    );
    assert_eq!(
        recovery
            .active_identity()
            .expect("recovery identity should respond")
            .expect("reverse replicated process should restore active character")
            .account_id,
        "demo"
    );
    fs::remove_dir_all(reverse_wal_dir).unwrap();
    fs::remove_dir_all(wal_dir).unwrap();
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
    start_server_with_limits(authority, test_limits())
}

fn start_server_with_limits(
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
    limits: ZoneRpcLimits,
) -> (
    SocketAddr,
    Arc<ZoneHostServer>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test zone host");
    let address = listener.local_addr().expect("test zone host address");
    let stop = Arc::new(AtomicBool::new(false));
    let shared: SharedZoneOwnerLeaseAuthority = authority;
    let server = Arc::new(ZoneHostServer::with_options_and_factory(
        GatewayConfig::default(),
        shared,
        None,
        limits,
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(60 * 60),
            BTreeMap::new(),
        )),
    ));
    let running_server = Arc::clone(&server);
    let server_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        running_server
            .serve_until(listener, server_stop)
            .expect("test zone host should run");
    });
    (address, server, stop, handle)
}

fn start_server_with_tick_cadence(
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
    cadence: Duration,
) -> (
    SocketAddr,
    Arc<ZoneHostServer>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind tick test zone host");
    let address = listener.local_addr().expect("tick test zone host address");
    let stop = Arc::new(AtomicBool::new(false));
    let shared: SharedZoneOwnerLeaseAuthority = authority;
    let server = Arc::new(ZoneHostServer::with_options_and_factory(
        GatewayConfig::default(),
        shared,
        None,
        test_limits(),
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            cadence,
            BTreeMap::new(),
        )),
    ));
    let running_server = Arc::clone(&server);
    let server_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        running_server
            .serve_until(listener, server_stop)
            .expect("tick test zone host should run");
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
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(60 * 60),
            BTreeMap::new(),
        )),
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

fn start_named_server_with_tick_cadence(
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
    host_id: &str,
    zone_capacity: usize,
    cadence: Duration,
) -> (
    SocketAddr,
    Arc<ZoneHostServer>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind named tick test Zone Host");
    let address = listener.local_addr().expect("named tick Zone Host address");
    let stop = Arc::new(AtomicBool::new(false));
    let shared: SharedZoneOwnerLeaseAuthority = authority;
    let server = Arc::new(ZoneHostServer::with_identity_and_factory(
        host_id,
        zone_capacity,
        GatewayConfig::default(),
        shared,
        None,
        test_limits(),
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            cadence,
            BTreeMap::new(),
        )),
    ));
    let running_server = Arc::clone(&server);
    let server_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        running_server
            .serve_until(listener, server_stop)
            .expect("named tick test Zone Host should run");
    });
    (address, server, stop, handle)
}

fn test_server(authority: Arc<InMemoryZoneOwnerLeaseAuthority>) -> Arc<ZoneHostServer> {
    let shared: SharedZoneOwnerLeaseAuthority = authority;
    Arc::new(ZoneHostServer::with_options_and_factory(
        GatewayConfig::default(),
        shared,
        None,
        test_limits(),
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(60 * 60),
            BTreeMap::new(),
        )),
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

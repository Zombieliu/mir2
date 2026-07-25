use std::collections::{BTreeMap, BTreeSet};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::{
    decode_zone_rpc_routing_hint, CapacityChallenge, CapacityChallengeResponse, CapacityWorkload,
    FinalizedGuildNodeRegistration, GatewayConfig, GuildNodeStatus, HomeTunnelAgent,
    HomeTunnelAgentConfig, HomeTunnelPlacement, HomeTunnelRelay, HomeTunnelRelayConfig,
    HomeTunnelTlsMaterial, InMemoryZoneOwnerLeaseAuthority, NodeCapacityCertificate,
    NodeSigningIdentity, SharedInProcessZoneRuntimeFactory, SharedZoneOwnerLeaseAuthority,
    SuiFinalityProof, TcpZoneOwnerRpcTransport, ZoneHostServer, ZoneId, ZoneOwnerCommandRequest,
    ZoneOwnerLeaseAuthority, ZoneOwnerRpcTransport, ZoneRpcLimits, ZoneRpcRoutingHint,
    ZONE_RPC_PROTOCOL_VERSION,
};
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::WorldCommand;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use tokio::sync::watch;

#[test]
fn relay_routing_hint_accepts_json_and_named_message_pack() {
    let expected = ZoneRpcRoutingHint {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        session_id: "session-codec-a".to_string(),
        zone_id: "map:0".to_string(),
    };
    let json = serde_json::to_vec(&expected).unwrap();
    let message_pack = rmp_serde::to_vec_named(&expected).unwrap();
    assert_eq!(decode_zone_rpc_routing_hint(&json).unwrap(), expected);
    assert_eq!(
        decode_zone_rpc_routing_hint(&message_pack).unwrap(),
        expected
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_mir2_session_crosses_outbound_mtls_quic_and_survives_udp_rebind() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let (zone_address, zone_stop, zone_handle) = start_zone_host(authority.clone());
    let (relay_tls, agent_tls) = tls_materials();
    let node = NodeSigningIdentity::from_seed([31; 32]);
    let relay_identity = NodeSigningIdentity::from_seed([32; 32]);
    let control_identity = NodeSigningIdentity::from_seed([33; 32]);
    let capacity_issuer = NodeSigningIdentity::from_seed([34; 32]);
    let certificate = capacity_certificate(&node, &capacity_issuer);
    let now = now_ms();
    let placement = HomeTunnelPlacement::issue(
        "home-placement-primary",
        "relay-test-a",
        "primary",
        node.node_id(),
        1,
        1,
        4,
        101,
        now.saturating_sub(100),
        now.saturating_add(60_000),
        &control_identity,
    )
    .expect("test placement should sign");
    let mut relay_config = HomeTunnelRelayConfig::with_defaults(
        "relay-test-a",
        "127.0.0.1:0".parse().unwrap(),
        "127.0.0.1:0".parse().unwrap(),
        relay_tls,
        relay_identity.clone(),
        capacity_issuer.public_key(),
        control_identity.public_key(),
        vec![placement],
    );
    relay_config.max_agent_connections = 1;
    let relay = HomeTunnelRelay::bind(relay_config)
        .await
        .expect("Relay should bind");
    let relay_address = relay.quic_addr().expect("Relay QUIC address");
    let gateway_address = relay.gateway_addr().expect("Relay gateway address");
    let (relay_shutdown_tx, relay_shutdown_rx) = watch::channel(false);
    let relay_task = tokio::spawn(relay.serve(relay_shutdown_rx));

    let agent = HomeTunnelAgent::connect(HomeTunnelAgentConfig::with_defaults(
        "relay-test-a",
        relay_address,
        "relay.test",
        zone_address,
        agent_tls.clone(),
        node,
        1,
        "home-agent-test-a",
        1,
        certificate,
        relay_identity.public_key(),
        control_identity.public_key(),
    ))
    .await
    .expect("outbound-only Home Agent should register over mTLS QUIC");
    let network = agent.network_handle();
    let second_connection = HomeTunnelAgent::connect(HomeTunnelAgentConfig::with_defaults(
        "relay-test-a",
        relay_address,
        "relay.test",
        zone_address,
        agent_tls,
        NodeSigningIdentity::from_seed([31; 32]),
        1,
        "home-agent-test-a-second",
        2,
        capacity_certificate(&NodeSigningIdentity::from_seed([31; 32]), &capacity_issuer),
        relay_identity.public_key(),
        control_identity.public_key(),
    ))
    .await;
    assert!(
        second_connection.is_err(),
        "Relay must fail closed when the concurrent Agent connection budget is exhausted"
    );
    let (agent_shutdown_tx, agent_shutdown_rx) = watch::channel(false);
    let agent_task = tokio::spawn(agent.serve(agent_shutdown_rx));

    let transport = TcpZoneOwnerRpcTransport::with_options(
        gateway_address.to_string(),
        ZoneId::primary(),
        "home-session-a",
        None,
        ZoneRpcLimits {
            io_timeout: Duration::from_secs(5),
            ..ZoneRpcLimits::default()
        },
    );
    wait_for_health(&transport).await;
    let lease = authority.owner_lease(&ZoneId::primary());
    let login = transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }),
        ))
        .expect("real Mir2 login should cross Home Tunnel");
    assert!(login
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease.clone(),
            WorldCommand::ClientPacket(ClientPacket::StartGame { character_index: 0 }),
        ))
        .expect("real Mir2 StartGame should cross Home Tunnel");
    assert_eq!(
        transport
            .active_identity()
            .expect("identity RPC should cross Home Tunnel")
            .expect("player should be active")
            .account_id,
        "demo"
    );

    let rebound = UdpSocket::bind("127.0.0.1:0").expect("bind replacement UDP socket");
    network
        .rebind(rebound)
        .expect("QUIC connection should survive a NAT-style UDP port rebind");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let keep_alive = transport
        .execute(ZoneOwnerCommandRequest::direct(
            lease,
            WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 4242 }),
        ))
        .expect("Mir2 Session should continue after UDP rebind");
    assert!(keep_alive
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::KeepAlive { time: 4242 })));

    agent_shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), agent_task)
        .await
        .expect("Agent shutdown should be bounded")
        .expect("Agent task should join")
        .expect("Agent should stop cleanly");
    relay_shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), relay_task)
        .await
        .expect("Relay shutdown should be bounded")
        .expect("Relay task should join")
        .expect("Relay should stop cleanly");
    stop_zone_host(zone_address, zone_stop, zone_handle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relay_rejects_agent_certificate_from_untrusted_ca() {
    let (relay_tls, _) = tls_materials();
    let rogue_agent_tls = rogue_agent_tls(&relay_tls.ca_certificate_der);
    let node = NodeSigningIdentity::from_seed([51; 32]);
    let relay_identity = NodeSigningIdentity::from_seed([52; 32]);
    let control_identity = NodeSigningIdentity::from_seed([53; 32]);
    let capacity_issuer = NodeSigningIdentity::from_seed([54; 32]);
    let now = now_ms();
    let placement = HomeTunnelPlacement::issue(
        "home-placement-untrusted-client",
        "relay-test-untrusted-client",
        "primary",
        node.node_id(),
        1,
        1,
        4,
        202,
        now.saturating_sub(100),
        now.saturating_add(60_000),
        &control_identity,
    )
    .unwrap();
    let relay = HomeTunnelRelay::bind(HomeTunnelRelayConfig::with_defaults(
        "relay-test-untrusted-client",
        "127.0.0.1:0".parse().unwrap(),
        "127.0.0.1:0".parse().unwrap(),
        relay_tls,
        relay_identity.clone(),
        capacity_issuer.public_key(),
        control_identity.public_key(),
        vec![placement],
    ))
    .await
    .unwrap();
    let relay_address = relay.quic_addr().unwrap();
    let (relay_shutdown_tx, relay_shutdown_rx) = watch::channel(false);
    let relay_task = tokio::spawn(relay.serve(relay_shutdown_rx));

    let error = HomeTunnelAgent::connect(HomeTunnelAgentConfig::with_defaults(
        "relay-test-untrusted-client",
        relay_address,
        "relay.test",
        "127.0.0.1:9".parse().unwrap(),
        rogue_agent_tls,
        node.clone(),
        1,
        "home-agent-untrusted-client",
        1,
        capacity_certificate(&node, &capacity_issuer),
        relay_identity.public_key(),
        control_identity.public_key(),
    ))
    .await
    .expect_err("mTLS must reject a client certificate from an untrusted CA");
    assert!(
        error.contains("connect Home Tunnel Relay")
            || error.contains("Home Tunnel Relay closed before challenge")
            || error.contains("accept Home Tunnel registration stream"),
        "unexpected mTLS rejection error: {error}"
    );

    relay_shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), relay_task)
        .await
        .expect("Relay shutdown should be bounded")
        .expect("Relay task should join")
        .expect("Relay should stop cleanly");
}

fn capacity_certificate(
    node: &NodeSigningIdentity,
    issuer: &NodeSigningIdentity,
) -> NodeCapacityCertificate {
    let now = now_ms();
    let registration = FinalizedGuildNodeRegistration {
        node_id: node.node_id().to_string(),
        operator_sui_address: format!("0x{}", "11".repeat(32)),
        public_key: node.public_key().to_string(),
        endpoint: "outbound-only".to_string(),
        failure_domain: "home-test-isp".to_string(),
        stake_mist: 1_000_000,
        max_sessions: 8,
        max_zones: 2,
        key_generation: 1,
        status: GuildNodeStatus::Active,
        finality: SuiFinalityProof {
            network: "testnet".to_string(),
            package_id: format!("0x{}", "22".repeat(32)),
            transaction_digest: "home-tunnel-integration".to_string(),
            event_sequence: 0,
            checkpoint: 42,
        },
    };
    let challenge = CapacityChallenge {
        challenge_id: "home-tunnel-integration-capacity".to_string(),
        node_id: node.node_id().to_string(),
        nonce: URL_SAFE_NO_PAD.encode([41; 32]),
        issued_at_ms: now.saturating_sub(1_000),
        expires_at_ms: now.saturating_add(60_000),
        workload: CapacityWorkload {
            concurrent_sessions: 8,
            max_sessions_per_zone: 4,
            zone_count: 2,
            command_count: 100,
            maximum_p95_latency_ms: 200,
            minimum_success_bps: 9_900,
        },
    };
    let response =
        CapacityChallengeResponse::sign(challenge, node, 1, 100, 0, 40, "ab".repeat(32), now)
            .unwrap();
    NodeCapacityCertificate::issue(&response, &registration, issuer, now, 600_000, 7).unwrap()
}

fn tls_materials() -> (HomeTunnelTlsMaterial, HomeTunnelTlsMaterial) {
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    let ca_key = KeyPair::generate().unwrap();
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let server = leaf(
        "relay.test",
        ExtendedKeyUsagePurpose::ServerAuth,
        &ca,
        &ca_key,
    );
    let client = leaf(
        "agent.test",
        ExtendedKeyUsagePurpose::ClientAuth,
        &ca,
        &ca_key,
    );
    let ca_der = ca.der().to_vec();
    (
        HomeTunnelTlsMaterial {
            ca_certificate_der: ca_der.clone(),
            certificate_chain_der: vec![server.0.der().to_vec()],
            private_key_pkcs8_der: server.1.serialize_der(),
        },
        HomeTunnelTlsMaterial {
            ca_certificate_der: ca_der,
            certificate_chain_der: vec![client.0.der().to_vec()],
            private_key_pkcs8_der: client.1.serialize_der(),
        },
    )
}

fn rogue_agent_tls(trusted_server_ca_der: &[u8]) -> HomeTunnelTlsMaterial {
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    let rogue_ca_key = KeyPair::generate().unwrap();
    let rogue_ca = ca_params.self_signed(&rogue_ca_key).unwrap();
    let client = leaf(
        "rogue-agent.test",
        ExtendedKeyUsagePurpose::ClientAuth,
        &rogue_ca,
        &rogue_ca_key,
    );
    HomeTunnelTlsMaterial {
        ca_certificate_der: trusted_server_ca_der.to_vec(),
        certificate_chain_der: vec![client.0.der().to_vec()],
        private_key_pkcs8_der: client.1.serialize_der(),
    }
}

fn leaf(
    name: &str,
    usage: ExtendedKeyUsagePurpose,
    ca: &Certificate,
    ca_key: &KeyPair,
) -> (Certificate, KeyPair) {
    let mut params = CertificateParams::new(vec![name.to_string()]).unwrap();
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![usage];
    let key = KeyPair::generate().unwrap();
    let certificate = params.signed_by(&key, ca, ca_key).unwrap();
    (certificate, key)
}

fn start_zone_host(
    authority: Arc<InMemoryZoneOwnerLeaseAuthority>,
) -> (SocketAddr, Arc<AtomicBool>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let shared: SharedZoneOwnerLeaseAuthority = authority;
    let server = Arc::new(ZoneHostServer::with_options_and_factory(
        GatewayConfig::default(),
        shared,
        None,
        ZoneRpcLimits::default(),
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(60 * 60),
            BTreeMap::new(),
        )),
    ));
    server.configure_zone_map_catalog(BTreeMap::new(), BTreeSet::from(["primary".to_string()]));
    let running = Arc::clone(&server);
    let running_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        running.serve_until(listener, running_stop).unwrap();
    });
    (address, stop, handle)
}

async fn wait_for_health(transport: &TcpZoneOwnerRpcTransport) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if transport.health().is_ok() {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Home Tunnel never routed a Zone Host health request"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn stop_zone_host(address: SocketAddr, stop: Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
    stop.store(true, Ordering::Release);
    let _ = TcpStream::connect(address);
    handle.join().unwrap();
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

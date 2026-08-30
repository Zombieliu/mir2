use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::net::{SocketAddr, TcpListener, TcpStream as StdTcpStream, UdpSocket};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::tcp::{chat_broadcast::ChatBroadcastHub, serve_tcp_gateway};
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
use mir2_protocol::{decode_server_packet, encode_client_packet, ClientPacket, ServerPacket};
use mir2_simulation::WorldCommand;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
async fn public_player_gateway_crosses_outbound_mtls_quic_and_survives_udp_rebind() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let gateway_token = "home-relay-official-gateway-test-token";
    let local_zone_token = "home-node-local-zone-test-token";
    let (zone_address, zone_stop, zone_handle) =
        start_zone_host(authority.clone(), Some(local_zone_token.to_string()));
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
    let placements_path = temporary_json_path("home-tunnel-placements");
    write_json_atomically(&placements_path, &vec![placement.clone()]);
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
    relay_config.placements_file = Some(placements_path.clone());
    relay_config.max_agent_connections = 1;
    relay_config.gateway_auth_token = Some(gateway_token.to_string());
    relay_config.io_timeout = Duration::from_secs(3);
    let relay = HomeTunnelRelay::bind(relay_config)
        .await
        .expect("Relay should bind");
    let relay_address = relay.quic_addr().expect("Relay QUIC address");
    let gateway_address = relay.gateway_addr().expect("Relay gateway address");
    let (relay_shutdown_tx, relay_shutdown_rx) = watch::channel(false);
    let relay_task = tokio::spawn(relay.serve(relay_shutdown_rx));

    let mut agent_config = HomeTunnelAgentConfig::with_defaults(
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
    );
    agent_config.local_zone_rpc_auth_token = Some(local_zone_token.to_string());
    agent_config.io_timeout = Duration::from_secs(2);
    let agent = HomeTunnelAgent::connect(agent_config)
        .await
        .expect("outbound-only Home Agent should register over mTLS QUIC");
    let network = agent.network_handle();
    let mut second_agent_config = HomeTunnelAgentConfig::with_defaults(
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
    );
    second_agent_config.local_zone_rpc_auth_token = Some(local_zone_token.to_string());
    let second_connection = HomeTunnelAgent::connect(second_agent_config).await;
    assert!(
        second_connection.is_err(),
        "Relay must fail closed when the concurrent Agent connection budget is exhausted"
    );
    let (agent_shutdown_tx, agent_shutdown_rx) = watch::channel(false);
    let agent_task = tokio::spawn(agent.serve(agent_shutdown_rx));
    let replacement = HomeTunnelPlacement::issue(
        "home-placement-primary-generation-2",
        "relay-test-a",
        "primary",
        NodeSigningIdentity::from_seed([31; 32]).node_id(),
        1,
        2,
        4,
        102,
        now.saturating_sub(100),
        now.saturating_add(60_000),
        &control_identity,
    )
    .expect("replacement placement should sign");
    write_json_atomically(&placements_path, &vec![replacement]);

    let unauthorized = TcpZoneOwnerRpcTransport::with_options(
        gateway_address.to_string(),
        ZoneId::primary(),
        "home-session-unauthorized",
        Some("wrong-gateway-token".to_string()),
        ZoneRpcLimits {
            io_timeout: Duration::from_secs(2),
            ..ZoneRpcLimits::default()
        },
    );
    assert!(
        unauthorized.health().is_err(),
        "Relay must reject a Zone RPC frame without the official Gateway credential"
    );
    let leaked_gateway_credential = TcpZoneOwnerRpcTransport::with_options(
        zone_address.to_string(),
        ZoneId::primary(),
        "home-session-leaked-gateway-token",
        Some(gateway_token.to_string()),
        ZoneRpcLimits {
            io_timeout: Duration::from_secs(2),
            ..ZoneRpcLimits::default()
        },
    );
    assert!(
        leaked_gateway_credential.health().is_err(),
        "official Gateway credential must not authorize the node-local Zone Host"
    );

    let transport = TcpZoneOwnerRpcTransport::with_options(
        gateway_address.to_string(),
        ZoneId::primary(),
        "home-session-a",
        Some(gateway_token.to_string()),
        ZoneRpcLimits {
            io_timeout: Duration::from_secs(5),
            ..ZoneRpcLimits::default()
        },
    );
    wait_for_health(&transport).await;
    transport
        .on_connect()
        .expect("direct on_connect probe should cross Home Tunnel");
    let probe_lease = authority.owner_lease(&ZoneId::primary());
    let probe_login = transport
        .execute(ZoneOwnerCommandRequest::direct(
            probe_lease,
            WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }),
        ))
        .expect("direct Mir2 login probe should cross Home Tunnel");
    assert!(probe_login
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));

    // The real client never connects to the Home Node or Relay. It connects to
    // the official Mir2 Gateway, whose Zone RPC transport targets the Relay.
    unsafe {
        std::env::set_var("MIR2_ZONE_HOST_ADDR", gateway_address.to_string());
        std::env::set_var("MIR2_ZONE_HOST_TOKEN", gateway_token);
    }
    let player_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("public player Gateway listener should bind");
    let player_gateway_address = player_listener.local_addr().unwrap();
    let chat_hub = ChatBroadcastHub::from_env().unwrap();
    let player_gateway_task = tokio::spawn(serve_tcp_gateway(
        player_listener,
        GatewayConfig::default(),
        chat_hub,
    ));
    let mut player = tokio::net::TcpStream::connect(player_gateway_address)
        .await
        .expect("real Mir2 player should connect to official Gateway");
    let _connected = read_player_packet(&mut player).await;
    send_player_packet(
        &mut player,
        &ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        },
    )
    .await;
    read_player_until(&mut player, |packet| {
        matches!(packet, ServerPacket::LoginSuccess { .. })
    })
    .await;
    send_player_packet(&mut player, &ClientPacket::StartGame { character_index: 0 }).await;
    read_player_until(&mut player, |packet| {
        matches!(packet, ServerPacket::StartGame { .. })
    })
    .await;

    let rebound = UdpSocket::bind("127.0.0.1:0").expect("bind replacement UDP socket");
    network
        .rebind(rebound)
        .expect("QUIC connection should survive a NAT-style UDP port rebind");
    tokio::time::sleep(Duration::from_millis(50)).await;
    send_player_packet(&mut player, &ClientPacket::KeepAlive { time: 4242 }).await;
    read_player_until(&mut player, |packet| {
        matches!(packet, ServerPacket::KeepAlive { time: 4242 })
    })
    .await;

    drop(player);
    tokio::time::sleep(Duration::from_millis(100)).await;
    player_gateway_task.abort();
    let _ = player_gateway_task.await;
    unsafe {
        std::env::remove_var("MIR2_ZONE_HOST_ADDR");
        std::env::remove_var("MIR2_ZONE_HOST_TOKEN");
    }
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
    let _ = std::fs::remove_file(placements_path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn home_agent_process_reconnects_after_relay_restart() {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let local_zone_token = "home-node-reconnect-local-zone-token";
    let (zone_address, zone_stop, zone_handle) =
        start_zone_host(authority, Some(local_zone_token.to_string()));
    let (relay_tls, agent_tls) = tls_materials();
    let node_seed = [61_u8; 32];
    let node = NodeSigningIdentity::from_seed(node_seed);
    let relay_identity = NodeSigningIdentity::from_seed([62; 32]);
    let control_identity = NodeSigningIdentity::from_seed([63; 32]);
    let capacity_issuer = NodeSigningIdentity::from_seed([64; 32]);
    let certificate = capacity_certificate(&node, &capacity_issuer);
    let now = now_ms();
    let placement = HomeTunnelPlacement::issue(
        "home-placement-reconnect",
        "relay-test-reconnect",
        "primary",
        node.node_id(),
        1,
        1,
        4,
        303,
        now.saturating_sub(100),
        now.saturating_add(60_000),
        &control_identity,
    )
    .unwrap();
    let relay_config = |quic_bind, gateway_bind| {
        let mut config = HomeTunnelRelayConfig::with_defaults(
            "relay-test-reconnect",
            quic_bind,
            gateway_bind,
            relay_tls.clone(),
            relay_identity.clone(),
            capacity_issuer.public_key(),
            control_identity.public_key(),
            vec![placement.clone()],
        );
        config.io_timeout = Duration::from_secs(2);
        config
    };
    let relay = HomeTunnelRelay::bind(relay_config(
        "127.0.0.1:0".parse().unwrap(),
        "127.0.0.1:0".parse().unwrap(),
    ))
    .await
    .unwrap();
    let relay_address = relay.quic_addr().unwrap();
    let gateway_address = relay.gateway_addr().unwrap();
    let (first_relay_shutdown_tx, first_relay_shutdown_rx) = watch::channel(false);
    let first_relay_task = tokio::spawn(relay.serve(first_relay_shutdown_rx));

    let fixture_dir = std::env::temp_dir().join(format!(
        "mir2-home-agent-reconnect-{}-{}",
        std::process::id(),
        now_ms()
    ));
    fs::create_dir_all(&fixture_dir).unwrap();
    let ca_path = fixture_dir.join("ca.der");
    let certificate_path = fixture_dir.join("agent.der");
    let private_key_path = fixture_dir.join("agent-key.der");
    let capacity_path = fixture_dir.join("capacity.json");
    let status_path = fixture_dir.join("status.json");
    fs::write(&ca_path, &agent_tls.ca_certificate_der).unwrap();
    fs::write(&certificate_path, &agent_tls.certificate_chain_der[0]).unwrap();
    fs::write(&private_key_path, &agent_tls.private_key_pkcs8_der).unwrap();
    fs::write(&capacity_path, serde_json::to_vec(&certificate).unwrap()).unwrap();

    let mut agent = Command::new(env!("CARGO_BIN_EXE_home_agent"))
        .env("MIR2_HOME_RELAY_ID", "relay-test-reconnect")
        .env("MIR2_HOME_RELAY_ADDR", relay_address.to_string())
        .env("MIR2_HOME_RELAY_SERVER_NAME", "relay.test")
        .env("MIR2_HOME_LOCAL_ZONE_RPC_ADDR", zone_address.to_string())
        .env("MIR2_HOME_LOCAL_ZONE_RPC_TOKEN", local_zone_token)
        .env("MIR2_HOME_AGENT_TLS_CA_DER", &ca_path)
        .env("MIR2_HOME_AGENT_TLS_CERT_CHAIN_DER", &certificate_path)
        .env("MIR2_HOME_AGENT_TLS_KEY_DER", &private_key_path)
        .env(
            "MIR2_HOME_AGENT_SIGNING_KEY",
            URL_SAFE_NO_PAD.encode(node_seed),
        )
        .env("MIR2_HOME_AGENT_KEY_GENERATION", "1")
        .env("MIR2_HOME_CAPACITY_CERTIFICATE_FILE", &capacity_path)
        .env("MIR2_HOME_RELAY_PUBLIC_KEY", relay_identity.public_key())
        .env(
            "MIR2_HOME_CONTROL_ISSUER_PUBLIC_KEY",
            control_identity.public_key(),
        )
        .env("MIR2_HOME_AGENT_STATUS_FILE", &status_path)
        .env_remove("MIR2_HOME_TELEMETRY_URL")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Home Agent process should start");

    wait_for_agent_relay_status(&status_path, &mut agent, true).await;
    first_relay_shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), first_relay_task)
        .await
        .expect("first Relay shutdown should be bounded")
        .expect("first Relay task should join")
        .expect("first Relay should stop cleanly");
    wait_for_agent_relay_status(&status_path, &mut agent, false).await;
    assert!(
        agent.try_wait().unwrap().is_none(),
        "Home Agent must stay alive while Relay is unavailable"
    );

    let rebind_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let relay = loop {
        match HomeTunnelRelay::bind(relay_config(relay_address, gateway_address)).await {
            Ok(relay) => break relay,
            Err(error)
                if (error.contains("Address already in use")
                    || error.contains("os error 10048")
                    || error.contains("os error 98"))
                    && tokio::time::Instant::now() < rebind_deadline =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => panic!("Relay should rebind its previous addresses: {error}"),
        }
    };
    let (second_relay_shutdown_tx, second_relay_shutdown_rx) = watch::channel(false);
    let second_relay_task = tokio::spawn(relay.serve(second_relay_shutdown_rx));
    wait_for_agent_relay_status(&status_path, &mut agent, true).await;

    agent.kill().expect("Home Agent process should stop");
    let _ = agent.wait();
    second_relay_shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), second_relay_task)
        .await
        .expect("second Relay shutdown should be bounded")
        .expect("second Relay task should join")
        .expect("second Relay should stop cleanly");
    stop_zone_host(zone_address, zone_stop, zone_handle);
    for path in [
        ca_path,
        certificate_path,
        private_key_path,
        capacity_path,
        status_path,
    ] {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_dir(fixture_dir);
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
    auth_token: Option<String>,
) -> (SocketAddr, Arc<AtomicBool>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let shared: SharedZoneOwnerLeaseAuthority = authority;
    let server = Arc::new(ZoneHostServer::with_options_and_factory(
        GatewayConfig::default(),
        shared,
        auth_token,
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

async fn wait_for_agent_relay_status(
    path: &std::path::Path,
    child: &mut std::process::Child,
    expected: bool,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        assert!(
            child.try_wait().unwrap().is_none(),
            "Home Agent exited before relayConnected={expected}"
        );
        if fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|status| status["relayConnected"].as_bool())
            == Some(expected)
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Home Agent never reported relayConnected={expected}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn stop_zone_host(address: SocketAddr, stop: Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
    stop.store(true, Ordering::Release);
    let _ = StdTcpStream::connect(address);
    handle.join().unwrap();
}

async fn send_player_packet(stream: &mut tokio::net::TcpStream, packet: &ClientPacket) {
    stream
        .write_all(&encode_client_packet(packet).expect("client packet should encode"))
        .await
        .expect("client packet should write");
}

async fn read_player_packet(stream: &mut tokio::net::TcpStream) -> ServerPacket {
    let mut header = [0_u8; 2];
    tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut header))
        .await
        .expect("player response header should arrive")
        .expect("player response header should read");
    let length = u16::from_le_bytes(header) as usize;
    assert!(length >= 4, "player response frame length should be valid");
    let mut frame = vec![0_u8; length];
    frame[..2].copy_from_slice(&header);
    tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut frame[2..]))
        .await
        .expect("player response body should arrive")
        .expect("player response body should read");
    decode_server_packet(&frame).expect("server packet should decode")
}

async fn read_player_until(
    stream: &mut tokio::net::TcpStream,
    expected: impl Fn(&ServerPacket) -> bool,
) -> Vec<ServerPacket> {
    let mut packets = Vec::new();
    loop {
        let packet = read_player_packet(stream).await;
        let done = expected(&packet);
        packets.push(packet);
        if done {
            return packets;
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn temporary_json_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mir2-{label}-{}-{}.json",
        std::process::id(),
        now_ms()
    ))
}

fn write_json_atomically(path: &std::path::Path, value: &impl serde::Serialize) {
    let staging = path.with_extension("json.next");
    std::fs::write(
        &staging,
        serde_json::to_vec_pretty(value).expect("temporary JSON should encode"),
    )
    .expect("temporary JSON should write");
    std::fs::rename(&staging, path).expect("temporary JSON should publish atomically");
}

//! Real loopback transport checks for immutable owner pins and live poll clones.
//! These tests use explicit constructor arguments and never mutate process env.

use super::*;
use crate::zone_lease::ObservedZoneLease;
use crate::zone_owner_resolver::{
    HostRegistry, PostgresObserverLimits, PostgresZoneLeaseObserver, ResolveError,
    StableZoneOwnerResolver, StableZoneOwnerRpcTransport, ZoneLeaseObserver,
};
use std::sync::mpsc as sync_mpsc;

const TEST_WAIT: Duration = Duration::from_secs(3);
const FRAME_LIMIT: usize = 64 * 1024;

fn limits() -> ZoneRpcLimits {
    ZoneRpcLimits {
        max_frame_bytes: FRAME_LIMIT,
        outbound_poll_limit: 8,
        io_timeout: Duration::from_secs(1),
        ..ZoneRpcLimits::default()
    }
}

fn listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn accept_request(listener: &TcpListener) -> (TcpStream, ZoneRpcEnvelope) {
    let deadline = Instant::now() + TEST_WAIT;
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "mock Host request did not arrive"
                );
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("mock Host accept failed: {error}"),
        }
    };
    stream.set_read_timeout(Some(TEST_WAIT)).unwrap();
    stream.set_write_timeout(Some(TEST_WAIT)).unwrap();
    let frame = read_frame(&mut stream, FRAME_LIMIT).unwrap();
    assert_eq!(
        frame.first(),
        Some(&b'{'),
        "explicit transport must use JSON"
    );
    let envelope = decode_rpc_envelope(&frame, ZoneRpcCodec::Json).unwrap();
    (stream, envelope)
}

fn assert_no_connection(listener: &TcpListener) {
    match listener.accept() {
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
        Ok(_) => panic!("an unauthorized or fallback TCP connection reached a Host"),
        Err(error) => panic!("mock Host connection check failed: {error}"),
    }
}

fn respond(stream: &mut TcpStream, payload: ZoneRpcPayload) {
    let bytes = encode_rpc_response(
        &ZoneRpcResponse::Ok {
            payload: Box::new(payload),
        },
        ZoneRpcCodec::Json,
    )
    .unwrap();
    write_frame(stream, &bytes, FRAME_LIMIT).unwrap();
}

fn health(host_id: &str) -> ZoneRpcPayload {
    ZoneRpcPayload::Health {
        host_id: host_id.to_owned(),
        process_id: 1,
        session_count: 0,
        active_connections: 1,
        session_capacity: 16,
        session_capacity_per_zone: 16,
        busiest_zone_session_count: 0,
        zone_count: 1,
        zone_capacity: 4,
        draining: false,
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        capabilities: Vec::new(),
    }
}

fn outbounds(sequence: u64, hp: i32) -> ZoneRpcPayload {
    ZoneRpcPayload::Outbounds {
        items: vec![WireSequencedServerFrame {
            sequence,
            frame: encode_server_packet(&ServerPacket::HealthChanged { hp, mp: 11 }).unwrap(),
            owner_health: None,
            source_registration_id: 0,
        }],
        stream_id: "pinned-owner-stream".to_owned(),
        reset: false,
        last_issued_sequence: sequence,
        has_more: false,
        current_source_registration_id: None,
    }
}

fn route_guard(valid: Arc<AtomicBool>) -> Arc<dyn Fn() -> Result<(), String> + Send + Sync> {
    Arc::new(move || {
        if valid.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err("zone_owner_requires_rebind".to_owned())
        }
    })
}

fn tcp(address: SocketAddr) -> TcpZoneOwnerRpcTransport {
    TcpZoneOwnerRpcTransport::with_explicit_options(
        address,
        ZoneId::new("stable-owner-zone"),
        "stable-owner-session".to_owned(),
        Some("explicit-test-token".to_owned()),
        limits(),
    )
}

#[test]
fn explicit_options_pin_one_json_endpoint_without_legacy_shard_standby_or_pool_settings() {
    let host = listener();
    let address = host.local_addr().unwrap();
    let transport = tcp(address);
    // Assert the configuration directly without reading/changing any global
    // env: legacy shard hashing/standby paths require multiple addresses, and
    // legacy codec/pool defaults must not enter this explicit constructor.
    assert_eq!(&*transport.addresses, &[address.to_string()]);
    assert_eq!(transport.active_endpoint.load(Ordering::Acquire), 0);
    assert_eq!(transport.codec, ZoneRpcCodec::Json);
    assert!(!transport.reuse_connections);
    assert!(transport.shared_connections.is_none());
    assert_eq!(transport.connections.len(), 1);
    assert!(transport.connections[0].lock().unwrap().is_none());
    assert_eq!(transport.auth_token.as_deref(), Some("explicit-test-token"));
    let server = thread::spawn(move || {
        let (mut stream, envelope) = accept_request(&host);
        assert_eq!(envelope.session_id, "stable-owner-session");
        assert_eq!(envelope.zone_id, "stable-owner-zone");
        assert_eq!(envelope.auth_token.as_deref(), Some("explicit-test-token"));
        assert!(matches!(envelope.request, ZoneRpcRequest::Health));
        respond(&mut stream, health("host-a"));
        host
    });
    assert_eq!(transport.health().unwrap().host_id, "host-a");
    let host = server.join().unwrap();
    assert_no_connection(&host);
}

#[test]
fn cloned_poll_guard_rejects_before_any_tcp_connection() {
    let host = listener();
    let valid = Arc::new(AtomicBool::new(false));
    let transport = tcp(host.local_addr().unwrap()).with_owner_route_guard(route_guard(valid));
    assert_eq!(
        transport.clone().poll_outbounds(0, 8).unwrap_err(),
        "zone_owner_requires_rebind"
    );
    assert_no_connection(&host);
}

#[test]
fn cloned_poll_discards_old_frames_when_pin_changes_during_readonly_response() {
    let host = listener();
    let valid = Arc::new(AtomicBool::new(true));
    let transport =
        tcp(host.local_addr().unwrap()).with_owner_route_guard(route_guard(valid.clone()));
    let (received, request_arrived) = sync_mpsc::sync_channel(1);
    let (release, response_allowed) = sync_mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        let (mut stream, envelope) = accept_request(&host);
        assert!(matches!(
            envelope.request,
            ZoneRpcRequest::PollOutbounds { .. }
        ));
        received.send(()).unwrap();
        response_allowed.recv_timeout(TEST_WAIT).unwrap();
        respond(&mut stream, outbounds(1, 777));
        host
    });
    let poller = transport.clone();
    let poll = thread::spawn(move || poller.poll_outbounds(0, 8));
    request_arrived.recv_timeout(TEST_WAIT).unwrap();
    valid.store(false, Ordering::SeqCst);
    release.send(()).unwrap();
    assert_eq!(
        poll.join().unwrap().unwrap_err(),
        "zone_owner_requires_rebind"
    );
    assert!(transport.outbound_stream_id.lock().unwrap().is_none());
    assert_eq!(transport.outbound_acknowledged.load(Ordering::Acquire), 0);
    let host = server.join().unwrap();
    assert_no_connection(&host);
}

#[test]
fn live_poll_clone_cancels_registration_and_invalidates_queued_frames_after_epoch_loss() {
    let host = listener();
    let new_host = listener();
    let observer = Observer::initial();
    let resolver = resolver(
        observer.clone(),
        host.local_addr().unwrap(),
        new_host.local_addr().unwrap(),
    );
    let (second_received, second_arrived) = sync_mpsc::sync_channel(1);
    let (release, second_response_allowed) = sync_mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        let (mut probe, envelope) = accept_request(&host);
        assert!(matches!(envelope.request, ZoneRpcRequest::Health));
        respond(&mut probe, health("host-a"));
        let (mut first, envelope) = accept_request(&host);
        assert!(matches!(
            envelope.request,
            ZoneRpcRequest::PollOutbounds {
                acknowledged_sequence: 0,
                ..
            }
        ));
        let mut first_batch = outbounds(1, 321);
        if let ZoneRpcPayload::Outbounds {
            items,
            last_issued_sequence,
            ..
        } = &mut first_batch
        {
            let mut retained = items[0].clone();
            retained.sequence = 2;
            items.push(retained);
            *last_issued_sequence = 2;
        }
        respond(&mut first, first_batch);
        let (mut second, envelope) = accept_request(&host);
        assert!(matches!(
            envelope.request,
            ZoneRpcRequest::PollOutbounds {
                acknowledged_sequence: 2,
                ..
            }
        ));
        second_received.send(()).unwrap();
        second_response_allowed.recv_timeout(TEST_WAIT).unwrap();
        respond(&mut second, outbounds(3, 654));
        host
    });
    let transport = stable(resolver).unwrap();
    let (sender, mut receiver) = mpsc::channel(8);
    let (signal, latest_cancelled) = tokio::sync::watch::channel(0);
    let registration = transport
        .register_live_outbound(
            SharedZoneLiveOutboundSender::single(sender).with_overload_signal(signal),
        )
        .unwrap()
        .unwrap();
    registration.activate();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let first = runtime.block_on(async {
        tokio::time::timeout(TEST_WAIT, receiver.recv())
            .await
            .unwrap()
            .unwrap()
    });
    assert_eq!(first.registration_id(), registration.registration_id());
    assert!(!first.is_overloaded());
    let retained = runtime.block_on(async {
        tokio::time::timeout(TEST_WAIT, receiver.recv())
            .await
            .unwrap()
            .unwrap()
    });
    assert_eq!(retained.registration_id(), registration.registration_id());
    assert!(!retained.is_overloaded());
    second_arrived.recv_timeout(TEST_WAIT).unwrap();
    observer.change_epoch();
    // The next poll is still blocked at the mock Host. No cancellation flag
    // has arrived: this claim must consult its own immutable owner pin now.
    assert_eq!(*latest_cancelled.borrow(), 0);
    assert!(!first.is_overloaded());
    assert!(
        first.claim_for_send().is_err(),
        "queued frame must reject a stale epoch before the next poll returns"
    );
    assert!(!retained.is_overloaded());
    release.send(()).unwrap();
    let sentinel = runtime.block_on(async {
        tokio::time::timeout(TEST_WAIT, receiver.recv())
            .await
            .unwrap()
            .unwrap()
    });
    assert_eq!(sentinel.registration_id(), registration.registration_id());
    assert!(
        sentinel.is_overloaded(),
        "epoch loss must cancel, not return frame 3"
    );
    assert!(sentinel.claim_for_send().is_err());
    assert!(
        retained.is_overloaded(),
        "already queued frame must share transport_failed flag"
    );
    assert!(retained.claim_for_send().is_err());
    assert_eq!(*latest_cancelled.borrow(), registration.registration_id());
    let host = server.join().unwrap();
    assert_no_connection(&host);
    assert_no_connection(&new_host);
    assert!(
        receiver.try_recv().is_err(),
        "no old-owner frame may follow cancellation"
    );
    drop(registration);
}

#[derive(Debug)]
struct Observer(Mutex<ObservedZoneLease>);

impl Observer {
    fn initial() -> Arc<Self> {
        Arc::new(Self(Mutex::new(ObservedZoneLease {
            owner_id: "host-a".to_owned(),
            fencing_token: 7,
            expires_at_ms: 1000,
            heartbeat_at_ms: 100,
        })))
    }

    fn change_owner(&self) {
        *self.0.lock().unwrap() = ObservedZoneLease {
            owner_id: "host-b".to_owned(),
            fencing_token: 8,
            expires_at_ms: 1000,
            heartbeat_at_ms: 100,
        };
    }

    fn change_epoch(&self) {
        self.0.lock().unwrap().fencing_token = 8;
    }
}

impl ZoneLeaseObserver for Observer {
    fn observe(&self, _: &ZoneId) -> Result<Option<ObservedZoneLease>, ResolveError> {
        Ok(Some(self.0.lock().unwrap().clone()))
    }
}

fn resolver(
    observer: Arc<Observer>,
    old: SocketAddr,
    new: SocketAddr,
) -> Arc<StableZoneOwnerResolver> {
    let registry = HostRegistry::from_json(
        &serde_json::json!({
            "version": 1,
            "hosts": [{"ownerId": "host-a", "endpoint": old.to_string()},
                      {"ownerId": "host-b", "endpoint": new.to_string()}],
        })
        .to_string(),
    )
    .unwrap();
    Arc::new(StableZoneOwnerResolver::new(
        registry,
        observer,
        Arc::new(|| 100),
    ))
}

fn stable(resolver: Arc<StableZoneOwnerResolver>) -> Result<StableZoneOwnerRpcTransport, String> {
    StableZoneOwnerRpcTransport::new(
        resolver,
        ZoneOwnerLease::new(ZoneId::new("stable-owner-zone"), "host-a", 7),
        "stable-owner-session".to_owned(),
        Some("explicit-test-token".to_owned()),
        limits(),
    )
}

#[test]
fn stable_constructor_matches_real_host_health_and_owner_change_never_contacts_new_host() {
    let old_host = listener();
    let new_host = listener();
    let observer = Observer::initial();
    let resolver = resolver(
        observer.clone(),
        old_host.local_addr().unwrap(),
        new_host.local_addr().unwrap(),
    );
    let server = thread::spawn(move || {
        let (mut stream, envelope) = accept_request(&old_host);
        assert!(matches!(envelope.request, ZoneRpcRequest::Health));
        respond(&mut stream, health("host-a"));
        let (mut stream, envelope) = accept_request(&old_host);
        assert!(matches!(envelope.request, ZoneRpcRequest::OnConnect));
        respond(&mut stream, ZoneRpcPayload::Packets { frames: Vec::new() });
        old_host
    });
    let transport = stable(resolver).unwrap();
    assert!(transport.on_connect().unwrap().is_empty());
    let old_host = server.join().unwrap();
    observer.change_owner();
    assert_eq!(
        transport.on_connect().unwrap_err(),
        "zone_owner_requires_rebind"
    );
    assert_no_connection(&old_host);
    assert_no_connection(&new_host);
}

#[test]
fn stable_constructor_rejects_mismatched_real_host_after_health_only() {
    let old_host = listener();
    let new_host = listener();
    let observer = Observer::initial();
    let resolver = resolver(
        observer,
        old_host.local_addr().unwrap(),
        new_host.local_addr().unwrap(),
    );
    let server = thread::spawn(move || {
        let (mut stream, envelope) = accept_request(&old_host);
        assert!(matches!(envelope.request, ZoneRpcRequest::Health));
        respond(&mut stream, health("wrong-host"));
        old_host
    });
    assert_eq!(
        stable(resolver).unwrap_err(),
        "zone_owner_host_identity_mismatch"
    );
    let old_host = server.join().unwrap();
    assert_no_connection(&old_host);
    assert_no_connection(&new_host);
}

#[test]
fn stable_constructor_rejects_owner_change_during_health_without_new_host_requests() {
    let old_host = listener();
    let new_host = listener();
    let observer = Observer::initial();
    let resolver = resolver(
        observer.clone(),
        old_host.local_addr().unwrap(),
        new_host.local_addr().unwrap(),
    );
    let server = thread::spawn(move || {
        let (mut stream, envelope) = accept_request(&old_host);
        assert!(matches!(envelope.request, ZoneRpcRequest::Health));
        observer.change_owner();
        respond(&mut stream, health("host-a"));
        old_host
    });
    assert_eq!(stable(resolver).unwrap_err(), "zone_owner_requires_rebind");
    let old_host = server.join().unwrap();
    assert_no_connection(&old_host);
    assert_no_connection(&new_host);
}

const PG_RPC_ZONE: &str = "observer-g1-pg-rpc-unique";
const PG_RPC_OWNER_A: &str = "g1-pg-host-a";
const PG_RPC_OWNER_B: &str = "g1-pg-host-b";

struct PgTcpFixture {
    writer: ::postgres::Client,
    zone: ZoneId,
    heartbeat: i64,
    owns_row: bool,
}

fn pg_rpc_urls() -> Option<(String, String)> {
    let reader = std::env::var("MIR2_TEST_ZONE_OBSERVER_URL");
    let writer = std::env::var("MIR2_TEST_ZONE_OBSERVER_WRITER_URL");
    if matches!(&reader, Err(std::env::VarError::NotPresent))
        && matches!(&writer, Err(std::env::VarError::NotPresent))
    {
        return None;
    }
    let reader = reader.unwrap_or_else(|_| panic!("explicit G1 reader fixture URL is required"));
    let writer = writer.unwrap_or_else(|_| panic!("explicit G1 writer fixture URL is required"));
    assert!(
        !reader.trim().is_empty() && !writer.trim().is_empty(),
        "configured G1 fixture URLs must not be empty"
    );
    Some((reader, writer))
}

fn checked_pg_rpc_config(dsn: &str) -> (::postgres::Config, SocketAddr) {
    let mut config: ::postgres::Config = dsn
        .parse()
        .unwrap_or_else(|_| panic!("configured G1 fixture URL is invalid"));
    let suffix = config
        .get_dbname()
        .and_then(|name| name.strip_prefix("mir2_zone_lease_test_"))
        .filter(|suffix| {
            !suffix.is_empty()
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        });
    assert!(
        suffix.is_some(),
        "G1 integration requires an isolated mir2_zone_lease_test_* database"
    );
    assert!(
        config.get_user().is_some_and(|user| !user.is_empty()),
        "G1 fixture requires an explicit database user"
    );
    assert_eq!(
        config.get_hosts().len().max(config.get_hostaddrs().len()),
        1,
        "G1 fixture requires one explicit database endpoint"
    );
    let ip = if let Some(ip) = config.get_hostaddrs().first() {
        *ip
    } else {
        match config.get_hosts().first() {
            Some(::postgres::config::Host::Tcp(host)) => host
                .parse::<std::net::IpAddr>()
                .unwrap_or_else(|_| panic!("G1 database host must be a numeric loopback IP")),
            _ => panic!("G1 database host must be a numeric loopback IP"),
        }
    };
    assert!(
        ip.is_loopback(),
        "G1 integration must use a loopback database"
    );
    assert_eq!(
        config.get_ports().len(),
        1,
        "G1 fixture requires one explicit database port"
    );
    let port = config.get_ports()[0];
    assert!(
        port != 0 && port != 5432,
        "G1 fixture must use its isolated non-default database port"
    );
    config
        .connect_timeout(Duration::from_secs(2))
        .notice_callback(|_| {})
        .options("-c statement_timeout=2000 -c lock_timeout=1000");
    (config, SocketAddr::new(ip, port))
}

fn pg_rpc_ok<T>(result: Result<T, ::postgres::Error>, message: &'static str) -> T {
    result.unwrap_or_else(|_| panic!("{message}"))
}

impl PgTcpFixture {
    fn create(writer: ::postgres::Client) -> Self {
        let heartbeat = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let mut fixture = Self {
            writer,
            zone: ZoneId::new(PG_RPC_ZONE),
            heartbeat,
            owns_row: false,
        };
        // No upsert: an existing row is not ours and must never be replaced or
        // deleted by this test's cleanup path.
        let inserted = pg_rpc_ok(fixture.writer.execute(
            "INSERT INTO public.zone_owner_leases(zone_id,owner_id,fencing_token,expires_at_ms,heartbeat_at_ms) VALUES($1,$2,7,$3,$4)",
            &[&fixture.zone.as_str(), &PG_RPC_OWNER_A, &(heartbeat + 60_000), &heartbeat],
        ), "G1 fixture lease insert failed; an existing test row must not be overwritten");
        fixture.owns_row = inserted == 1;
        assert!(
            fixture.owns_row,
            "G1 fixture must insert exactly its own lease row"
        );
        fixture
    }

    fn change_owner(&mut self) {
        let updated = pg_rpc_ok(self.writer.execute(
            "UPDATE public.zone_owner_leases SET owner_id=$2,fencing_token=8 WHERE zone_id=$1 AND owner_id=$3 AND fencing_token=7 AND heartbeat_at_ms=$4",
            &[&self.zone.as_str(), &PG_RPC_OWNER_B, &PG_RPC_OWNER_A, &self.heartbeat],
        ), "G1 fixture owner update failed");
        assert_eq!(
            updated, 1,
            "G1 fixture must update only its own prior epoch"
        );
    }
}

impl Drop for PgTcpFixture {
    fn drop(&mut self) {
        if self.owns_row {
            let _ = self.writer.execute(
                "DELETE FROM public.zone_owner_leases WHERE zone_id=$1 AND heartbeat_at_ms=$2 AND owner_id IN ($3,$4) AND fencing_token IN (7,8)",
                &[&self.zone.as_str(), &self.heartbeat, &PG_RPC_OWNER_A, &PG_RPC_OWNER_B],
            );
        }
    }
}

fn pg_rpc_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn pg_rpc_host_reads(host: TcpListener, host_id: &'static str) -> thread::JoinHandle<TcpListener> {
    thread::spawn(move || {
        let (mut stream, envelope) = accept_request(&host);
        assert!(matches!(envelope.request, ZoneRpcRequest::Health));
        respond(&mut stream, health(host_id));
        let (mut stream, envelope) = accept_request(&host);
        assert!(matches!(envelope.request, ZoneRpcRequest::ActiveIdentity));
        respond(
            &mut stream,
            ZoneRpcPayload::ActiveIdentity { identity: None },
        );
        host
    })
}

#[test]
fn postgres_reader_to_real_tcp_host_stays_pinned_until_explicit_rebind() {
    let Some((reader_dsn, writer_dsn)) = pg_rpc_urls() else {
        return;
    };
    let (reader_config, reader_endpoint) = checked_pg_rpc_config(&reader_dsn);
    let (writer_config, writer_endpoint) = checked_pg_rpc_config(&writer_dsn);
    assert_eq!(
        reader_endpoint, writer_endpoint,
        "G1 reader and writer must share the isolated database endpoint"
    );
    assert!(
        reader_config.get_dbname() == writer_config.get_dbname(),
        "G1 reader and writer must share the isolated database name"
    );
    assert!(
        reader_config.get_user() != writer_config.get_user(),
        "G1 reader and writer must be separate roles"
    );
    let writer = pg_rpc_ok(
        writer_config.connect(::postgres::NoTls),
        "explicit G1 writer fixture cannot connect",
    );
    let observer = Arc::new(
        PostgresZoneLeaseObserver::connect(&reader_dsn, PostgresObserverLimits::default())
            .unwrap_or_else(|_| {
                panic!("explicit G1 reader fixture failed the readonly privilege audit")
            }),
    );
    let mut fixture = PgTcpFixture::create(writer);
    let host_a = listener();
    let host_b = listener();
    let address_a = host_a.local_addr().unwrap();
    let address_b = host_b.local_addr().unwrap();
    let original_registry = HostRegistry::from_json(
        &serde_json::json!({
            "version": 1, "hosts": [{"ownerId": PG_RPC_OWNER_A, "endpoint": address_a.to_string()}],
        })
        .to_string(),
    )
    .unwrap();
    let expanded_reordered_registry = HostRegistry::from_json(
        &serde_json::json!({
            "version": 1,
            "hosts": [{"ownerId": PG_RPC_OWNER_B, "endpoint": address_b.to_string()},
                      {"ownerId": PG_RPC_OWNER_A, "endpoint": address_a.to_string()}],
        })
        .to_string(),
    )
    .unwrap();
    let original =
        StableZoneOwnerResolver::new(original_registry, observer.clone(), Arc::new(pg_rpc_clock));
    let expanded = Arc::new(StableZoneOwnerResolver::new(
        expanded_reordered_registry,
        observer,
        Arc::new(pg_rpc_clock),
    ));
    let initial = original.resolve(&fixture.zone).unwrap();
    assert_eq!(initial.endpoint(), address_a);
    assert_eq!(
        expanded.resolve(&fixture.zone).unwrap(),
        initial,
        "adding/reordering registry entries must not move the real PostgreSQL lease"
    );
    assert_no_connection(&host_a);
    assert_no_connection(&host_b);

    let server_a = pg_rpc_host_reads(host_a, PG_RPC_OWNER_A);
    let old_transport = StableZoneOwnerRpcTransport::new(
        expanded.clone(),
        initial.lease().clone(),
        "g1-pg-session-a".to_owned(),
        Some("explicit-test-token".to_owned()),
        limits(),
    )
    .unwrap();
    assert!(old_transport.active_identity().unwrap().is_none());
    let host_a = server_a.join().unwrap();
    fixture.change_owner();
    assert_eq!(
        old_transport.on_connect().unwrap_err(),
        "zone_owner_requires_rebind"
    );
    assert_eq!(
        old_transport.active_identity().unwrap_err(),
        "zone_owner_requires_rebind"
    );
    assert_no_connection(&host_a);
    assert_no_connection(&host_b);

    let rebound = expanded.resolve(&fixture.zone).unwrap();
    assert_eq!(rebound.lease().owner_id(), PG_RPC_OWNER_B);
    assert_eq!(rebound.lease().fencing_token(), 8);
    assert_eq!(rebound.endpoint(), address_b);
    let server_b = pg_rpc_host_reads(host_b, PG_RPC_OWNER_B);
    let new_transport = StableZoneOwnerRpcTransport::new(
        expanded,
        rebound.lease().clone(),
        "g1-pg-session-b".to_owned(),
        Some("explicit-test-token".to_owned()),
        limits(),
    )
    .unwrap();
    assert!(new_transport.active_identity().unwrap().is_none());
    let host_b = server_b.join().unwrap();
    assert_no_connection(&host_a);
    assert_no_connection(&host_b);
}

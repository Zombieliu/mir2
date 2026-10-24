//! Typed ordinary attacks never replay a possibly committed mutation.
use super::*;
use crate::routing::ZoneOwnerLeaseAuthority;
use crate::InMemoryZoneOwnerLeaseAuthority;

fn request(zone: &ZoneId) -> ZoneOwnerCommandRequest {
    ZoneOwnerCommandRequest::production_player(
        ZoneOwnerLease::new(zone.clone(), "attack-owner", 1),
        true,
        WorldCommand::Attack { object_id: 700 },
    )
}

fn transport(addresses: Vec<String>, codec: ZoneRpcCodec) -> TcpZoneOwnerRpcTransport {
    let mut transport = TcpZoneOwnerRpcTransport::with_endpoints(
        addresses,
        ZoneId::new("attack-zone"),
        "attack-rpc-session",
        None,
        ZoneRpcLimits {
            io_timeout: Duration::from_millis(500),
            ..ZoneRpcLimits::default()
        },
    )
    .unwrap();
    // The fixture controls the exact one-frame connections, independently of
    // a developer's opt-in shared-pool environment.
    transport.shared_connections = None;
    transport.codec = codec;
    transport
}

fn respond(stream: &mut TcpStream, payload: ZoneRpcPayload, codec: ZoneRpcCodec) {
    let bytes = encode_rpc_response(
        &ZoneRpcResponse::Ok {
            payload: Box::new(payload),
        },
        codec,
    )
    .unwrap();
    write_frame(stream, &bytes, 64 * 1024).unwrap();
}

fn health(codec: ZoneRpcCodec, capable: bool) -> Vec<u8> {
    let capabilities = if capable {
        vec![ZONE_RPC_NATIVE_ATTACK_OUTCOME_V1]
    } else {
        vec![]
    };
    let value = serde_json::json!({"status":"ok","payload":{
        "type":"health", "host_id":"attack-test-host", "process_id":1,
        "session_count":1, "active_connections":1, "session_capacity":16,
        "session_capacity_per_zone":16, "busiest_zone_session_count":1,
        "zone_count":1, "zone_capacity":4, "draining":false,
        "protocol_version":ZONE_RPC_PROTOCOL_VERSION,"capabilities":capabilities
    }});
    encode_rpc_value(&value, codec).unwrap()
}

fn accept_request(listener: &TcpListener, codec: ZoneRpcCodec) -> (TcpStream, ZoneRpcEnvelope) {
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let bytes = read_frame(&mut stream, 64 * 1024).unwrap();
    let request = decode_rpc_envelope(&bytes, codec).unwrap();
    (stream, request)
}

fn complete(admission: WireNativeAttackAdmission) -> ZoneRpcPayload {
    // An ordinary location correction is deliberately identical for both
    // results. Only the explicit typed outcome may distinguish them.
    ZoneRpcPayload::NativeAttack {
        response: WireNativeAttackResponse::Complete {
            frames: vec![encode_server_packet(&ServerPacket::UserLocation {
                location: mir2_protocol::UserLocation {
                    position: Point { x: 10, y: 20 },
                    direction: mir2_protocol::MirDirection::Down,
                },
            })
            .unwrap()],
            packet_count: 1,
            snapshot_tick: 7,
            active_identity: None,
            admission,
        },
    }
}

#[test]
fn typed_launch_and_nonlaunch_survive_json_and_messagepack_without_packet_inference() {
    for codec in [ZoneRpcCodec::Json, ZoneRpcCodec::MessagePack] {
        for admission in [
            WireNativeAttackAdmission::Launched,
            WireNativeAttackAdmission::NotLaunched,
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (mut stream, envelope) = accept_request(&listener, codec);
                assert!(matches!(envelope.request, ZoneRpcRequest::Health));
                write_frame(&mut stream, &health(codec, true), 64 * 1024).unwrap();
                drop(stream);
                let (mut stream, envelope) = accept_request(&listener, codec);
                assert!(matches!(
                    envelope.request,
                    ZoneRpcRequest::ExecuteNativeAttack {
                        authenticated: true,
                        object_id: 700,
                        ..
                    }
                ));
                respond(&mut stream, complete(admission), codec);
            });
            let transport = transport(vec![address.to_string()], codec);
            let execution = transport
                .execute_production_attack_requiring_typed_outcome(request(&transport.zone_id))
                .unwrap();
            server.join().unwrap();
            assert_eq!(
                execution.admission,
                match admission {
                    WireNativeAttackAdmission::Launched => NativeAttackAdmission::Launched,
                    WireNativeAttackAdmission::NotLaunched => NativeAttackAdmission::NotLaunched,
                }
            );
            assert_eq!(
                execution.execution.outcome.command_kind,
                WorldCommand::Attack { object_id: 700 }.kind()
            );
            assert_eq!(execution.execution.outcome.packet_count, 1);
            assert_eq!(execution.execution.outcome.snapshot_tick, 7);
            assert!(matches!(
                execution.execution.packets[0],
                ServerPacket::UserLocation { .. }
            ));
        }
    }
}

#[test]
fn old_host_is_probed_but_receives_no_attack_or_generic_execute() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, envelope) = accept_request(&listener, ZoneRpcCodec::Json);
        assert!(matches!(envelope.request, ZoneRpcRequest::Health));
        write_frame(&mut stream, &health(ZoneRpcCodec::Json, false), 64 * 1024).unwrap();
        drop(stream);
        listener
    });
    let transport = transport(vec![address.to_string()], ZoneRpcCodec::Json);
    assert!(
        matches!(transport.execute_production_attack_requiring_typed_outcome(request(&transport.zone_id)),
        Err(NativeAttackError::BeforeExecution(detail)) if detail.contains(ZONE_RPC_NATIVE_ATTACK_OUTCOME_V1))
    );
    let listener = server.join().unwrap();
    listener.set_nonblocking(true).unwrap();
    assert!(matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
}

#[test]
fn lost_reply_never_contacts_another_endpoint_or_replays_generic_attack() {
    let first = TcpListener::bind("127.0.0.1:0").unwrap();
    let second = TcpListener::bind("127.0.0.1:0").unwrap();
    let addresses = vec![
        first.local_addr().unwrap().to_string(),
        second.local_addr().unwrap().to_string(),
    ];
    let observed = Arc::new(AtomicUsize::new(0));
    let counted = observed.clone();
    let server = thread::spawn(move || {
        let (mut stream, envelope) = accept_request(&first, ZoneRpcCodec::Json);
        assert!(matches!(envelope.request, ZoneRpcRequest::Health));
        write_frame(&mut stream, &health(ZoneRpcCodec::Json, true), 64 * 1024).unwrap();
        drop(stream);
        let (stream, envelope) = accept_request(&first, ZoneRpcCodec::Json);
        assert!(matches!(
            envelope.request,
            ZoneRpcRequest::ExecuteNativeAttack { .. }
        ));
        counted.fetch_add(1, Ordering::SeqCst);
        // The host may have committed. Closing without a response provides no
        // evidence that replay at this or another owner would be safe.
        drop(stream);
        first
    });
    let transport = transport(addresses, ZoneRpcCodec::Json);
    assert!(
        matches!(transport.execute_production_attack_requiring_typed_outcome(request(&transport.zone_id)),
        Err(NativeAttackError::Unknown(detail)) if detail.contains("no endpoint fallback"))
    );
    let first = server.join().unwrap();
    assert_eq!(observed.load(Ordering::SeqCst), 1);
    for listener in [first, second] {
        listener.set_nonblocking(true).unwrap();
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock)
        );
    }
}

#[test]
fn absent_typed_result_after_dispatch_is_unknown_instead_of_nonlaunch() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = accept_request(&listener, ZoneRpcCodec::Json);
        write_frame(&mut stream, &health(ZoneRpcCodec::Json, true), 64 * 1024).unwrap();
        drop(stream);
        let (mut stream, envelope) = accept_request(&listener, ZoneRpcCodec::Json);
        assert!(matches!(
            envelope.request,
            ZoneRpcRequest::ExecuteNativeAttack { .. }
        ));
        respond(
            &mut stream,
            ZoneRpcPayload::Execution {
                frames: vec![],
                packet_count: 0,
                snapshot_tick: 1,
                active_identity: None,
                game_shop_purchase_outcome: None,
            },
            ZoneRpcCodec::Json,
        );
    });
    let transport = transport(vec![address.to_string()], ZoneRpcCodec::Json);
    assert!(matches!(
        transport.execute_production_attack_requiring_typed_outcome(request(&transport.zone_id)),
        Err(NativeAttackError::Unknown(_))
    ));
    server.join().unwrap();
}

#[test]
fn typed_errors_keep_unsupported_preflight_and_unknown_distinct() {
    for error in [
        NativeAttackError::Unsupported,
        NativeAttackError::BeforeExecution("lease rejected".into()),
        NativeAttackError::Unknown("failed after dispatch".into()),
    ] {
        let bytes = encode_rpc_value(
            &WireNativeAttackResponse::Error {
                error: error.clone().into(),
            },
            ZoneRpcCodec::Json,
        )
        .unwrap();
        let response: WireNativeAttackResponse =
            decode_rpc_value(&bytes, ZoneRpcCodec::Json).unwrap();
        assert_eq!(response.into_execution(), Err(error));
    }
}

#[test]
fn incomplete_or_corrupt_wire_results_never_default_to_nonlaunch() {
    for value in [
        serde_json::json!({"status":"complete","frames":[],"packet_count":0,"snapshot_tick":0,"active_identity":null}),
        serde_json::json!({"status":"complete","frames":[],"packet_count":0,"snapshot_tick":0,"active_identity":null,"admission":"future"}),
    ] {
        assert!(serde_json::from_value::<WireNativeAttackResponse>(value).is_err());
    }
    let response = WireNativeAttackResponse::Complete {
        frames: vec![],
        packet_count: 1,
        snapshot_tick: 0,
        active_identity: None,
        admission: WireNativeAttackAdmission::NotLaunched,
    };
    assert!(matches!(
        response.into_execution(),
        Err(NativeAttackError::Unknown(_))
    ));
    let response = WireNativeAttackResponse::Complete {
        frames: vec![vec![0xff]],
        packet_count: 1,
        snapshot_tick: 0,
        active_identity: None,
        admission: WireNativeAttackAdmission::Launched,
    };
    assert!(matches!(
        response.into_execution(),
        Err(NativeAttackError::Unknown(_))
    ));
}

#[test]
fn invalid_production_requests_fail_before_contacting_the_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let transport = transport(
        vec![listener.local_addr().unwrap().to_string()],
        ZoneRpcCodec::Json,
    );
    let lease = request(&transport.zone_id).owner_lease().clone();
    listener.set_nonblocking(true).unwrap();
    for (name, request) in [
        (
            "direct",
            ZoneOwnerCommandRequest::direct(lease.clone(), WorldCommand::Attack { object_id: 700 }),
        ),
        (
            "unauthenticated",
            ZoneOwnerCommandRequest::production_player(
                lease.clone(),
                false,
                WorldCommand::Attack { object_id: 700 },
            ),
        ),
        (
            "debug-command",
            ZoneOwnerCommandRequest::production_player(
                lease.clone(),
                true,
                WorldCommand::MoveTo {
                    position: Point { x: 1, y: 2 },
                    running: false,
                },
            ),
        ),
        ("wrong-world", request(&ZoneId::new("another-world"))),
        (
            "zero-target",
            ZoneOwnerCommandRequest::production_player(
                lease,
                true,
                WorldCommand::Attack { object_id: 0 },
            ),
        ),
    ] {
        assert!(
            matches!(
                transport.execute_production_attack_requiring_typed_outcome(request),
                Err(NativeAttackError::BeforeExecution(_))
            ),
            "{name}"
        );
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
            "{name} must not contact even the capability endpoint"
        );
    }
}

#[test]
fn retrying_rpc_helper_cannot_send_the_dedicated_mutation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let transport = transport(
        vec![listener.local_addr().unwrap().to_string()],
        ZoneRpcCodec::Json,
    );
    assert!(transport
        .call(ZoneRpcRequest::ExecuteNativeAttack {
            owner_lease: request(&transport.zone_id).owner_lease().into(),
            authenticated: true,
            object_id: 700,
        })
        .unwrap_err()
        .contains("single-attempt"));
    listener.set_nonblocking(true).unwrap();
    assert!(matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
}

fn host() -> (
    ZoneHostServer,
    Arc<InMemoryZoneOwnerLeaseAuthority>,
    ZoneOwnerLease,
) {
    let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
    let lease = authority.owner_lease(&ZoneId::primary());
    let server = ZoneHostServer::with_options_and_factory(
        GatewayConfig::default(),
        authority.clone(),
        None,
        ZoneRpcLimits::default(),
        Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
            Duration::from_secs(3600),
            BTreeMap::new(),
        )),
    );
    (server, authority, lease)
}

fn envelope(lease: &ZoneOwnerLease, authenticated: bool) -> ZoneRpcEnvelope {
    ZoneRpcEnvelope {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        session_id: "host-attack-session".into(),
        zone_id: ZoneId::primary().as_str().into(),
        auth_token: None,
        request: ZoneRpcRequest::ExecuteNativeAttack {
            owner_lease: lease.into(),
            authenticated,
            object_id: 700,
        },
    }
}

#[test]
fn real_host_rejects_unauthenticated_and_mismatched_world_before_runtime_or_journal() {
    let (server, _, lease) = host();
    let mut zero_target = envelope(&lease, true);
    let ZoneRpcRequest::ExecuteNativeAttack { object_id, .. } = &mut zero_target.request else {
        unreachable!()
    };
    *object_id = 0;
    for (name, envelope) in [
        ("unauthenticated", envelope(&lease, false)),
        (
            "wrong-world",
            envelope(
                &ZoneOwnerLease::new(ZoneId::new("other-world"), "owner", 1),
                true,
            ),
        ),
        ("zero-target", zero_target),
    ] {
        let ZoneRpcPayload::NativeAttack { response } = server.handle_envelope(envelope).unwrap()
        else {
            panic!("typed response")
        };
        assert!(
            matches!(
                response.into_execution(),
                Err(NativeAttackError::BeforeExecution(_))
            ),
            "{name}"
        );
        assert_eq!(
            server.session_count(),
            0,
            "{name} must not create a hosted Session"
        );
        assert_eq!(
            server.runtime_execute_calls.load(Ordering::Acquire),
            0,
            "{name}"
        );
        assert!(server.journal.lock().unwrap().entries.is_empty(), "{name}");
    }
}

#[test]
fn real_host_rejects_stale_owner_before_entering_runtime() {
    let (server, _, lease) = host();
    let stale = ZoneOwnerLease::new(
        lease.zone_id().clone(),
        "stale-owner",
        lease.fencing_token(),
    );
    let ZoneRpcPayload::NativeAttack { response } =
        server.handle_envelope(envelope(&stale, true)).unwrap()
    else {
        panic!("typed response")
    };
    assert!(matches!(
        response.into_execution(),
        Err(NativeAttackError::BeforeExecution(_))
    ));
    assert_eq!(server.runtime_execute_calls.load(Ordering::Acquire), 0);
    assert!(server.journal.lock().unwrap().entries.is_empty());
}

#[test]
fn real_host_promotion_and_sequence_gates_cover_dedicated_attack() {
    let (server, _, lease) = host();
    server
        .quiesced_zones
        .lock()
        .unwrap()
        .insert(lease.zone_id().as_str().into());
    assert_eq!(
        server
            .handle_envelope(envelope(&lease, true))
            .unwrap_err()
            .code,
        "zone_quiesced"
    );
    server.quiesced_zones.lock().unwrap().clear();
    server
        .journal
        .lock()
        .unwrap()
        .replication
        .cursors
        .entry(lease.zone_id().as_str().into())
        .or_default()
        .next_sequence = u64::MAX;
    let ZoneRpcPayload::NativeAttack { response } =
        server.handle_envelope(envelope(&lease, true)).unwrap()
    else {
        panic!("typed response")
    };
    assert!(
        matches!(response.into_execution(), Err(NativeAttackError::BeforeExecution(detail)) if detail.contains("sequence"))
    );
    assert_eq!(server.runtime_execute_calls.load(Ordering::Acquire), 0);
    assert!(server.journal.lock().unwrap().entries.is_empty());
}

#[test]
fn real_standby_rejects_dedicated_attack_before_session_creation() {
    let (server, _, lease) = host();
    server
        .runtime_factory
        .lock()
        .unwrap()
        .mark_zone_as_replica(lease.zone_id());
    assert_eq!(
        server
            .handle_envelope(envelope(&lease, true))
            .unwrap_err()
            .code,
        "zone_replica_read_only"
    );
    assert_eq!(server.session_count(), 0);
    assert_eq!(server.runtime_execute_calls.load(Ordering::Acquire), 0);
}

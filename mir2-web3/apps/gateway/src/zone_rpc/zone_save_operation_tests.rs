//! Wire/loopback boundary coverage. Scripted receipts prove transport behavior;
//! only the separate PostgreSQL integration tests prove durable save commits.
use super::*;
use crate::routing::ZoneOwnerLeaseAuthority;

const WAIT: Duration = Duration::from_secs(2);
const FRAME_LIMIT: usize = 1024 * 1024;
const SESSION: &str = "save-rpc-session";
const ZONE: &str = "save-rpc-zone";

struct ProbeAuthority {
    transactional: bool,
    world_calls: AtomicUsize,
}
impl ZoneOwnerLeaseAuthority for ProbeAuthority {
    fn requires_transactional_zone_save_fence(&self) -> bool {
        self.transactional
    }
    fn owner_lease(&self, _: &ZoneId) -> ZoneOwnerLease {
        self.world_calls.fetch_add(1, Ordering::SeqCst);
        panic!("receipt lookup/capability validation must not admit a world")
    }
    fn validate_owner_lease(&self, _: &ZoneOwnerLease) -> Result<(), String> {
        self.world_calls.fetch_add(1, Ordering::SeqCst);
        Err("unexpected world admission".into())
    }
}
fn authority(transactional: bool) -> Arc<ProbeAuthority> {
    Arc::new(ProbeAuthority {
        transactional,
        world_calls: AtomicUsize::new(0),
    })
}
fn lease() -> ZoneOwnerLease {
    ZoneOwnerLease::new(ZoneId::new(ZONE), "save-owner", 7)
}
fn binding() -> ZoneSaveOperationBinding {
    ZoneSaveOperationBinding::new(
        "save-operation".into(),
        SESSION.into(),
        ZoneSaveFence::new(ZONE.into(), "save-owner".into(), 7).unwrap(),
    )
    .unwrap()
}
fn limits() -> ZoneRpcLimits {
    ZoneRpcLimits {
        max_frame_bytes: FRAME_LIMIT,
        io_timeout: WAIT,
        ..ZoneRpcLimits::default()
    }
}
fn transport(address: SocketAddr, codec: ZoneRpcCodec) -> TcpZoneOwnerRpcTransport {
    let mut transport = TcpZoneOwnerRpcTransport::with_explicit_options(
        address,
        ZoneId::new(ZONE),
        SESSION.into(),
        Some("save-test-token".into()),
        limits(),
    );
    transport.codec = codec;
    transport
}
fn envelope(request: ZoneRpcRequest) -> ZoneRpcEnvelope {
    ZoneRpcEnvelope {
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        session_id: SESSION.into(),
        zone_id: ZONE.into(),
        auth_token: Some("save-test-token".into()),
        request,
    }
}
fn accept(listener: &TcpListener, codec: ZoneRpcCodec) -> (TcpStream, ZoneRpcEnvelope) {
    let deadline = Instant::now() + WAIT;
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "save RPC request never arrived");
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    };
    stream.set_read_timeout(Some(WAIT)).unwrap();
    stream.set_write_timeout(Some(WAIT)).unwrap();
    let bytes = read_frame(&mut stream, FRAME_LIMIT).unwrap();
    (stream, decode_rpc_envelope(&bytes, codec).unwrap())
}
fn listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}
fn assert_no_connection(listener: &TcpListener) {
    assert!(matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
}
fn respond(stream: &mut TcpStream, payload: ZoneRpcPayload, codec: ZoneRpcCodec) {
    let bytes = encode_rpc_response(
        &ZoneRpcResponse::Ok {
            payload: Box::new(payload),
        },
        codec,
    )
    .unwrap();
    write_frame(stream, &bytes, FRAME_LIMIT).unwrap();
}
fn health(capable: bool) -> ZoneRpcPayload {
    ZoneRpcPayload::Health {
        host_id: "save-test-host".into(),
        process_id: 1,
        session_count: 0,
        active_connections: 1,
        session_capacity: 8,
        session_capacity_per_zone: 8,
        busiest_zone_session_count: 0,
        zone_count: 0,
        zone_capacity: 8,
        draining: false,
        protocol_version: ZONE_RPC_PROTOCOL_VERSION,
        capabilities: if capable {
            vec![ZONE_RPC_ZONE_SAVE_OPERATIONS_V1.into()]
        } else {
            vec![]
        },
    }
}
fn source_config() -> GatewayConfig {
    let mut config = GatewayConfig::default();
    // Configuration-only capability test; no database connection or migration.
    config.account_store_database_url = Some("postgres://test@127.0.0.1/save_capability".into());
    config.account_store_database_mode = mir2_simulation::AccountStoreDatabaseMode::SourceOfTruth;
    config
}
fn receipt_json(status: &str, binding: &ZoneSaveOperationBinding) -> serde_json::Value {
    let config = GatewayConfig::default();
    let index = config.default_character.index;
    let frozen = config.account_store.lock().unwrap().accounts["demo"].saves[&index].clone();
    let mut expected = frozen.clone();
    expected.revision += 1;
    let frozen = serde_json::to_value(frozen).unwrap();
    let expected = serde_json::to_value(expected).unwrap();
    let digest = |value: &serde_json::Value| {
        Sha256::digest(serde_json::to_vec(value).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    serde_json::json!({
        "operation": { "binding": binding.to_json().unwrap(), "accountId": "demo",
            "frozenCheckpoint": frozen, "expectedSavedCheckpoint": expected,
            "checkpointDigest": digest(&frozen), "expectedDigest": digest(&expected),
            "lastAccessBinaryDatetime": null },
        "status": status,
        "savedCheckpoint": if status == "Prepared" { None } else { Some(expected) },
        "accountStoreVersion": if status == "Prepared" { None } else { Some(4) },
        "characterSaveVersion": if status == "Prepared" { None } else { Some(3) }
    })
}

#[test]
fn capability_requires_postgres_source_and_transactional_lease_without_world_admission() {
    for (config, transactional, expected) in [
        (GatewayConfig::default(), false, false),
        (GatewayConfig::default(), true, false),
        (source_config(), false, false),
        (source_config(), true, true),
    ] {
        let authority = authority(transactional);
        let server = ZoneHostServer::with_options(
            config,
            authority.clone(),
            Some("save-test-token".into()),
            limits(),
        );
        let ZoneRpcPayload::Health { capabilities, .. } = server
            .handle_envelope(envelope(ZoneRpcRequest::Health))
            .unwrap()
        else {
            panic!("health expected")
        };
        assert_eq!(
            capabilities
                .iter()
                .any(|v| v == ZONE_RPC_ZONE_SAVE_OPERATIONS_V1),
            expected
        );
        assert_eq!(server.session_count(), 0);
        assert_eq!(authority.world_calls.load(Ordering::SeqCst), 0);
    }
    let mut file = source_config();
    file.account_store_path = Some(std::path::PathBuf::from("save-capability-file.json"));
    let server = ZoneHostServer::with_options(
        file,
        authority(true),
        Some("save-test-token".into()),
        limits(),
    );
    assert!(!server.supports_zone_save_operations());
    let mut projection = source_config();
    projection.account_store_database_mode = mir2_simulation::AccountStoreDatabaseMode::Mirror;
    let server = ZoneHostServer::with_options(
        projection,
        authority(true),
        Some("save-test-token".into()),
        limits(),
    );
    assert!(!server.supports_zone_save_operations());
}

#[test]
fn transactional_owner_rejects_legacy_save_before_world_admission() {
    let authority = authority(true);
    let server = ZoneHostServer::with_options(
        source_config(),
        authority.clone(),
        Some("save-test-token".into()),
        limits(),
    );
    let error = server
        .handle_envelope(envelope(ZoneRpcRequest::SaveActiveCharacter))
        .unwrap_err();
    assert_eq!(error.code, "unsupported_capability");
    assert_eq!(server.session_count(), 0);
    assert_eq!(authority.world_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn real_loopback_host_without_capability_rejects_both_codecs_without_creating_runtime() {
    for codec in [ZoneRpcCodec::Json, ZoneRpcCodec::MessagePack] {
        let authority = authority(false);
        let server = Arc::new(ZoneHostServer::with_options(
            GatewayConfig::default(),
            authority.clone(),
            Some("save-test-token".into()),
            limits(),
        ));
        let socket = listener();
        let address = socket.local_addr().unwrap();
        let host = server.clone();
        let worker = thread::spawn(move || {
            // PREPARE probes Health and must stop before sending a mutation.
            let deadline = Instant::now() + WAIT;
            let (stream, _) = loop {
                match socket.accept() {
                    Ok(pair) => break pair,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "real Host capability probe never arrived"
                        );
                        thread::sleep(Duration::from_millis(1))
                    }
                    Err(error) => panic!("accept failed: {error}"),
                }
            };
            host.handle_connection(stream).unwrap();
            socket
        });
        let error = transport(address, codec)
            .prepare_zone_save_operation(&lease(), "save-operation")
            .unwrap_err();
        assert!(error.contains(ZONE_RPC_ZONE_SAVE_OPERATIONS_V1));
        assert!(!error.contains("COMMIT_OUTCOME_UNKNOWN"));
        assert_no_connection(&worker.join().unwrap());
        assert_eq!(server.session_count(), 0);
        assert_eq!(authority.world_calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn legacy_or_multi_endpoint_and_foreign_bindings_fail_before_contact() {
    let socket = listener();
    let address = socket.local_addr().unwrap();
    let legacy = TcpZoneOwnerRpcTransport::with_options(
        address.to_string(),
        ZoneId::new(ZONE),
        SESSION,
        None,
        limits(),
    );
    assert!(!legacy.supports_zone_save_operations());
    assert!(legacy
        .prepare_zone_save_operation(&lease(), "save-operation")
        .is_err());
    let mut multiple = transport(address, ZoneRpcCodec::Json);
    multiple.addresses = Arc::new(vec![address.to_string(), address.to_string()]);
    assert!(!multiple.supports_zone_save_operations());
    assert!(multiple.persist_zone_save_operation(&binding()).is_err());
    let pinned = transport(address, ZoneRpcCodec::Json);
    for foreign in [
        ZoneSaveOperationBinding::new(
            "operation".into(),
            "foreign-session".into(),
            binding().fence().clone(),
        )
        .unwrap(),
        ZoneSaveOperationBinding::new(
            "operation".into(),
            SESSION.into(),
            ZoneSaveFence::new("foreign-zone".into(), "save-owner".into(), 7).unwrap(),
        )
        .unwrap(),
    ] {
        assert!(pinned.persist_zone_save_operation(&foreign).is_err());
        assert!(pinned.query_zone_save_operation(&foreign).is_err());
        assert!(pinned.release_zone_save_operation(&foreign).is_err());
    }
    assert_no_connection(&socket);
}

#[test]
fn prepare_persist_release_reply_loss_is_unknown_and_never_retried() {
    for codec in [ZoneRpcCodec::Json, ZoneRpcCodec::MessagePack] {
        for action in 0..3 {
            let socket = listener();
            let address = socket.local_addr().unwrap();
            let worker = thread::spawn(move || {
                let (mut stream, request) = accept(&socket, codec);
                assert!(matches!(request.request, ZoneRpcRequest::Health));
                respond(&mut stream, health(true), codec);
                drop(stream);
                let (stream, request) = accept(&socket, codec);
                assert!(matches!(
                    (action, request.request),
                    (0, ZoneRpcRequest::PrepareZoneSaveOperation { .. })
                        | (1, ZoneRpcRequest::PersistZoneSaveOperation { .. })
                        | (2, ZoneRpcRequest::ReleaseZoneSaveOperation { .. })
                ));
                drop(stream); // bytes arrived; outcome is now deliberately unknown.
                socket
            });
            let rpc = transport(address, codec);
            let error = match action {
                0 => rpc
                    .prepare_zone_save_operation(&lease(), "save-operation")
                    .unwrap_err(),
                1 => rpc.persist_zone_save_operation(&binding()).unwrap_err(),
                _ => rpc.release_zone_save_operation(&binding()).unwrap_err(),
            };
            assert!(error.contains("COMMIT_OUTCOME_UNKNOWN"));
            assert!(error.contains("no endpoint fallback"));
            assert_no_connection(&worker.join().unwrap());
        }
    }
}

#[test]
fn checked_receipts_roundtrip_both_codecs_and_writes_submit_only_binding() {
    for codec in [ZoneRpcCodec::Json, ZoneRpcCodec::MessagePack] {
        for action in 0..4 {
            let socket = listener();
            let address = socket.local_addr().unwrap();
            let expected = binding();
            let response = receipt_json(
                if action == 0 {
                    "Prepared"
                } else if action == 3 {
                    "Released"
                } else {
                    "Committed"
                },
                &expected,
            );
            ZoneSaveOperationReceipt::from_json_checked(response.clone()).unwrap();
            let worker = thread::spawn(move || {
                let (mut stream, request) = accept(&socket, codec);
                assert!(matches!(request.request, ZoneRpcRequest::Health));
                respond(&mut stream, health(true), codec);
                drop(stream);
                let (mut stream, request) = accept(&socket, codec);
                assert_eq!(request.session_id, SESSION);
                assert_eq!(request.zone_id, ZONE);
                if action > 0 {
                    let value = serde_json::to_value(&request.request).unwrap();
                    let args = value["arguments"].as_object().unwrap();
                    assert_eq!(args.len(), 1);
                    assert_eq!(args["binding"], binding().to_json().unwrap());
                    assert_eq!(args["binding"].as_object().unwrap().len(), 5);
                }
                respond(
                    &mut stream,
                    ZoneRpcPayload::ZoneSaveOperation {
                        receipt: Some(response),
                    },
                    codec,
                );
            });
            let rpc = transport(address, codec);
            if action == 0 {
                let prepared = rpc
                    .prepare_zone_save_operation(&lease(), "save-operation")
                    .unwrap()
                    .unwrap();
                assert_eq!(prepared.remote_operation_binding(), Some(&expected));
            } else {
                let receipt = match action {
                    1 => rpc.persist_zone_save_operation(&expected).unwrap(),
                    2 => rpc.query_zone_save_operation(&expected).unwrap().unwrap(),
                    _ => rpc.release_zone_save_operation(&expected).unwrap(),
                };
                assert_eq!(receipt.operation().binding(), &expected);
                assert_eq!(receipt.account_store_version(), Some(4));
            }
            worker.join().unwrap();
        }
    }
}

#[test]
fn malformed_bindings_reject_before_capability_or_world_admission() {
    let authority = authority(true);
    let server = ZoneHostServer::with_options(
        source_config(),
        authority.clone(),
        Some("save-test-token".into()),
        limits(),
    );
    for (field, value) in [
        ("rpcSessionId", serde_json::json!("foreign-session")),
        ("zoneId", serde_json::json!("foreign-zone")),
        ("operationId", serde_json::json!("")),
        ("fencingToken", serde_json::json!(0)),
        ("fencingToken", serde_json::json!(u64::MAX)),
    ] {
        let mut wire = binding().to_json().unwrap();
        wire[field] = value;
        let wire: WireZoneSaveBinding = serde_json::from_value(wire).unwrap();
        for request in [
            ZoneRpcRequest::QueryZoneSaveOperation {
                binding: wire.clone(),
            },
            ZoneRpcRequest::PersistZoneSaveOperation {
                binding: wire.clone(),
            },
            ZoneRpcRequest::ReleaseZoneSaveOperation { binding: wire },
        ] {
            assert_eq!(
                server.handle_envelope(envelope(request)).unwrap_err().code,
                "invalid_request"
            );
        }
    }
    let mut wire = binding().to_json().unwrap();
    wire["checkpoint"] = serde_json::json!({"gold": 99999});
    assert!(serde_json::from_value::<WireZoneSaveBinding>(wire).is_err());
    for (operation_id, token) in [("", 7), ("save-operation", 0), ("save-operation", u64::MAX)] {
        let mut owner_lease = WireZoneOwnerLease::from(&lease());
        owner_lease.fencing_token = token;
        assert_eq!(
            server
                .handle_envelope(envelope(ZoneRpcRequest::PrepareZoneSaveOperation {
                    owner_lease,
                    operation_id: operation_id.into(),
                }))
                .unwrap_err()
                .code,
            "invalid_request"
        );
    }
    assert_eq!(server.session_count(), 0);
    assert_eq!(authority.world_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn malformed_or_foreign_receipt_never_establishes_commitment() {
    let socket = listener();
    let rpc = transport(socket.local_addr().unwrap(), ZoneRpcCodec::Json);
    let expected = binding();
    let mut foreign = receipt_json("Committed", &expected);
    foreign["operation"]["binding"]["operationId"] = serde_json::json!("foreign-operation");
    assert!(rpc
        .decode_zone_save_receipt(
            ZoneRpcPayload::ZoneSaveOperation {
                receipt: Some(foreign)
            },
            &expected
        )
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    let mut changed = receipt_json("Committed", &expected);
    changed["operation"]["checkpointDigest"] = serde_json::json!("tampered");
    assert!(rpc
        .decode_zone_save_receipt(
            ZoneRpcPayload::ZoneSaveOperation {
                receipt: Some(changed)
            },
            &expected
        )
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    assert!(rpc
        .decode_zone_save_receipt(ZoneRpcPayload::Unit, &expected)
        .unwrap_err()
        .contains("COMMIT_OUTCOME_UNKNOWN"));
    assert_no_connection(&socket);
}

#[test]
fn query_is_readonly_control_and_mutations_cannot_use_retrying_helper() {
    let socket = listener();
    let rpc = transport(socket.local_addr().unwrap(), ZoneRpcCodec::Json);
    let wire = WireZoneSaveBinding::from(&binding());
    let query = ZoneRpcRequest::QueryZoneSaveOperation {
        binding: wire.clone(),
    };
    assert!(!zone_rpc_request_requires_active_mutation(&query));
    assert!(!zone_rpc_request_requires_active_session(&query));
    assert_eq!(query.priority(), ZoneRpcPriority::Control);
    for request in [
        ZoneRpcRequest::PrepareZoneSaveOperation {
            owner_lease: (&lease()).into(),
            operation_id: "save-operation".into(),
        },
        ZoneRpcRequest::PersistZoneSaveOperation {
            binding: wire.clone(),
        },
        ZoneRpcRequest::ReleaseZoneSaveOperation { binding: wire },
    ] {
        assert!(zone_rpc_request_requires_active_mutation(&request));
        assert!(zone_rpc_request_requires_active_session(&request));
        assert_eq!(request.priority(), ZoneRpcPriority::Control);
        assert!(rpc.call(request).unwrap_err().contains("single-attempt"));
    }
    assert_no_connection(&socket);
}

#[test]
fn unknown_fault_classification_freezes_only_the_affected_zone_and_query_checks_auth_first() {
    let authority = authority(false);
    let server = ZoneHostServer::with_options(
        GatewayConfig::default(),
        authority.clone(),
        Some("save-test-token".into()),
        limits(),
    );
    let error = server.classify_zone_runtime_error(
        ZONE,
        "ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN: reply lost".into(),
    );
    assert_eq!(error.code, "commit_outcome_unknown");
    assert_eq!(
        server
            .unknown_commit_frozen_zones
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec![ZONE.to_string()]
    );
    assert_eq!(
        server
            .classify_zone_runtime_error("other-zone", "validation failed".into())
            .code,
        "runtime"
    );
    assert!(!server
        .unknown_commit_frozen_zones
        .lock()
        .unwrap()
        .contains("other-zone"));
    server.quiesced_zones.lock().unwrap().insert(ZONE.into());
    let query = || {
        envelope(ZoneRpcRequest::QueryZoneSaveOperation {
            binding: (&binding()).into(),
        })
    };
    // Even in a frozen/quiesced Zone this reaches only capability validation,
    // before any runtime/lease admission. Actual old-epoch DB reads are PG tests.
    assert_eq!(
        server.handle_envelope(query()).unwrap_err().code,
        "unsupported_capability"
    );
    let mut unauthorized = query();
    unauthorized.auth_token = Some("wrong-token".into());
    assert_eq!(
        server.handle_envelope(unauthorized).unwrap_err().code,
        "unauthorized"
    );
    assert_eq!(server.session_count(), 0);
    assert_eq!(authority.world_calls.load(Ordering::SeqCst), 0);
}

//! Private loopback GameShop gift boundaries. Prepared wallets are test fixtures,
//! not payment-provider or installed-client acceptance.
use super::*;
use crate::routing::ZoneOwnerLeaseAuthority;
use crate::InMemoryZoneOwnerLeaseAuthority;
use mir2_simulation::{InProcessWorldRuntime, Stage5SystemsState, WorldRuntime};

const PAYER: &str = "gift_rpc_payer";
const RECEIVER: &str = "gift_rpc_receiver";
const PAYER_INDEX: i32 = 71;
const RECEIVER_INDEX: i32 = 72;
const RECEIVER_NAME: &str = "RpcGiftFriend";
const PRIVATE_TOKEN: &str = "gift-rpc-private-fixture";

struct SourceFixture {
    config: GatewayConfig,
    source: Arc<ZoneHostServer>,
    lease: ZoneOwnerLease,
    session_id: String,
}

impl SourceFixture {
    fn new(label: &str) -> Self {
        let config = GatewayConfig::default()
            .with_billing_monthly_card_credit_price(Some(10))
            .unwrap();
        {
            let mut store = config.account_store.lock().unwrap();
            for (account_id, index, name, credit) in [
                (PAYER, PAYER_INDEX, "RpcGiftPayer", 100),
                (RECEIVER, RECEIVER_INDEX, RECEIVER_NAME, 7),
            ] {
                let mut character = config.default_character.clone();
                character.index = index;
                character.name = name.into();
                let mut account = AccountRecord::new(character);
                account.password = "gift-rpc-fixture-password".into();
                let save = account.saves.get_mut(&index).unwrap();
                save.credit = credit;
                save.gold = 321;
                store.accounts.insert(account_id.into(), account);
            }
            store.next_character_index = 100;
        }
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let lease = authority.owner_lease(&ZoneId::primary());
        let source = Arc::new(ZoneHostServer::with_options_and_factory(
            config.clone(),
            authority,
            Some(PRIVATE_TOKEN.into()),
            ZoneRpcLimits::default(),
            Arc::new(SharedInProcessZoneRuntimeFactory::with_tick_cadences(
                Duration::from_secs(3600),
                BTreeMap::new(),
            )),
        ));
        Self {
            config,
            source,
            lease,
            session_id: format!("gift-rpc-{label}"),
        }
    }

    fn envelope(&self, request: ZoneRpcRequest) -> ZoneRpcEnvelope {
        ZoneRpcEnvelope {
            protocol_version: ZONE_RPC_PROTOCOL_VERSION,
            session_id: self.session_id.clone(),
            zone_id: ZoneId::primary().as_str().into(),
            auth_token: Some(PRIVATE_TOKEN.into()),
            request,
        }
    }

    fn command_envelope(&self, command: WorldCommand) -> ZoneRpcEnvelope {
        self.envelope(ZoneRpcRequest::Execute {
            owner_lease: WireZoneOwnerLease::from(&self.lease),
            mode: WireZoneOwnerCommandMode::from(ZoneOwnerCommandMode::ProductionPlayer {
                authenticated: true,
            }),
            command: WireWorldCommand::from_world(command).unwrap(),
        })
    }

    fn login_and_start(&self) {
        for command in [
            WorldCommand::ClientPacket(ClientPacket::Login {
                account_id: PAYER.into(),
                password: "gift-rpc-fixture-password".into(),
            }),
            WorldCommand::ClientPacket(ClientPacket::StartGame {
                character_index: PAYER_INDEX,
            }),
        ] {
            let payload = self
                .source
                .handle_envelope(self.command_envelope(command))
                .unwrap();
            let ZoneRpcPayload::Execution { frames, .. } = payload else {
                panic!("Source must return execution");
            };
            let packets = decode_server_frames(frames).unwrap();
            assert!(packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::LoginSuccess { .. } | ServerPacket::StartGame { result: 4, .. }
            )));
        }
    }

    fn gift(&self) -> NativeGameShopGiftRequest {
        NativeGameShopGiftRequest {
            purchase: NativeGameShopPurchaseRequest {
                protocol_version: mir2_simulation::NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
                server_idempotency_key: base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode([7; 32]),
                gateway_session_id: self.session_id.clone(),
                account_id: PAYER.into(),
                character_index: PAYER_INDEX,
                client_request_id: "gs-gift-rpc-one".into(),
                g_index: mir2_game_data::BILLING_MONTHLY_CARD_GAME_SHOP_INDEX,
                quantity: 1,
                price_type: 0,
            },
            recipient_name: RECEIVER_NAME.into(),
        }
    }

    fn request(&self) -> ZoneOwnerCommandRequest {
        ZoneOwnerCommandRequest::production_player(
            self.lease.clone(),
            true,
            WorldCommand::NativeGameShopGift(self.gift()),
        )
    }

    fn transport(&self, addresses: Vec<String>, codec: ZoneRpcCodec) -> TcpZoneOwnerRpcTransport {
        let mut limits = ZoneRpcLimits::default();
        limits.io_timeout = Duration::from_secs(3);
        let mut transport = TcpZoneOwnerRpcTransport::with_endpoints(
            addresses,
            ZoneId::primary(),
            &self.session_id,
            Some(PRIVATE_TOKEN.into()),
            limits,
        )
        .unwrap();
        transport.codec = codec;
        transport
    }

    fn assert_single_recipient_delivery(&self) {
        let store = self.config.account_store.lock().unwrap();
        assert_eq!(store.accounts[PAYER].saves[&PAYER_INDEX].credit, 90);
        assert_eq!(store.accounts[PAYER].saves[&PAYER_INDEX].gold, 321);
        assert_eq!(store.accounts[RECEIVER].saves[&RECEIVER_INDEX].credit, 7);
        assert_eq!(store.accounts[RECEIVER].saves[&RECEIVER_INDEX].gold, 321);
        for (account, index, expected) in [(PAYER, PAYER_INDEX, 0), (RECEIVER, RECEIVER_INDEX, 1)] {
            let save = &store.accounts[account].saves[&index];
            let systems: Stage5SystemsState =
                serde_json::from_str(save.stage5_systems_json.as_deref().unwrap_or("{}")).unwrap();
            assert_eq!(
                systems
                    .mail
                    .iter()
                    .filter(|mail| mail.subject == "Game shop gift" && !mail.deleted)
                    .count(),
                expected
            );
        }
        let card = store.accounts[RECEIVER].monthly_card.as_ref().unwrap();
        assert_eq!(card.item_receipts.len(), 1);
        assert!(card
            .item_receipts
            .values()
            .all(|receipt| receipt.redeemed_at_ms.is_none()));
    }
}

fn accept_bounded(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                return stream;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "gift TCP request was not sent");
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("gift loopback accept failed: {error}"),
        }
    }
}

fn answer_health(
    source: &ZoneHostServer,
    listener: &TcpListener,
    codec: ZoneRpcCodec,
    remove_gift_capability: bool,
) {
    let mut stream = accept_bounded(listener);
    let frame = read_frame(&mut stream, 64 * 1024).unwrap();
    let envelope = decode_rpc_envelope(&frame, codec).unwrap();
    assert!(matches!(envelope.request, ZoneRpcRequest::Health));
    let mut payload = source.handle_envelope(envelope).unwrap();
    let ZoneRpcPayload::Health { capabilities, .. } = &mut payload else {
        panic!("Source health payload");
    };
    assert!(capabilities
        .iter()
        .any(|value| value == ZONE_RPC_NATIVE_GAME_SHOP_GIFT_V1));
    if remove_gift_capability {
        capabilities.retain(|value| value != ZONE_RPC_NATIVE_GAME_SHOP_GIFT_V1);
        assert!(capabilities
            .iter()
            .any(|value| value == ZONE_RPC_NATIVE_GAME_SHOP_PURCHASE_V2));
    }
    let response = encode_rpc_response(
        &ZoneRpcResponse::Ok {
            payload: Box::new(payload),
        },
        codec,
    )
    .unwrap();
    write_frame(&mut stream, &response, 64 * 1024).unwrap();
}

#[test]
fn gift_world_and_wire_roundtrip_distinct_mutation_in_both_codecs() {
    let fixture = SourceFixture::new("wire");
    let original = fixture.gift();
    let wire =
        WireWorldCommand::from_world(WorldCommand::NativeGameShopGift(original.clone())).unwrap();
    assert_eq!(
        correlated_mutation_policy(&WorldCommand::NativeGameShopGift(original.clone())),
        Some(CorrelatedMutationPolicy::NativeGiftV1)
    );
    let value = serde_json::to_value(&wire).unwrap();
    assert_eq!(value["command"], "nativeGameShopGiftV1");
    for decoded in [
        serde_json::from_slice::<WireWorldCommand>(&serde_json::to_vec(&wire).unwrap()).unwrap(),
        rmp_serde::from_slice::<WireWorldCommand>(&rmp_serde::to_vec_named(&wire).unwrap())
            .unwrap(),
    ] {
        assert_eq!(
            wire_correlated_mutation_policy(&decoded).unwrap(),
            Some(CorrelatedMutationPolicy::NativeGiftV1)
        );
        let WorldCommand::NativeGameShopGift(request) = decoded.into_world().unwrap() else {
            panic!("a gift must not decode as a self-purchase");
        };
        assert_eq!(request, original);
    }
}

#[test]
fn gift_raw_common_call_rejects_before_any_socket_io() {
    let fixture = SourceFixture::new("common-guard");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let transport = fixture.transport(
        vec![listener.local_addr().unwrap().to_string()],
        ZoneRpcCodec::Json,
    );
    let envelope = fixture.command_envelope(WorldCommand::NativeGameShopGift(fixture.gift()));
    let error = transport.call(envelope.request).unwrap_err();
    assert!(error.contains("single-attempt executor"), "{error}");
    assert!(matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
}

#[test]
fn purchase_capability_does_not_allow_gift_in_generic_or_typed_path() {
    for typed in [false, true] {
        let fixture = SourceFixture::new(if typed { "old-typed" } else { "old-generic" });
        fixture.login_and_start();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let source = Arc::clone(&fixture.source);
        let host = thread::spawn(move || {
            answer_health(&source, &listener, ZoneRpcCodec::Json, true);
            listener.set_nonblocking(true).unwrap();
            assert!(
                matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
                "no Execute or downgraded buy may reach an unsupported host"
            );
        });
        let transport = fixture.transport(vec![address], ZoneRpcCodec::Json);
        let result = if typed {
            transport.execute_requiring_typed_game_shop_purchase_outcome(fixture.request())
        } else {
            transport.execute(fixture.request())
        };
        let error = result.unwrap_err();
        assert!(error.contains(ZONE_RPC_NATIVE_GAME_SHOP_GIFT_V1), "{error}");
        host.join().unwrap();
        let store = fixture.config.account_store.lock().unwrap();
        assert_eq!(store.accounts[PAYER].saves[&PAYER_INDEX].credit, 100);
        assert!(store.accounts[RECEIVER].monthly_card.is_none());
    }
}

#[test]
fn capable_source_gift_commits_recipient_parcel_and_typed_mail_outcome_over_tcp() {
    for codec in [ZoneRpcCodec::Json, ZoneRpcCodec::MessagePack] {
        let fixture = SourceFixture::new(if codec == ZoneRpcCodec::Json {
            "json"
        } else {
            "msgpack"
        });
        fixture.login_and_start();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let source = Arc::clone(&fixture.source);
        let host = thread::spawn(move || {
            answer_health(&source, &listener, codec, false);
            let mut stream = accept_bounded(&listener);
            let frame = read_frame(&mut stream, 64 * 1024).unwrap();
            let envelope = decode_rpc_envelope(&frame, codec).unwrap();
            assert!(matches!(
                envelope.request,
                ZoneRpcRequest::Execute {
                    command: WireWorldCommand::NativeGameShopGiftV1 { .. },
                    ..
                }
            ));
            let payload = source.handle_envelope(envelope).unwrap();
            let response = encode_rpc_response(
                &ZoneRpcResponse::Ok {
                    payload: Box::new(payload),
                },
                codec,
            )
            .unwrap();
            write_frame(&mut stream, &response, 64 * 1024).unwrap();
        });
        let transport = fixture.transport(vec![address], codec);
        let execution = transport
            .execute_requiring_typed_game_shop_purchase_outcome(fixture.request())
            .unwrap();
        let outcome = execution.game_shop_purchase_outcome.unwrap();
        assert!(outcome.success, "{outcome:?}");
        assert!(outcome.mail_id.is_some());
        assert_eq!(outcome.price_type, 0);
        host.join().unwrap();
        fixture.assert_single_recipient_delivery();
    }
}

#[test]
fn gift_response_loss_after_real_source_commit_never_connects_to_second_endpoint() {
    for typed in [false, true] {
        let fixture = SourceFixture::new(if typed { "loss-typed" } else { "loss-generic" });
        fixture.login_and_start();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let fallback = TcpListener::bind("127.0.0.1:0").unwrap();
        fallback.set_nonblocking(true).unwrap();
        let fallback_address = fallback.local_addr().unwrap().to_string();
        let source = Arc::clone(&fixture.source);
        let commits = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&commits);
        let host = thread::spawn(move || {
            answer_health(&source, &listener, ZoneRpcCodec::Json, false);
            let mut stream = accept_bounded(&listener);
            let frame = read_frame(&mut stream, 64 * 1024).unwrap();
            let envelope = decode_rpc_envelope(&frame, ZoneRpcCodec::Json).unwrap();
            assert!(matches!(
                envelope.request,
                ZoneRpcRequest::Execute {
                    command: WireWorldCommand::NativeGameShopGiftV1 { .. },
                    ..
                }
            ));
            let payload = source.handle_envelope(envelope).unwrap();
            assert!(matches!(
                payload,
                ZoneRpcPayload::Execution {
                    game_shop_purchase_outcome: Some(GameShopPurchaseOutcome { success: true, .. }),
                    ..
                }
            ));
            observed.fetch_add(1, Ordering::SeqCst);
            // The real Source transaction has committed; deliberately lose its reply.
            drop(stream);
        });
        let transport = fixture.transport(vec![address, fallback_address], ZoneRpcCodec::Json);
        let result = if typed {
            transport.execute_requiring_typed_game_shop_purchase_outcome(fixture.request())
        } else {
            transport.execute(fixture.request())
        };
        host.join().unwrap();
        let error = result.unwrap_err();
        assert!(error.contains("commit state is unknown"), "{error}");
        assert!(error.contains("no endpoint fallback"), "{error}");
        assert_eq!(commits.load(Ordering::SeqCst), 1);
        assert!(
            matches!(fallback.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock),
            "not even a capability probe may connect to the fallback after Execute"
        );
        fixture.assert_single_recipient_delivery();
    }
}

struct OldWorldRuntime(InProcessWorldRuntime);
impl WorldRuntime for OldWorldRuntime {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn on_connect(&self) -> Vec<ServerPacket> {
        self.0.on_connect()
    }
    fn execute(&mut self, command: WorldCommand) -> Result<Vec<ServerPacket>, String> {
        self.0.execute(command)
    }
    fn supports_typed_game_shop_purchase_outcome(&self) -> bool {
        true
    }
    fn world_snapshot(&self) -> WorldSnapshot {
        self.0.world_snapshot()
    }
    fn active_identity(&self) -> Option<ActiveSessionIdentity> {
        self.0.active_identity()
    }
    fn save_active_character(&mut self) -> Result<(), String> {
        self.0.save_active_character()
    }
    fn refresh_active_external_mail(&mut self) -> bool {
        self.0.refresh_active_external_mail()
    }
    // Deliberately inherits the old/default false gift capability.
}

#[test]
fn source_health_gift_capability_requires_exact_existing_opted_in_runtime() {
    let fixture = SourceFixture::new("health");
    let supports = |payload: ZoneRpcPayload| match payload {
        ZoneRpcPayload::Health { capabilities, .. } => capabilities
            .iter()
            .any(|value| value == ZONE_RPC_NATIVE_GAME_SHOP_GIFT_V1),
        _ => panic!("Health must return its own payload"),
    };
    assert!(!supports(
        fixture
            .source
            .handle_envelope(fixture.envelope(ZoneRpcRequest::Health))
            .unwrap()
    ));
    assert_eq!(
        fixture.source.session_count(),
        0,
        "Health must not create a Source Session"
    );
    fixture.login_and_start();
    assert!(supports(
        fixture
            .source
            .handle_envelope(fixture.envelope(ZoneRpcRequest::Health))
            .unwrap()
    ));
    let mut other = fixture.envelope(ZoneRpcRequest::Health);
    other.session_id = "unopened-gift-session".into();
    assert!(!supports(fixture.source.handle_envelope(other).unwrap()));
    let old = Arc::new(HostedZoneOwnerCommandClient::new(Box::new(
        OldWorldRuntime(InProcessWorldRuntime::new(fixture.config.clone())),
    )));
    fixture.source.sessions.lock().unwrap().insert(
        (
            fixture.session_id.clone(),
            ZoneId::primary().as_str().into(),
        ),
        Arc::new(ZoneHostSession::new(old, 16)),
    );
    assert!(
        !supports(
            fixture
                .source
                .handle_envelope(fixture.envelope(ZoneRpcRequest::Health))
                .unwrap()
        ),
        "buy support alone must not advertise Source gift support"
    );
}

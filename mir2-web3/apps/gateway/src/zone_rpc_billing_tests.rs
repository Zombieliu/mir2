//! Local private RPC billing boundaries. No Stripe or external repository calls.
use super::*;
use crate::routing::ZoneOwnerLeaseAuthority;
use crate::InMemoryZoneOwnerLeaseAuthority;
use mir2_simulation::billing::{RechargeOffer, RechargeOrderState, VerifiedRechargePayment};
use mir2_simulation::monthly_card::monthly_card_now_ms;

const ACCOUNT: &str = "billingrpc";
const CHARACTER: i32 = 8;
const PRIVATE_TOKEN: &str = "billing-private-loopback-fixture";

#[derive(Debug, Clone, Copy)]
enum BillingCase {
    Refresh,
    Buy,
    Activate,
}

impl BillingCase {
    fn command(self, unique_id: u64) -> WorldCommand {
        match self {
            Self::Refresh => WorldCommand::BillingRefresh {
                character_index: CHARACTER,
            },
            Self::Buy => WorldCommand::BillingBuyMonthlyCard {
                character_index: CHARACTER,
                request_id: "private-rpc-card".into(),
            },
            Self::Activate => WorldCommand::BillingActivateMonthlyCard {
                character_index: CHARACTER,
                unique_id,
            },
        }
    }
}

struct SourceFixture {
    config: GatewayConfig,
    server: Arc<ZoneHostServer>,
    lease: ZoneOwnerLease,
    session_id: String,
}

impl SourceFixture {
    fn new(label: &str) -> Self {
        let config = GatewayConfig::default()
            .with_billing_monthly_card_credit_price(Some(10))
            .unwrap();
        let mut character = config.default_character.clone();
        character.index = CHARACTER;
        character.name = "BillingRpc".into();
        let mut account = AccountRecord::new(character);
        account.password = "fixture-password".into();
        account.saves.get_mut(&CHARACTER).unwrap().credit = 100;
        {
            let mut store = config.account_store.lock().unwrap();
            store.next_character_index = CHARACTER + 1;
            store.accounts.insert(ACCOUNT.into(), account);
        }
        let authority = Arc::new(InMemoryZoneOwnerLeaseAuthority::new());
        let lease = authority.owner_lease(&ZoneId::primary());
        let server = Arc::new(ZoneHostServer::with_options_and_factory(
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
            server,
            lease,
            session_id: format!("billing-rpc-{label}"),
        }
    }

    fn envelope(&self, command: WorldCommand) -> ZoneRpcEnvelope {
        ZoneRpcEnvelope {
            protocol_version: ZONE_RPC_PROTOCOL_VERSION,
            session_id: self.session_id.clone(),
            zone_id: ZoneId::primary().as_str().into(),
            auth_token: Some(PRIVATE_TOKEN.into()),
            request: ZoneRpcRequest::Execute {
                owner_lease: WireZoneOwnerLease::from(&self.lease),
                mode: WireZoneOwnerCommandMode::from(ZoneOwnerCommandMode::Direct),
                command: WireWorldCommand::from_world(command).unwrap(),
            },
        }
    }

    fn login_and_start_in_source(&self) {
        for command in [login_command(), start_command()] {
            let payload = self.server.handle_envelope(self.envelope(command)).unwrap();
            let ZoneRpcPayload::Execution { frames, .. } = payload else {
                panic!("Source execution payload");
            };
            let packets = decode_server_frames(frames).unwrap();
            assert!(
                packets.iter().any(|packet| matches!(
                    packet,
                    ServerPacket::LoginSuccess { .. } | ServerPacket::StartGame { result: 4, .. }
                )),
                "Source fixture login/start failed: {packets:?}"
            );
        }
        assert_eq!(self.server.session_count(), 1);
    }

    fn paid_order(&self) -> String {
        let now = monthly_card_now_ms();
        let order = self
            .config
            .prepare_recharge_order(
                ACCOUNT,
                CHARACTER,
                "private-rpc-recharge",
                &RechargeOffer {
                    id: "credits-25".into(),
                    label: "Fixture Credits".into(),
                    currency: "usd".into(),
                    amount_minor: 500,
                    credits: 25,
                },
                now,
                false,
            )
            .unwrap();
        self.config
            .confirm_recharge_payment(
                ACCOUNT,
                &order.id,
                &VerifiedRechargePayment {
                    session_id: "cs_privatebillingfixture".into(),
                    payment_intent_id: "pi_privatebillingfixture".into(),
                    amount_minor: 500,
                    currency: "usd".into(),
                    livemode: false,
                },
                now,
            )
            .unwrap();
        order.id
    }

    fn transport(&self, addresses: Vec<String>, token: &str) -> TcpZoneOwnerRpcTransport {
        let mut limits = ZoneRpcLimits::default();
        limits.io_timeout = Duration::from_secs(5);
        let mut transport = TcpZoneOwnerRpcTransport::with_endpoints(
            addresses,
            ZoneId::primary(),
            self.session_id.clone(),
            Some(token.into()),
            limits,
        )
        .unwrap();
        transport.codec = ZoneRpcCodec::Json;
        transport
    }
}

fn login_command() -> WorldCommand {
    WorldCommand::ClientPacket(ClientPacket::Login {
        account_id: ACCOUNT.into(),
        password: "fixture-password".into(),
    })
}

fn start_command() -> WorldCommand {
    WorldCommand::ClientPacket(ClientPacket::StartGame {
        character_index: CHARACTER,
    })
}

fn execute(
    transport: &TcpZoneOwnerRpcTransport,
    lease: &ZoneOwnerLease,
    command: WorldCommand,
) -> WorldCommandExecution {
    transport
        .execute(ZoneOwnerCommandRequest::direct(lease.clone(), command))
        .unwrap()
}

fn accept_bounded(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                return stream;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "billing TCP request was never sent"
                );
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("billing loopback accept failed: {error}"),
        }
    }
}

#[test]
fn billing_world_and_wire_classifiers_preserve_single_attempt_in_both_codecs() {
    for case in [
        BillingCase::Refresh,
        BillingCase::Buy,
        BillingCase::Activate,
    ] {
        assert_eq!(
            correlated_mutation_policy(&case.command(123)),
            Some(CorrelatedMutationPolicy::Billing)
        );
        let wire = WireWorldCommand::from_world(case.command(123)).unwrap();
        assert_eq!(
            wire_correlated_mutation_policy(&wire).unwrap(),
            Some(CorrelatedMutationPolicy::Billing)
        );
        for decoded in [
            serde_json::from_slice::<WireWorldCommand>(&serde_json::to_vec(&wire).unwrap())
                .unwrap(),
            rmp_serde::from_slice::<WireWorldCommand>(&rmp_serde::to_vec_named(&wire).unwrap())
                .unwrap(),
        ] {
            assert_eq!(
                wire_correlated_mutation_policy(&decoded).unwrap(),
                Some(CorrelatedMutationPolicy::Billing)
            );
            assert_eq!(
                correlated_mutation_policy(&decoded.into_world().unwrap()),
                Some(CorrelatedMutationPolicy::Billing)
            );
        }
    }
    assert_eq!(correlated_mutation_policy(&WorldCommand::Tick), None);
}

#[test]
fn billing_raw_common_call_rejects_all_three_commands_before_network_io() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let zone = ZoneId::primary();
    let mut limits = ZoneRpcLimits::default();
    limits.io_timeout = Duration::from_millis(200);
    let mut transport = TcpZoneOwnerRpcTransport::with_options(
        listener.local_addr().unwrap().to_string(),
        zone.clone(),
        "billing-common-call-guard",
        Some(PRIVATE_TOKEN.into()),
        limits,
    );
    transport.codec = ZoneRpcCodec::Json;
    let lease = ZoneOwnerLease::new(zone, "in-process", 1);
    for case in [
        BillingCase::Refresh,
        BillingCase::Buy,
        BillingCase::Activate,
    ] {
        let error = transport
            .call(ZoneRpcRequest::Execute {
                owner_lease: WireZoneOwnerLease::from(&lease),
                mode: WireZoneOwnerCommandMode::from(ZoneOwnerCommandMode::Direct),
                command: WireWorldCommand::from_world(case.command(123)).unwrap(),
            })
            .unwrap_err();
        assert!(
            error.contains("single-attempt executor"),
            "{case:?}: {error}"
        );
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == io::ErrorKind::WouldBlock)
        );
    }
}

#[test]
fn billing_raw_player_commands_are_rejected_even_when_authenticated() {
    for case in [
        BillingCase::Refresh,
        BillingCase::Buy,
        BillingCase::Activate,
    ] {
        for authenticated in [false, true] {
            let error = mir2_simulation::validate_production_player_command(
                authenticated,
                &case.command(123),
            )
            .unwrap_err();
            assert!(
                error.contains("authenticated billing seam"),
                "{case:?}: {error}"
            );
        }
    }
}

fn response_loss_after_source_commit(case: BillingCase) {
    let fixture = SourceFixture::new(&format!("loss-{case:?}"));
    let unique_id = if matches!(case, BillingCase::Activate) {
        fixture
            .config
            .buy_monthly_card_for_character(
                ACCOUNT,
                CHARACTER,
                "activation-initial-item",
                monthly_card_now_ms(),
            )
            .unwrap()
            .unique_id
    } else {
        0
    };
    fixture.login_and_start_in_source();
    let order_id = if matches!(case, BillingCase::Refresh) {
        Some(fixture.paid_order())
    } else {
        None
    };
    let committing_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let committing_address = committing_listener.local_addr().unwrap();
    let fallback_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    fallback_listener.set_nonblocking(true).unwrap();
    let fallback_address = fallback_listener.local_addr().unwrap();
    let committed = Arc::new(AtomicUsize::new(0));
    let observed_commit = Arc::clone(&committed);
    let source = Arc::clone(&fixture.server);
    let committing_server = thread::spawn(move || {
        let mut health_stream = accept_bounded(&committing_listener);
        let bytes = read_frame(&mut health_stream, 64 * 1024).unwrap();
        let health_envelope = decode_rpc_envelope(&bytes, ZoneRpcCodec::Json).unwrap();
        assert!(matches!(health_envelope.request, ZoneRpcRequest::Health));
        let health = source.handle_envelope(health_envelope).unwrap();
        assert!(
            matches!(&health, ZoneRpcPayload::Health { capabilities, .. }
            if capabilities.iter().any(|capability| capability == ZONE_RPC_BILLING_V1))
        );
        let response = encode_rpc_response(
            &ZoneRpcResponse::Ok {
                payload: Box::new(health),
            },
            ZoneRpcCodec::Json,
        )
        .unwrap();
        write_frame(&mut health_stream, &response, 64 * 1024).unwrap();
        drop(health_stream);
        let mut stream = accept_bounded(&committing_listener);
        let bytes = read_frame(&mut stream, 64 * 1024).unwrap();
        let envelope = decode_rpc_envelope(&bytes, ZoneRpcCodec::Json).unwrap();
        let ZoneRpcRequest::Execute { command, .. } = &envelope.request else {
            panic!("billing must send one Execute after the Source capability probe");
        };
        assert_eq!(
            wire_correlated_mutation_policy(command).unwrap(),
            Some(CorrelatedMutationPolicy::Billing)
        );
        // Execute the real authenticated Source transaction, then deliberately
        // lose the response. This is not a fabricated success packet.
        let outcome = source.handle_envelope(envelope).unwrap();
        assert!(matches!(outcome, ZoneRpcPayload::Execution { .. }));
        observed_commit.fetch_add(1, Ordering::SeqCst);
        drop(stream);
    });
    let transport = fixture.transport(
        vec![committing_address.to_string(), fallback_address.to_string()],
        PRIVATE_TOKEN,
    );
    let result = transport.execute(ZoneOwnerCommandRequest::direct(
        fixture.lease.clone(),
        case.command(unique_id),
    ));
    committing_server.join().unwrap();
    assert_eq!(committed.load(Ordering::SeqCst), 1);
    assert!(
        matches!(fallback_listener.accept(),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock),
        "a committed billing command reached a fallback socket"
    );
    let error = result.expect_err("a lost financial response is an unknown outcome");
    assert!(error.contains("commit state is unknown"), "{error}");
    assert!(error.contains("no endpoint fallback"), "{error}");
    match case {
        BillingCase::Refresh => {
            assert_eq!(
                fixture
                    .config
                    .recharge_credit_status(ACCOUNT, CHARACTER)
                    .unwrap(),
                (125, 0)
            );
            let orders = fixture.config.recharge_orders(ACCOUNT, CHARACTER).unwrap();
            let order = orders
                .iter()
                .find(|order| Some(&order.id) == order_id.as_ref())
                .unwrap();
            assert_eq!(order.state, RechargeOrderState::Applied);
            assert_eq!(order.applied.as_ref().unwrap().credits, 25);
        }
        BillingCase::Buy => {
            assert_eq!(
                fixture
                    .config
                    .recharge_credit_status(ACCOUNT, CHARACTER)
                    .unwrap(),
                (90, 0)
            );
            assert_eq!(
                fixture
                    .config
                    .list_billing_monthly_cards(ACCOUNT, CHARACTER)
                    .unwrap()
                    .len(),
                1
            );
        }
        BillingCase::Activate => {
            assert!(
                fixture
                    .config
                    .monthly_card_status(ACCOUNT, monthly_card_now_ms())
                    .unwrap()
                    .can_enter_game
            );
            assert!(fixture
                .config
                .list_billing_monthly_cards(ACCOUNT, CHARACTER)
                .unwrap()
                .is_empty());
            let store = fixture.config.account_store.lock().unwrap();
            let card = store.accounts[ACCOUNT].monthly_card.as_ref().unwrap();
            assert!(card.expires_at_ms > monthly_card_now_ms());
            assert!(card
                .item_receipts
                .values()
                .all(|item| item.redeemed_at_ms.is_some()));
        }
    }
}

#[test]
fn billing_refresh_tcp_response_loss_never_replays_after_source_credit_commit() {
    response_loss_after_source_commit(BillingCase::Refresh);
}

#[test]
fn billing_buy_tcp_response_loss_never_replays_after_source_item_commit() {
    response_loss_after_source_commit(BillingCase::Buy);
}

#[test]
fn billing_activate_tcp_response_loss_never_replays_after_source_access_commit() {
    response_loss_after_source_commit(BillingCase::Activate);
}

#[test]
fn billing_old_host_without_capability_rejects_all_three_before_source_mutation() {
    let fixture = SourceFixture::new("old-host-capability");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let source = Arc::clone(&fixture.server);
    let old_host = thread::spawn(move || {
        for _ in 0..3 {
            let mut stream = accept_bounded(&listener);
            let bytes = read_frame(&mut stream, 64 * 1024).unwrap();
            let envelope = decode_rpc_envelope(&bytes, ZoneRpcCodec::Json).unwrap();
            assert!(
                matches!(envelope.request, ZoneRpcRequest::Health),
                "an unsupported old Host must receive no financial Execute"
            );
            let mut payload = source.handle_envelope(envelope).unwrap();
            let ZoneRpcPayload::Health { capabilities, .. } = &mut payload else {
                panic!("Source health response");
            };
            // Retain the real Source health shape but advertise the old Host's
            // capabilities. The transport must fail before issuing a new wire.
            capabilities.retain(|capability| capability != ZONE_RPC_BILLING_V1);
            let response = encode_rpc_response(
                &ZoneRpcResponse::Ok {
                    payload: Box::new(payload),
                },
                ZoneRpcCodec::Json,
            )
            .unwrap();
            write_frame(&mut stream, &response, 64 * 1024).unwrap();
        }
    });
    let transport = fixture.transport(vec![address], PRIVATE_TOKEN);
    for case in [
        BillingCase::Refresh,
        BillingCase::Buy,
        BillingCase::Activate,
    ] {
        let error = transport
            .execute(ZoneOwnerCommandRequest::direct(
                fixture.lease.clone(),
                case.command(123),
            ))
            .unwrap_err();
        assert!(error.contains(ZONE_RPC_BILLING_V1), "{case:?}: {error}");
        assert!(
            error.contains("unavailable before execution"),
            "{case:?}: {error}"
        );
    }
    old_host.join().unwrap();
    assert_eq!(fixture.server.session_count(), 0);
    assert_eq!(
        fixture
            .config
            .recharge_credit_status(ACCOUNT, CHARACTER)
            .unwrap(),
        (100, 0)
    );
    let store = fixture.config.account_store.lock().unwrap();
    assert!(store.accounts[ACCOUNT].billing.is_none());
    assert!(store.accounts[ACCOUNT].monthly_card.is_none());
}

struct PrivateServer {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for PrivateServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

#[test]
fn billing_private_tcp_source_roundtrip_requires_token_login_and_owned_character() {
    let fixture = SourceFixture::new("roundtrip");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    let source = Arc::clone(&fixture.server);
    let stopping = Arc::clone(&stop);
    let _server = PrivateServer {
        stop,
        thread: Some(thread::spawn(move || {
            source.serve_until(listener, stopping).unwrap()
        })),
    };
    let wrong_token = fixture.transport(vec![address.clone()], "wrong-fixture-token");
    let unauthorized = wrong_token
        .execute(ZoneOwnerCommandRequest::direct(
            fixture.lease.clone(),
            BillingCase::Refresh.command(0),
        ))
        .unwrap_err();
    assert!(unauthorized.contains("unauthorized"));
    assert_eq!(fixture.server.session_count(), 0);
    let transport = fixture.transport(vec![address], PRIVATE_TOKEN);
    let unauthenticated = transport
        .execute(ZoneOwnerCommandRequest::direct(
            fixture.lease.clone(),
            BillingCase::Refresh.command(0),
        ))
        .unwrap_err();
    assert!(unauthenticated.contains("billingAuthenticationRequired"));
    execute(&transport, &fixture.lease, login_command());
    let started = execute(&transport, &fixture.lease, start_command());
    assert!(started
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(
        transport.active_identity().unwrap().unwrap().account_id,
        ACCOUNT
    );
    for case in [
        BillingCase::Refresh,
        BillingCase::Buy,
        BillingCase::Activate,
    ] {
        let rejected = transport
            .execute(ZoneOwnerCommandRequest::production_player(
                fixture.lease.clone(),
                true,
                case.command(123),
            ))
            .unwrap_err();
        assert!(
            rejected.contains("authenticated billing seam"),
            "{rejected}"
        );
    }
    assert_eq!(
        fixture
            .config
            .recharge_credit_status(ACCOUNT, CHARACTER)
            .unwrap(),
        (100, 0)
    );
    fixture.paid_order();
    let refreshed = execute(&transport, &fixture.lease, BillingCase::Refresh.command(0));
    assert_eq!(
        refreshed
            .packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::GainedCredit { credit: 25 }))
            .count(),
        1
    );
    assert_eq!(
        fixture
            .config
            .recharge_credit_status(ACCOUNT, CHARACTER)
            .unwrap(),
        (125, 0)
    );
    let repeated_refresh = execute(&transport, &fixture.lease, BillingCase::Refresh.command(0));
    assert!(!repeated_refresh
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedCredit { .. })));
    execute(&transport, &fixture.lease, BillingCase::Buy.command(0));
    let cards = fixture
        .config
        .list_billing_monthly_cards(ACCOUNT, CHARACTER)
        .unwrap();
    assert_eq!(cards.len(), 1);
    execute(&transport, &fixture.lease, BillingCase::Buy.command(0));
    assert_eq!(
        fixture
            .config
            .recharge_credit_status(ACCOUNT, CHARACTER)
            .unwrap(),
        (115, 0)
    );
    assert_eq!(
        fixture
            .config
            .list_billing_monthly_cards(ACCOUNT, CHARACTER)
            .unwrap(),
        cards
    );
    execute(
        &transport,
        &fixture.lease,
        BillingCase::Activate.command(cards[0].unique_id),
    );
    let first_expiry = fixture
        .config
        .monthly_card_status(ACCOUNT, monthly_card_now_ms())
        .unwrap()
        .expires_at_ms
        .unwrap();
    assert!(first_expiry > monthly_card_now_ms());
    execute(
        &transport,
        &fixture.lease,
        BillingCase::Activate.command(cards[0].unique_id),
    );
    assert_eq!(
        fixture
            .config
            .monthly_card_status(ACCOUNT, monthly_card_now_ms())
            .unwrap()
            .expires_at_ms,
        Some(first_expiry)
    );
    assert!(fixture
        .config
        .list_billing_monthly_cards(ACCOUNT, CHARACTER)
        .unwrap()
        .is_empty());
    let wrong_character = transport
        .execute(ZoneOwnerCommandRequest::direct(
            fixture.lease.clone(),
            WorldCommand::BillingRefresh {
                character_index: CHARACTER + 1,
            },
        ))
        .unwrap_err();
    assert!(wrong_character.contains("billingCharacterMismatch"));
    assert_eq!(
        fixture
            .config
            .recharge_credit_status(ACCOUNT, CHARACTER)
            .unwrap(),
        (115, 0)
    );
}

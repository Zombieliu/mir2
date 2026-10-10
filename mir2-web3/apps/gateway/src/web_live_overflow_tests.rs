use super::*;
use mir2_protocol::{MirClass, MirGender};
use mir2_simulation::{SimulationConfig, WorldEntityKind};
use tokio::sync::{oneshot, watch};

#[derive(Debug)]
struct HealthValidationProbe(Arc<std::sync::atomic::AtomicUsize>);

impl crate::routing::OwnerHealthValidator for HealthValidationProbe {
    fn validate(&self, _: &mir2_simulation::ZoneOwnerHealthChange) -> Result<bool, String> {
        match self.0.load(Ordering::Acquire) {
            0 => Ok(false), 1 => Ok(true), _ => Err("prepared validation transport failure".into()),
        }
    }
}

// This probe isolates actual WS scheduling and cancellation. Current-life
// Source validation is covered by the real Zone and authenticated RPC tests.
async fn health_sender_socket_case(validation: usize) -> (Vec<Value>, Result<(), transport::TransportEnd>) {
    let (done_tx, done_rx) = oneshot::channel::<()>();
    let (result_tx, result_rx) = oneshot::channel();
    let handoff = Arc::new(Mutex::new(Some((done_rx, result_tx))));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route("/ws", get(move |upgrade: WebSocketUpgrade| {
        let (done, result) = handoff.lock().unwrap().take().unwrap();
        async move { upgrade.on_upgrade(move |socket| async move {
            let (sink, _stream) = socket.split();
            let terminal = transport::TransportSignal::new();
            let sender = Arc::new(transport::SocketSender::new(sink, terminal.clone()));
            let gate = Arc::new(AsyncRwLock::new(()));
            let active = Arc::new(AtomicU64::new(42));
            let (failure_tx, failure_rx) = watch::channel(0);
            let work = async {
                let bootstrap = gate.write().await;
                let (normal_tx, normal_rx) = mpsc::channel(2);
                let (_owner_tx, owner_rx) = mpsc::channel(1);
                let _writer = spawn_zone_outbound_sender(normal_rx, owner_rx, sender.clone(),
                    gate.clone(), active.clone(), failure_tx);
                let decision = Arc::new(std::sync::atomic::AtomicUsize::new(1));
                let outbound = SharedZoneLiveOutbound::new(42, ServerPacket::HealthChanged { hp: 10, mp: 9 })
                    .with_owner_health(crate::routing::OwnerHealthDelivery {
                        change: mir2_simulation::ZoneOwnerHealthChange {
                            cursor: mir2_simulation::ZoneOwnerHealthCursor {
                                session_id: mir2_simulation::SessionId::new("web-health-probe"), online_owner: "private-probe".into(),
                                object_id: 1, life_generation: 1, dead: false, health_sequence: 1,
                            }, hp_before: 11, mp_before: 9, hp: 10, mp: 9,
                        }, validator: Arc::new(HealthValidationProbe(decision.clone())),
                        highest_sent_sequence: Arc::new(Mutex::new(0)),
                    });
                normal_tx.send(outbound).await.unwrap();
                tokio::time::timeout(Duration::from_secs(2), async {
                    while normal_tx.capacity() != 2 { tokio::task::yield_now().await; }
                }).await.unwrap();
                // The writer has already dequeued the packet, but cannot pass
                // the bootstrap gate. Change validation before releasing it.
                decision.store(validation, Ordering::Release);
                drop(bootstrap);
                normal_tx.send(SharedZoneLiveOutbound::new(42, ServerPacket::KeepAlive { time: 99 })).await.unwrap();
                let _ = done.await;
            };
            let outcome = run_until_transport_end(work, failure_rx, active.clone(), terminal).await;
            let _ = result.send(outcome);
        }) }
    }));
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });
    let (mut client, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws")).await.unwrap();
    let mut received = Vec::new();
    while let Ok(Some(Ok(message))) = tokio::time::timeout(Duration::from_secs(2), client.next()).await {
        if let tokio_tungstenite::tungstenite::Message::Text(text) = message {
            let value: Value = serde_json::from_str(&text).unwrap();
            let terminal = value["packet"] == "KeepAlive";
            received.push(value);
            if terminal { break; }
        } else { break; }
    }
    let _ = done_tx.send(());
    let outcome = tokio::time::timeout(Duration::from_secs(2), result_rx).await.unwrap().unwrap();
    drop(client);
    server.abort(); let _ = server.await;
    (received, outcome)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_health_web_final_gate_rejects_already_dequeued_stale_health() {
    let (received, outcome) = health_sender_socket_case(0).await;
    assert_eq!(outcome, Ok(()));
    assert_eq!(received.len(), 1);
    assert_eq!(received[0]["packet"], "KeepAlive");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_health_web_current_health_is_sent_once_before_next_normal_packet() {
    let (received, outcome) = health_sender_socket_case(1).await;
    assert_eq!(outcome, Ok(()));
    assert_eq!(received.len(), 2);
    assert_eq!(received[0]["packet"], "HealthChanged");
    assert_eq!(received[0]["payload"]["hp"], 10);
    assert_eq!(received[1]["packet"], "KeepAlive");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_health_web_validation_failure_cancels_the_whole_current_transport() {
    let (received, outcome) = health_sender_socket_case(2).await;
    assert_eq!(outcome, Err(transport::TransportEnd::ZoneOverload(42)));
    assert!(received.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_ack_order_web_sender_waits_for_prepared_registration_activation() {
    let (done_tx, done_rx) = oneshot::channel();
    let done = Arc::new(Mutex::new(Some(done_rx)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/ws",
        get(move |upgrade: WebSocketUpgrade| {
            let done = done.lock().unwrap().take().unwrap();
            async move {
                upgrade.on_upgrade(move |socket| async move {
                    let (sink, _stream) = socket.split();
                    let sender = Arc::new(transport::SocketSender::new(sink, transport::TransportSignal::new()));
                    let gate = Arc::new(AsyncRwLock::new(()));
                    let bootstrap = gate.write().await;
                    let active = Arc::new(AtomicU64::new(0));
                    let (normal_tx, normal_rx) = mpsc::channel(2);
                    let (owner_tx, owner_rx) = mpsc::channel(1);
                    let _writer = spawn_zone_outbound_sender(
                        normal_rx,
                        owner_rx,
                        Arc::clone(&sender),
                        Arc::clone(&gate),
                        Arc::clone(&active),
                        watch::channel(0).0,
                    );
                    let location = |x| ServerPacket::UserLocation {
                        location: mir2_protocol::UserLocation {
                            position: Point { x, y: 23 },
                            direction: MirDirection::Right,
                        },
                    };
                    owner_tx
                        .send(SharedZoneLiveOutbound::new(42, location(25)))
                        .await
                        .unwrap();
                    // Prove the sender consumed the prepared epoch while bootstrap
                    // still holds the gate. No timer-based scheduling assumption.
                    tokio::time::timeout(Duration::from_secs(2), async {
                        while owner_tx.capacity() == 0 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    send_server_packet(&sender, &ServerPacket::KeepAlive { time: 11 })
                        .await
                        .unwrap();
                    active.store(42, Ordering::Release);
                    drop(bootstrap);
                    // Old registrations must still be fenced after activation.
                    owner_tx
                        .send(SharedZoneLiveOutbound::new(41, location(9)))
                        .await
                        .unwrap();
                    normal_tx
                        .send(SharedZoneLiveOutbound::new(
                            42,
                            ServerPacket::KeepAlive { time: 12 },
                        ))
                        .await
                        .unwrap();
                    let _ = done.await;
                })
            }
        }),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let (mut client, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
        .await
        .unwrap();
    let mut received = Vec::new();
    for _ in 0..3 {
        if let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) =
            tokio::time::timeout(Duration::from_millis(500), client.next()).await
        {
            received.push(serde_json::from_str::<Value>(&text).unwrap());
        } else {
            break;
        }
    }
    let _ = done_tx.send(());
    server.abort();
    let _ = server.await;
    assert_eq!(
        received.len(),
        3,
        "prepared ACK must survive activation: {received:?}"
    );
    assert_eq!(received[0]["packet"], "KeepAlive");
    assert_eq!(received[1]["packet"], "UserLocation");
    assert_eq!(received[1]["payload"]["x"], 25);
    assert_eq!(received[2]["packet"], "KeepAlive");
}

#[tokio::test]
async fn live_overflow_old_registration_signal_cannot_stop_replacement() {
    let (signal, receiver) = watch::channel(0);
    signal.send_replace(41);
    let active = Arc::new(AtomicU64::new(42));
    let work = async {
        tokio::time::sleep(Duration::from_millis(15)).await;
        "replacement stayed active"
    };
    assert_eq!(
        run_until_transport_end(work, receiver, active, transport::TransportSignal::new()).await,
        Ok("replacement stayed active")
    );
}

#[tokio::test]
async fn live_overflow_prepared_registration_signal_waits_for_matching_activation() {
    let (signal, receiver) = watch::channel(0);
    signal.send_replace(42);
    let active = Arc::new(AtomicU64::new(41));
    let next = Arc::clone(&active);
    let activation = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        next.store(42, Ordering::Release);
    });
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        run_until_transport_end(std::future::pending::<()>(), receiver, active, transport::TransportSignal::new()),
    )
    .await
    .unwrap();
    activation.await.unwrap();
    assert_eq!(result, Err(transport::TransportEnd::ZoneOverload(42)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_overflow_cancels_stalled_real_websocket_writer_and_closes_transport() {
    let (signal, receiver) = watch::channel(0);
    let (ready_tx, ready_rx) = oneshot::channel();
    let (closed_tx, closed_rx) = oneshot::channel();
    let state = Arc::new(Mutex::new(Some((receiver, ready_tx, closed_tx))));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/ws",
        get({
            let state = Arc::clone(&state);
            move |upgrade: WebSocketUpgrade| {
                let (overflows, ready, closed) = state.lock().unwrap().take().unwrap();
                async move {
                    upgrade.on_upgrade(move |socket| async move {
                        let active = Arc::new(AtomicU64::new(23));
                        let terminal = transport::TransportSignal::new();
                        let writer_terminal = terminal.clone();
                        let work = async move {
                            let (sink, stream) = socket.split();
                            let sender = Arc::new(transport::SocketSender::new(sink, writer_terminal.clone()));
                            let (_inputs, _reader, _pending) = spawn_socket_reader(
                                stream,
                                Arc::clone(&sender),
                                Arc::new(RwLock::new(None)),
                                Arc::new(AsyncRwLock::new(())),
                                Arc::new(AtomicBool::new(false)),
                                writer_terminal,
                                Arc::new(transport::MovementDrain::default()),
                                "overflow-transport-fixture".to_owned(),
                            );
                            // A permanently occupied writer lock models a stalled
                            // send. The control path must not acquire this lock.
                            let _occupied_writer = sender.lock_for_test().await;
                            let _ = ready.send(());
                            let _ =
                                send_server_packet(&sender, &ServerPacket::KeepAlive { time: 7 })
                                    .await;
                        };
                        let result = run_until_transport_end(work, overflows, active, terminal).await;
                        let _ = closed.send(result);
                    })
                }
            }
        }),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let (mut client, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), ready_rx)
        .await
        .unwrap()
        .unwrap();
    signal.send_replace(23);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), closed_rx)
            .await
            .unwrap()
            .unwrap(),
        Err(transport::TransportEnd::ZoneOverload(23))
    );
    let received = tokio::time::timeout(Duration::from_secs(2), client.next())
        .await
        .unwrap();
    assert!(
        !matches!(
            received,
            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(_)))
        ),
        "close transport; do not fabricate a protocol Disconnect or LogOut"
    );
    server.abort();
    let _ = server.await;
}

fn ordinary_session(config: &SimulationConfig) -> GatewaySession {
    let registry = ZoneRegistry::in_process();
    ordinary_session_on_registry(config, &registry, "overflowresume", "OverflowTest")
}

fn ordinary_session_on_registry(
    config: &SimulationConfig,
    registry: &ZoneRegistry,
    account_id: &str,
    character_name: &str,
) -> GatewaySession {
    let mut session = GatewaySession::new_with_zone_registry(config.clone(), registry);
    session
        .try_handle_packet(ClientPacket::NewAccount {
            account_id: account_id.into(),
            password: "overflow-resume-local-only".into(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .unwrap();
    assert!(session
        .try_handle_packet(ClientPacket::Login {
            account_id: account_id.into(),
            password: "overflow-resume-local-only".into(),
        })
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let index = session
        .try_handle_packet(ClientPacket::NewCharacter {
            name: character_name.into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
        })
        .unwrap()
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap();
    session
        .try_handle_packet(ClientPacket::StartGame {
            character_index: index,
        })
        .unwrap();
    assert!(session.active_identity().is_some());
    session
}

#[test]
fn owner_ack_order_real_tick_map_information_rebinds_owner_and_observer_after_portal() {
    struct ResetFullWorldCollision;
    impl Drop for ResetFullWorldCollision {
        fn drop(&mut self) {
            mir2_simulation::set_crystal_full_world_zone_collision(false);
        }
    }
    // The compact unit-test Bichon region does not include this real portal
    // landing (315,476). Use the same full collision data as the real gateway.
    mir2_simulation::set_crystal_full_world_zone_collision(true);
    let _restore_collision = ResetFullWorldCollision;
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let registry = ZoneRegistry::in_process();
    let mut owner =
        ordinary_session_on_registry(&config, &registry, "mapepochowner", "MapEpochOwner");
    let mut observer =
        ordinary_session_on_registry(&config, &registry, "mapepochpeer", "MapEpochPeer");
    // Fixture placement only. The tested transfer is an ordinary two-step
    // walk to the real MageHouse exit, completed by the shared cadence/Tick.
    owner.transfer_map("crystal:0115:17:19");
    observer.transfer_map("crystal:0:315:479");
    let (normal_tx, mut normal_rx) = mpsc::channel(64);
    let (owner_tx, mut owner_rx) = mpsc::channel(1);
    let sender = SharedZoneLiveOutboundSender::new(normal_tx, owner_tx);
    let active = AtomicU64::new(0);
    let ingress: SharedZoneMovementIngressSlot = Arc::new(RwLock::new(None));
    let mut registration = None;
    refresh_zone_live_outbound(
        &owner,
        true,
        true,
        &ingress,
        &sender,
        &active,
        &mut registration,
    )
    .unwrap();
    let old_id = active.load(Ordering::Acquire);
    assert!(old_id > 0);
    let (peer_tx, mut peer_rx) = mpsc::channel(64);
    let peer_registration = observer
        .register_zone_live_outbound(SharedZoneLiveOutboundSender::single(peer_tx))
        .unwrap()
        .unwrap();
    peer_registration.activate();
    owner.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Down,
    });
    let queued = owner.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Down,
    });
    assert!(!responses_begin_zone_bootstrap(&queued));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let snapshot = owner.world_snapshot();
        let player = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .unwrap();
        if (player.x, player.y) == (17, 21) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "ordinary pending movement did not reach portal"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let transferred = owner.tick();
    assert!(transferred.iter().any(
        |packet| matches!(packet, ServerPacket::MapInformation { info } if info.file_name == "0")
    ));
    assert!(
        !responses_require_resume_rotation(&transferred),
        "registration repair must not change token policy"
    );
    assert!(responses_begin_zone_bootstrap(&transferred));
    assert!(transferred.iter().any(|packet| matches!(packet, ServerPacket::UserLocation { location } if location.position == Point { x: 315, y: 476 })));
    refresh_zone_live_outbound(
        &owner,
        true,
        responses_begin_zone_bootstrap(&transferred),
        &ingress,
        &sender,
        &active,
        &mut registration,
    )
    .unwrap();
    let new_id = active.load(Ordering::Acquire);
    assert_ne!(
        old_id, new_id,
        "real MapInformation must replace the invalidated old registration"
    );
    // An idle Tick/ordinary interaction must keep this new epoch stable.
    let idle = owner.tick();
    assert!(!responses_begin_zone_bootstrap(&idle));
    refresh_zone_live_outbound(
        &owner,
        true,
        false,
        &ingress,
        &sender,
        &active,
        &mut registration,
    )
    .unwrap();
    assert_eq!(active.load(Ordering::Acquire), new_id);
    while let Ok(outbound) = owner_rx.try_recv() {
        assert_eq!(
            outbound.registration_id(),
            old_id,
            "queued old-map ACK remains fenced"
        );
    }
    while normal_rx.try_recv().is_ok() {}
    let peer_id = {
        let mut id = None;
        while let Ok(outbound) = peer_rx.try_recv() {
            if let ServerPacket::ObjectPlayer { info } = outbound.into_packet() {
                if info.name == "MapEpochOwner" {
                    id = Some(info.object_id);
                }
            }
        }
        id.expect("peer must see the newly arrived owner")
    };
    let pre_move = owner.world_snapshot();
    let responses = owner.handle_packet(ClientPacket::Walk {
        direction: MirDirection::Down,
    });
    assert!(
        !responses
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
        "post-arrival owner ACK must still use live priority"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut owner_arrived = false;
    let mut peer_saw_move = false;
    let mut actual_locations = Vec::new();
    while !(owner_arrived && peer_saw_move) {
        while let Ok(outbound) = owner_rx.try_recv() {
            assert_eq!(outbound.registration_id(), new_id);
            if let ServerPacket::UserLocation { location } = outbound.into_packet() {
                owner_arrived |= location.position == Point { x: 315, y: 477 };
                actual_locations.push(location.position);
            }
        }
        while let Ok(outbound) = peer_rx.try_recv() {
            if let ServerPacket::ObjectWalk { movement } = outbound.into_packet() {
                peer_saw_move |=
                    movement.object_id == peer_id && movement.position == Point { x: 315, y: 477 };
            }
        }
        assert!(
            Instant::now() < deadline,
            "new-map owner ACK={owner_arrived}, observer move={peer_saw_move}, locations={actual_locations:?}, direct={responses:?}, before={:?}",
            pre_move.entities.iter().filter(|entity| (entity.x-315).abs() <= 2 && (entity.y-476).abs() <= 3).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_overflow_transport_loss_saves_authoritative_move_and_keeps_one_use_resume() {
    let config = SimulationConfig::default();
    let mut session = ordinary_session(&config);
    let identity = session.active_identity().unwrap();
    let initial = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    let receipt = session
        .zone_movement_ingress()
        .unwrap()
        .try_execute(ClientPacket::Walk {
            direction: MirDirection::Right,
        })
        .unwrap()
        .unwrap();
    let location = receipt
        .packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::UserLocation { location } => Some(location.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        location.position,
        Point {
            x: initial.x + 1,
            y: initial.y
        }
    );

    let store = ReconnectSessionStore::default();
    let mut native_resume = NativeResumeConnectionState::new();
    native_resume.opted_in = true;
    let now = gateway_unix_ms();
    let gateway_id = session.session_id().to_string();
    let issued = store
        .issue_resume_credential(
            None,
            ResumeIssueContext {
                account_id: &identity.account_id,
                character_index: identity.character_index,
                gateway_session_id: &gateway_id,
                identity_session_id: "overflow-resume-fixture",
                identity_expires_at_ms: now + 60_000,
                source_connection_nonce: &native_resume.connection_nonce,
            },
            now,
            1,
            || true,
        )
        .unwrap();
    native_resume.family_id = Some(issued.binding.family_id.clone());
    let (signal, receiver) = watch::channel(0);
    signal.send_replace(9);
    assert_eq!(
        run_until_transport_end(
            std::future::pending::<()>(),
            receiver,
            Arc::new(AtomicU64::new(9)),
            transport::TransportSignal::new(),
        )
        .await,
        Err(transport::TransportEnd::ZoneOverload(9))
    );
    let leave = ExplicitWorldLeaveState::default();
    assert!(!leave.completed);
    let mut queue = WebSessionSaveQueue::new(GatewaySaveQueueConfig::new(
        Duration::from_secs(60),
        Duration::from_secs(60),
        8,
    ));
    let outcome = persist_web_session_before_teardown(&mut session, &mut queue).await;
    assert_eq!(outcome, WebTeardownPersistenceOutcome::Saved);
    assert!(apply_teardown_persistence_to_resume(
        outcome,
        &mut native_resume,
        &store,
        Some(&identity.account_id)
    ));
    session.release_teardown_for_resume().unwrap();
    let saved = config.account_store.lock().unwrap().accounts[&identity.account_id].saves
        [&identity.character_index]
        .clone();
    assert_eq!(saved.position, location.position);
    assert_eq!(saved.direction, location.direction);
    let capacity = Arc::new(GatewayCapacityState::with_limits(None, Some(1), Some(1)));
    store.store(
        crate::GatewaySessionCacheKey {
            account_id: identity.account_id.clone(),
            character_index: identity.character_index,
        },
        session,
        Some(capacity.try_acquire_active_session().unwrap()),
        capacity.try_acquire_reconnect_lease().unwrap(),
        native_resume.family_id,
        Duration::from_secs(60),
    );
    let (restored, _) = store
        .take_by_credential(&issued.credential, &issued.binding, gateway_unix_ms())
        .expect("overflow is transport loss, so its saved resume remains lawful");
    assert_eq!(restored.session.session_id(), gateway_id);
    let restored_self = restored
        .session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    assert_eq!(
        (restored_self.x, restored_self.y),
        (location.position.x, location.position.y)
    );
    assert!(store
        .take_by_credential(&issued.credential, &issued.binding, gateway_unix_ms())
        .is_none());
}

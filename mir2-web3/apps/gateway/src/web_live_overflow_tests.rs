use super::*;
use mir2_protocol::{MirClass, MirGender};
use mir2_simulation::{SimulationConfig, WorldEntityKind};
use tokio::sync::{oneshot, watch};

#[derive(Debug)]
struct HealthValidationProbe(Arc<std::sync::atomic::AtomicUsize>);

impl crate::routing::OwnerHealthValidator for HealthValidationProbe {
    fn validate(&self, _: &mir2_simulation::ZoneOwnerHealthChange) -> Result<bool, String> {
        match self.0.load(Ordering::Acquire) {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("prepared validation transport failure".into()),
        }
    }
}

// This probe isolates actual WS scheduling and cancellation. Current-life
// Source validation is covered by the real Zone and authenticated RPC tests.
async fn health_sender_socket_case(
    validation: usize,
) -> (Vec<Value>, Result<(), transport::TransportEnd>) {
    let (done_tx, done_rx) = oneshot::channel::<()>();
    let (result_tx, result_rx) = oneshot::channel();
    let handoff = Arc::new(Mutex::new(Some((done_rx, result_tx))));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/ws",
        get(move |upgrade: WebSocketUpgrade| {
            let (done, result) = handoff.lock().unwrap().take().unwrap();
            async move {
                upgrade.on_upgrade(move |socket| async move {
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
                        let _writer = spawn_zone_outbound_sender(
                            normal_rx,
                            owner_rx,
                            sender.clone(),
                            gate.clone(),
                            active.clone(),
                            failure_tx,
                            None,
                        );
                        let decision = Arc::new(std::sync::atomic::AtomicUsize::new(1));
                        let outbound = SharedZoneLiveOutbound::new(
                            42,
                            ServerPacket::HealthChanged { hp: 10, mp: 9 },
                        )
                        .with_owner_health(
                            crate::routing::OwnerHealthDelivery {
                                change: mir2_simulation::ZoneOwnerHealthChange {
                                    cursor: mir2_simulation::ZoneOwnerHealthCursor {
                                        session_id: mir2_simulation::SessionId::new(
                                            "web-health-probe",
                                        ),
                                        online_owner: "private-probe".into(),
                                        object_id: 1,
                                        life_generation: 1,
                                        dead: false,
                                        health_sequence: 1,
                                    },
                                    hp_before: 11,
                                    mp_before: 9,
                                    hp: 10,
                                    mp: 9,
                                },
                                validator: Arc::new(HealthValidationProbe(decision.clone())),
                                highest_sent_sequence: Arc::new(Mutex::new(0)),
                            },
                        );
                        normal_tx.send(outbound).await.unwrap();
                        tokio::time::timeout(Duration::from_secs(2), async {
                            while normal_tx.capacity() != 2 {
                                tokio::task::yield_now().await;
                            }
                        })
                        .await
                        .unwrap();
                        // The writer has already dequeued the packet, but cannot pass
                        // the bootstrap gate. Change validation before releasing it.
                        decision.store(validation, Ordering::Release);
                        drop(bootstrap);
                        normal_tx
                            .send(SharedZoneLiveOutbound::new(
                                42,
                                ServerPacket::KeepAlive { time: 99 },
                            ))
                            .await
                            .unwrap();
                        let _ = done.await;
                    };
                    let outcome =
                        run_until_transport_end(work, failure_rx, active.clone(), terminal).await;
                    let _ = result.send(outcome);
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
    while let Ok(Some(Ok(message))) =
        tokio::time::timeout(Duration::from_secs(2), client.next()).await
    {
        if let tokio_tungstenite::tungstenite::Message::Text(text) = message {
            let value: Value = serde_json::from_str(&text).unwrap();
            let terminal = value["packet"] == "KeepAlive";
            received.push(value);
            if terminal {
                break;
            }
        } else {
            break;
        }
    }
    let _ = done_tx.send(());
    let outcome = tokio::time::timeout(Duration::from_secs(2), result_rx)
        .await
        .unwrap()
        .unwrap();
    drop(client);
    server.abort();
    let _ = server.await;
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
async fn owner_route_web_final_gate_rejects_dequeued_non_health_frame_and_cancels_transport() {
    let mut socket = OutboundBatchSocket::new().await;
    let valid = Arc::new(AtomicBool::new(true));
    let guard_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let guard_valid = valid.clone();
    let observed_calls = guard_calls.clone();
    let mut outbound = SharedZoneLiveOutbound::new(42, ServerPacket::KeepAlive { time: 901 });
    assert!(outbound.owner_health().is_none());
    outbound.with_owner_route_guard(Some(Arc::new(move || {
        observed_calls.fetch_add(1, Ordering::AcqRel);
        if guard_valid.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err("zone_owner_requires_rebind".to_owned())
        }
    })));
    socket
        .normal
        .as_ref()
        .unwrap()
        .send(outbound)
        .await
        .unwrap();
    // Capacity proves dequeue while bootstrap still owns the execution gate.
    // The owner changes after dequeue and before final Web serialization.
    outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
    assert_eq!(guard_calls.load(Ordering::Acquire), 0);
    assert_eq!(*socket.failure.borrow(), 0);
    socket.enqueue(42, 902).await;
    let cancelled = tokio::spawn(run_until_transport_end(
        std::future::pending::<()>(),
        socket.failure.clone(),
        socket.active.clone(),
        socket.terminal.clone(),
    ));
    valid.store(false, Ordering::Release);
    drop(socket.bootstrap.take());
    outbound_batch_wait_until(|| socket.writer.handle.is_finished()).await;
    let outcome = tokio::time::timeout(Duration::from_secs(2), cancelled)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(outcome, Err(transport::TransportEnd::ZoneOverload(42)));
    assert_eq!(
        socket.terminal.reason(),
        Some(transport::TransportEnd::ZoneOverload(42))
    );
    assert_eq!(*socket.failure.borrow(), 42);
    assert!(
        guard_calls.load(Ordering::Acquire) > 0,
        "a non-health frame must run its owner guard at the final Web send gate"
    );
    // The writer is finished, so neither the stale guarded packet nor the
    // following ordinary packet can arrive later. A transport Close is fine;
    // an application Text/Binary frame would prove the final gate was bypassed.
    if let Ok(Some(Ok(message))) =
        tokio::time::timeout(Duration::from_millis(20), socket.client.next()).await
    {
        assert!(
            !matches!(
                message,
                tokio_tungstenite::tungstenite::Message::Text(_)
                    | tokio_tungstenite::tungstenite::Message::Binary(_)
            ),
            "stale or subsequent application packet reached the actual WebSocket"
        );
    }
    assert!(
        socket.gate.try_write().is_ok(),
        "failed final validation must release the execution gate"
    );
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
                    let sender = Arc::new(transport::SocketSender::new(
                        sink,
                        transport::TransportSignal::new(),
                    ));
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
                        None,
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
        run_until_transport_end(
            std::future::pending::<()>(),
            receiver,
            active,
            transport::TransportSignal::new(),
        ),
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
                            let sender = Arc::new(transport::SocketSender::new(
                                sink,
                                writer_terminal.clone(),
                            ));
                            let (_inputs, _reader, _pending) = spawn_socket_reader(
                                stream,
                                Arc::clone(&sender),
                                Arc::new(RwLock::new(None)),
                                Arc::new(AsyncRwLock::new(())),
                                Arc::new(AtomicBool::new(false)),
                                writer_terminal,
                                Arc::new(transport::MovementDrain::default()),
                                "overflow-transport-fixture".to_owned(),
                                None,
                            );
                            // A permanently occupied writer lock models a stalled
                            // send. The control path must not acquire this lock.
                            let _occupied_writer = sender.lock_for_test().await;
                            let _ = ready.send(());
                            let _ =
                                send_server_packet(&sender, &ServerPacket::KeepAlive { time: 7 })
                                    .await;
                        };
                        let result =
                            run_until_transport_end(work, overflows, active, terminal).await;
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
// Real socket fixture for the bounded sender. Bootstrap starts locked so tests
// can establish channel contents and the fair-lock waiter order explicitly.
struct OutboundBatchSocket {
    client: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    sender: SharedWebSocketSender,
    terminal: transport::TransportSignal,
    gate: SharedSerialExecutionGate,
    bootstrap: Option<tokio::sync::OwnedRwLockWriteGuard<()>>,
    active: Arc<AtomicU64>,
    normal: Option<mpsc::Sender<SharedZoneLiveOutbound>>,
    owner: Option<mpsc::Sender<SharedZoneLiveOutbound>>,
    failure: watch::Receiver<u64>,
    writer: ZoneOutboundSenderTask,
    _done: oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for OutboundBatchSocket {
    fn drop(&mut self) {
        self.writer.handle.abort();
        self.server.abort();
    }
}

impl OutboundBatchSocket {
    async fn new() -> Self {
        Self::new_with_clock(Instant::now).await
    }

    async fn new_with_clock(now: impl Fn() -> Instant + Send + 'static) -> Self {
        let (ready_tx, ready_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let handoff = Arc::new(Mutex::new(Some((ready_tx, done_rx))));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route(
            "/ws",
            get(move |upgrade: WebSocketUpgrade| {
                let (ready, done) = handoff.lock().unwrap().take().unwrap();
                async move {
                    upgrade.on_upgrade(move |socket| async move {
                        let (sink, _stream) = socket.split();
                        let terminal = transport::TransportSignal::new();
                        let sender = Arc::new(transport::SocketSender::new(sink, terminal.clone()));
                        let _ = ready.send((sender, terminal));
                        let _ = done.await;
                    })
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let (client, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
            .await
            .unwrap();
        let (sender, terminal) = ready_rx.await.unwrap();
        let gate = Arc::new(AsyncRwLock::new(()));
        let bootstrap = gate.clone().write_owned().await;
        let active = Arc::new(AtomicU64::new(42));
        let (normal, normal_rx) = mpsc::channel(128);
        let (owner, owner_rx) = mpsc::channel(16);
        let (failure_tx, failure) = watch::channel(0);
        let writer = spawn_zone_outbound_sender_with_clock(
            normal_rx,
            owner_rx,
            sender.clone(),
            gate.clone(),
            active.clone(),
            failure_tx,
            None,
            now,
        );
        Self {
            client,
            sender,
            terminal,
            gate,
            bootstrap: Some(bootstrap),
            active,
            normal: Some(normal),
            owner: Some(owner),
            failure,
            writer,
            _done: done_tx,
            server,
        }
    }

    async fn enqueue(&self, epoch: u64, time: i64) {
        self.normal
            .as_ref()
            .unwrap()
            .send(SharedZoneLiveOutbound::new(
                epoch,
                ServerPacket::KeepAlive { time },
            ))
            .await
            .unwrap();
    }

    async fn packet(&mut self) -> Value {
        let message = tokio::time::timeout(Duration::from_secs(2), self.client.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let tokio_tungstenite::tungstenite::Message::Text(text) = message else {
            panic!("expected packet, received {message:?}");
        };
        serde_json::from_str(&text).unwrap()
    }
}

async fn outbound_batch_wait_until(mut ready: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

async fn outbound_batch_queue_writer<F: std::future::Future>(mut writer: std::pin::Pin<&mut F>) {
    assert!(
        std::future::poll_fn(|cx| {
            std::task::Poll::Ready(std::future::Future::poll(writer.as_mut(), cx).is_pending())
        })
        .await
    );
}

#[tokio::test]
async fn zone_outbound_batch_ready_fifo_precedes_queued_writer_after_bootstrap_wait() {
    let base = Instant::now();
    let elapsed_ms = Arc::new(AtomicU64::new(0));
    let clock = elapsed_ms.clone();
    let mut socket = OutboundBatchSocket::new_with_clock(move || {
        base + Duration::from_millis(clock.load(Ordering::Acquire))
    })
    .await;
    socket.enqueue(42, 1).await;
    outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
    socket.enqueue(42, 2).await;
    socket.enqueue(42, 3).await;
    let mut next_writer = Box::pin(socket.gate.clone().write_owned());
    outbound_batch_queue_writer(next_writer.as_mut()).await;
    // Advance while the reader waits, then freeze. Bootstrap wait must not
    // consume the batch budget, regardless of real socket or task scheduling.
    elapsed_ms.store(5, Ordering::Release);
    drop(socket.bootstrap.take());
    let guard = tokio::time::timeout(Duration::from_secs(2), next_writer)
        .await
        .unwrap();
    send_server_packet(&socket.sender, &ServerPacket::KeepAlive { time: 99 })
        .await
        .unwrap();
    drop(guard);
    for time in [1, 2, 3, 99] {
        assert_eq!(socket.packet().await["payload"]["time"], time);
    }
}

#[tokio::test]
async fn zone_outbound_batch_stale_backlog_is_count_bounded_and_yields_to_writer() {
    let frozen = Instant::now();
    let mut socket = OutboundBatchSocket::new_with_clock(move || frozen).await;
    socket.enqueue(41, 0).await;
    outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
    for time in 1..96 {
        socket.enqueue(41, time).await;
    }
    let mut next_writer = Box::pin(socket.gate.clone().write_owned());
    outbound_batch_queue_writer(next_writer.as_mut()).await;
    drop(socket.bootstrap.take());
    let guard = tokio::time::timeout(Duration::from_secs(2), next_writer)
        .await
        .unwrap();
    let consumed = 96 - (128 - socket.normal.as_ref().unwrap().capacity());
    // The next batch may have dequeued its first packet outside the read gate.
    assert!(
        (32..=33).contains(&consumed),
        "consumed {consumed} before writer"
    );
    assert!(!socket.writer.handle.is_finished());
    drop(guard);
    socket.enqueue(42, 99).await;
    assert_eq!(socket.packet().await["payload"]["time"], 99);
}

#[derive(Debug)]
struct OutboundBatchSlowHealth;

impl crate::routing::OwnerHealthValidator for OutboundBatchSlowHealth {
    fn validate(&self, _: &mir2_simulation::ZoneOwnerHealthChange) -> Result<bool, String> {
        // A deliberately slow single claim exceeds the between-packet budget.
        std::thread::sleep(Duration::from_millis(5));
        Ok(false)
    }
}

fn outbound_batch_health(
    validator: Arc<dyn crate::routing::OwnerHealthValidator>,
) -> SharedZoneLiveOutbound {
    SharedZoneLiveOutbound::new(42, ServerPacket::HealthChanged { hp: 10, mp: 9 })
        .with_owner_health(crate::routing::OwnerHealthDelivery {
            change: mir2_simulation::ZoneOwnerHealthChange {
                cursor: mir2_simulation::ZoneOwnerHealthCursor {
                    session_id: mir2_simulation::SessionId::new("batch-health-probe"),
                    online_owner: "private-batch-probe".into(),
                    object_id: 1,
                    life_generation: 1,
                    dead: false,
                    health_sequence: 1,
                },
                hp_before: 11,
                mp_before: 9,
                hp: 10,
                mp: 9,
            },
            validator,
            highest_sent_sequence: Arc::new(Mutex::new(0)),
        })
}

#[tokio::test]
async fn zone_outbound_batch_slow_empty_health_releases_budget_before_next_packet() {
    let mut socket = OutboundBatchSocket::new().await;
    socket
        .normal
        .as_ref()
        .unwrap()
        .send(outbound_batch_health(Arc::new(OutboundBatchSlowHealth)))
        .await
        .unwrap();
    outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
    socket.enqueue(42, 2).await;
    let mut next_writer = Box::pin(socket.gate.clone().write_owned());
    outbound_batch_queue_writer(next_writer.as_mut()).await;
    drop(socket.bootstrap.take());
    let guard = tokio::time::timeout(Duration::from_secs(2), next_writer)
        .await
        .unwrap();
    send_server_packet(&socket.sender, &ServerPacket::KeepAlive { time: 99 })
        .await
        .unwrap();
    drop(guard);
    assert_eq!(socket.packet().await["payload"]["time"], 99);
    assert_eq!(socket.packet().await["payload"]["time"], 2);
}

#[tokio::test]
async fn zone_outbound_batch_preserves_first_packet_then_owner_priority_and_closed_channel() {
    let mut socket = OutboundBatchSocket::new().await;
    socket.enqueue(42, 1).await;
    outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
    socket.enqueue(42, 2).await;
    socket.enqueue(41, 90).await;
    socket.enqueue(42, 3).await;
    socket
        .owner
        .as_ref()
        .unwrap()
        .send(SharedZoneLiveOutbound::new(
            42,
            ServerPacket::UserLocation {
                location: mir2_protocol::UserLocation {
                    position: Point { x: 25, y: 23 },
                    direction: MirDirection::Right,
                },
            },
        ))
        .await
        .unwrap();
    drop(socket.owner.take());
    drop(socket.bootstrap.take());
    assert_eq!(socket.packet().await["payload"]["time"], 1);
    assert_eq!(socket.packet().await["packet"], "UserLocation");
    assert_eq!(socket.packet().await["payload"]["time"], 2);
    assert_eq!(socket.packet().await["payload"]["time"], 3);
    // One channel is closed and the other empty/open: neither can hold the gate.
    let guard = tokio::time::timeout(Duration::from_secs(2), socket.gate.write())
        .await
        .unwrap();
    drop(guard);
    socket.enqueue(42, 4).await;
    assert_eq!(socket.packet().await["payload"]["time"], 4);
    drop(socket.normal.take());
    outbound_batch_wait_until(|| socket.writer.handle.is_finished()).await;
    assert_eq!(*socket.failure.borrow(), 0);
}

#[tokio::test]
async fn zone_outbound_batch_prepared_ack_waits_for_bootstrap_and_filters_stale_epoch() {
    let mut socket = OutboundBatchSocket::new().await;
    socket.active.store(0, Ordering::Release);
    socket
        .owner
        .as_ref()
        .unwrap()
        .send(SharedZoneLiveOutbound::new(
            42,
            ServerPacket::UserLocation {
                location: mir2_protocol::UserLocation {
                    position: Point { x: 25, y: 23 },
                    direction: MirDirection::Right,
                },
            },
        ))
        .await
        .unwrap();
    outbound_batch_wait_until(|| socket.owner.as_ref().unwrap().capacity() == 16).await;
    socket.enqueue(41, 90).await;
    socket.enqueue(42, 2).await;
    send_server_packet(&socket.sender, &ServerPacket::KeepAlive { time: 11 })
        .await
        .unwrap();
    assert_eq!(socket.packet().await["payload"]["time"], 11);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), socket.client.next())
            .await
            .is_err()
    );
    socket.active.store(42, Ordering::Release);
    drop(socket.bootstrap.take());
    assert_eq!(socket.packet().await["packet"], "UserLocation");
    assert_eq!(socket.packet().await["payload"]["time"], 2);
}

#[tokio::test]
async fn zone_outbound_batch_health_claim_remains_validated_between_normal_packets() {
    for validation in [0, 1, 2] {
        let mut socket = OutboundBatchSocket::new().await;
        socket.enqueue(42, 1).await;
        outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
        socket
            .normal
            .as_ref()
            .unwrap()
            .send(outbound_batch_health(Arc::new(HealthValidationProbe(
                Arc::new(AtomicUsize::new(validation)),
            ))))
            .await
            .unwrap();
        socket.enqueue(42, 2).await;
        drop(socket.bootstrap.take());
        assert_eq!(socket.packet().await["payload"]["time"], 1);
        if validation == 2 {
            outbound_batch_wait_until(|| socket.writer.handle.is_finished()).await;
            assert_eq!(*socket.failure.borrow(), 42);
            assert!(
                tokio::time::timeout(Duration::from_millis(20), socket.client.next())
                    .await
                    .is_err()
            );
        } else {
            if validation == 1 {
                assert_eq!(socket.packet().await["packet"], "HealthChanged");
            }
            assert_eq!(socket.packet().await["payload"]["time"], 2);
            assert_eq!(*socket.failure.borrow(), 0);
        }
    }
}

#[tokio::test]
async fn zone_outbound_batch_overload_stops_before_later_packet() {
    let mut socket = OutboundBatchSocket::new().await;
    socket.enqueue(42, 1).await;
    outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
    let mut overloaded = SharedZoneLiveOutbound::new(42, ServerPacket::KeepAlive { time: 90 });
    overloaded.with_transport_failure_flag(Arc::new(AtomicBool::new(true)));
    socket
        .normal
        .as_ref()
        .unwrap()
        .send(overloaded)
        .await
        .unwrap();
    socket.enqueue(42, 2).await;
    drop(socket.bootstrap.take());
    assert_eq!(socket.packet().await["payload"]["time"], 1);
    outbound_batch_wait_until(|| socket.writer.handle.is_finished()).await;
    assert_eq!(*socket.failure.borrow(), 42);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), socket.client.next())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn zone_outbound_batch_send_error_stops_and_releases_gate() {
    let mut socket = OutboundBatchSocket::new().await;
    socket.enqueue(42, 1).await;
    outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
    socket.enqueue(42, 2).await;
    socket.terminal.end(transport::TransportEnd::WriteError);
    drop(socket.bootstrap.take());
    outbound_batch_wait_until(|| socket.writer.handle.is_finished()).await;
    assert_eq!(*socket.failure.borrow(), 42);
    assert_eq!(
        socket.terminal.reason(),
        Some(transport::TransportEnd::WriteError)
    );
    assert!(socket.gate.try_write().is_ok());
    assert!(
        tokio::time::timeout(Duration::from_millis(20), socket.client.next())
            .await
            .is_err()
    );
}

#[derive(Debug)]
struct OutboundBatchReplacedHealth {
    active: Arc<AtomicU64>,
    fail: bool,
}

impl crate::routing::OwnerHealthValidator for OutboundBatchReplacedHealth {
    fn validate(&self, _: &mir2_simulation::ZoneOwnerHealthChange) -> Result<bool, String> {
        self.active.store(43, Ordering::Release);
        if self.fail {
            Err("old registration validation failed after replacement".into())
        } else {
            Ok(true)
        }
    }
}

#[tokio::test]
async fn zone_outbound_batch_health_completion_rechecks_epoch_before_send_or_failure() {
    for fail in [false, true] {
        let mut socket = OutboundBatchSocket::new().await;
        socket
            .normal
            .as_ref()
            .unwrap()
            .send(outbound_batch_health(Arc::new(
                OutboundBatchReplacedHealth {
                    active: socket.active.clone(),
                    fail,
                },
            )))
            .await
            .unwrap();
        outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
        socket.enqueue(42, 90).await;
        socket.enqueue(43, 3).await;
        drop(socket.bootstrap.take());
        // Neither a successful old health claim nor its failure may escape
        // the final epoch fence or cancel the replacement registration.
        assert_eq!(socket.packet().await["payload"]["time"], 3);
        assert_eq!(*socket.failure.borrow(), 0);
        assert_eq!(socket.active.load(Ordering::Acquire), 43);
    }
}

// Per-stamp capture avoids process-global environment changes in parallel tests.
fn delivery_trace_packet() -> ServerPacket {
    ServerPacket::UserLocation {
        location: mir2_protocol::UserLocation {
            position: Point { x: 25, y: 23 },
            direction: MirDirection::Right,
        },
    }
}

fn delivery_trace_fields(line: &str) -> std::collections::BTreeMap<&str, &str> {
    assert!(line.starts_with("[gateway-zone-delivery-trace] "));
    line.split_whitespace()
        .skip(1)
        .map(|field| field.split_once('=').unwrap())
        .collect()
}

fn delivery_trace_stages(events: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    events
        .lock()
        .unwrap()
        .iter()
        .map(|line| delivery_trace_fields(line)["stage"].to_owned())
        .collect()
}

#[test]
fn zone_delivery_trace_disabled_and_nonplayer_remove_preserve_wire() {
    let packet = delivery_trace_packet();
    let events = Arc::new(Mutex::new(Vec::new()));
    let plain = SharedZoneLiveOutbound::new(42, packet.clone());
    assert!(plain.delivery_trace().is_none());
    let disabled = plain.with_test_delivery_trace(false, 50048, &packet, true, events.clone());
    assert!(disabled.delivery_trace().is_none());
    assert_eq!(
        serde_json::to_value(disabled.into_packet()).unwrap(),
        serde_json::to_value(&packet).unwrap()
    );
    let monster_remove = ServerPacket::ObjectRemove { object_id: 77 };
    let excluded = SharedZoneLiveOutbound::new(42, monster_remove.clone())
        .with_test_delivery_trace(true, 50048, &monster_remove, false, events.clone());
    assert!(excluded.delivery_trace().is_none());
    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn zone_delivery_trace_player_remove_keeps_distinct_recipient_and_target() {
    let packet = ServerPacket::ObjectRemove { object_id: 50041 };
    let events = Arc::new(Mutex::new(Vec::new()));
    let outbound = SharedZoneLiveOutbound::new(42, packet.clone()).with_test_delivery_trace(
        true,
        50048,
        &packet,
        true,
        events.clone(),
    );
    outbound.delivery_trace().unwrap().emit(
        crate::routing::ZoneDeliveryTraceStage::Dequeue,
        Some(Instant::now()),
        None,
    );
    let events = events.lock().unwrap();
    let fields = delivery_trace_fields(&events[0]);
    assert_eq!(fields["recipientZoneID"], "50048");
    assert_eq!(fields["targetObjectID"], "50041");
    assert_eq!(fields["kind"], "ObjectRemove");
    assert_eq!(fields["hasPosition"], "0");
    assert_eq!(
        serde_json::to_value(outbound.into_packet()).unwrap(),
        serde_json::to_value(packet).unwrap()
    );
}

#[test]
fn zone_delivery_trace_full_and_closed_are_not_enqueue_success() {
    let (tx, rx) = mpsc::channel(1);
    let sender = SharedZoneLiveOutboundSender::single(tx);
    let events = Arc::new(Mutex::new(Vec::new()));
    let packet = delivery_trace_packet();
    for _ in 0..2 {
        let result = sender.try_send(
            SharedZoneLiveOutbound::new(42, packet.clone()).with_test_delivery_trace(
                true,
                50048,
                &packet,
                true,
                events.clone(),
            ),
        );
        if events.lock().unwrap().len() == 1 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(mpsc::error::TrySendError::Full(_))));
        }
    }
    drop(rx);
    assert!(matches!(
        sender.try_send(
            SharedZoneLiveOutbound::new(42, packet.clone()).with_test_delivery_trace(
                true,
                50048,
                &packet,
                true,
                events.clone()
            )
        ),
        Err(mpsc::error::TrySendError::Closed(_))
    ));
    assert_eq!(
        delivery_trace_stages(&events),
        ["enqueue", "full", "closed"]
    );
    let events = events.lock().unwrap();
    let ids = events
        .iter()
        .map(|line| delivery_trace_fields(line)["serverTraceID"])
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 3);
}

#[tokio::test]
async fn zone_delivery_trace_same_packet_survives_queue_gate_and_send() {
    let mut socket = OutboundBatchSocket::new().await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let packet = delivery_trace_packet();
    let sender = SharedZoneLiveOutboundSender::new(
        socket.normal.as_ref().unwrap().clone(),
        socket.owner.as_ref().unwrap().clone(),
    );
    sender
        .try_send(
            SharedZoneLiveOutbound::new(42, packet.clone()).with_test_delivery_trace(
                true,
                50048,
                &packet,
                true,
                events.clone(),
            ),
        )
        .unwrap();
    outbound_batch_wait_until(|| socket.owner.as_ref().unwrap().capacity() == 16).await;
    assert!(!delivery_trace_stages(&events)
        .iter()
        .any(|stage| stage == "gate"));
    drop(socket.bootstrap.take());
    assert_eq!(socket.packet().await["packet"], "UserLocation");
    outbound_batch_wait_until(|| {
        delivery_trace_stages(&events)
            .iter()
            .any(|stage| stage == "sent")
    })
    .await;
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 5);
    let trace_id = delivery_trace_fields(&events[0])["serverTraceID"].to_owned();
    for line in events.iter() {
        let fields = delivery_trace_fields(line);
        assert_eq!(fields["serverTraceID"], trace_id);
        assert_eq!(fields["recipientZoneID"], "50048");
        assert_eq!(fields["targetObjectID"], "50048");
        assert_eq!(fields["kind"], "UserLocation");
        for (key, value) in fields {
            if key != "stage" && key != "kind" {
                assert!(value.parse::<i128>().is_ok(), "non-numeric field: {key}");
            }
        }
        for sensitive in [
            "account",
            "character",
            "credential",
            "private",
            "payload",
            "name",
        ] {
            assert!(!line.contains(sensitive));
        }
    }
    let stages = events
        .iter()
        .map(|line| delivery_trace_fields(line)["stage"])
        .filter(|stage| *stage != "enqueue")
        .collect::<Vec<_>>();
    assert_eq!(stages, ["dequeue", "gate", "send", "sent"]);
}

#[tokio::test]
async fn zone_delivery_trace_terminal_results_preserve_sender_fences() {
    for terminal in [
        "stale",
        "health-empty",
        "health-error",
        "overload",
        "send-error",
    ] {
        let mut socket = OutboundBatchSocket::new().await;
        let events = Arc::new(Mutex::new(Vec::new()));
        let metadata_packet = delivery_trace_packet();
        let mut outbound = if terminal.starts_with("health-") {
            let decision = if terminal == "health-empty" { 0 } else { 2 };
            outbound_batch_health(Arc::new(HealthValidationProbe(Arc::new(AtomicUsize::new(
                decision,
            )))))
        } else {
            SharedZoneLiveOutbound::new(42, metadata_packet.clone())
        }
        .with_test_delivery_trace(true, 50048, &metadata_packet, true, events.clone());
        if terminal == "stale" {
            socket.active.store(43, Ordering::Release);
        } else if terminal == "overload" {
            outbound.with_transport_failure_flag(Arc::new(AtomicBool::new(true)));
        } else if terminal == "send-error" {
            socket.terminal.end(transport::TransportEnd::WriteError);
        }
        socket
            .normal
            .as_ref()
            .unwrap()
            .send(outbound)
            .await
            .unwrap();
        outbound_batch_wait_until(|| socket.normal.as_ref().unwrap().capacity() == 128).await;
        // An untraced trailing packet proves skip cases still continue normally.
        socket
            .enqueue(if terminal == "stale" { 43 } else { 42 }, 123)
            .await;
        drop(socket.bootstrap.take());
        if terminal == "stale" || terminal == "health-empty" {
            assert_eq!(socket.packet().await["payload"]["time"], 123);
            assert_eq!(*socket.failure.borrow(), 0);
        } else {
            outbound_batch_wait_until(|| socket.writer.handle.is_finished()).await;
            assert_eq!(*socket.failure.borrow(), 42);
            assert!(socket.gate.try_write().is_ok());
            assert!(
                tokio::time::timeout(Duration::from_millis(20), socket.client.next())
                    .await
                    .is_err()
            );
        }
        let stages = delivery_trace_stages(&events);
        assert_eq!(stages.last().map(String::as_str), Some(terminal));
        assert!(!stages.iter().any(|stage| stage == "sent"));
        assert_eq!(
            stages
                .iter()
                .filter(|stage| stage.as_str() == "send")
                .count(),
            usize::from(terminal == "send-error")
        );
    }
}

#[test]
fn zone_outbound_block_flag_requires_exact_opt_in_without_env_mutation() {
    assert!(!zone_outbound_block_enabled_value(None));
    for value in ["", "0", "true", "01", " 1", "1 "] {
        assert!(!zone_outbound_block_enabled_value(Some(value)));
    }
    assert!(zone_outbound_block_enabled_value(Some("1")));
    assert!(matches!(
        ZoneOutboundBlockKind::for_packet(&delivery_trace_packet()),
        ZoneOutboundBlockKind::UserLocation
    ));
    assert!(matches!(
        ZoneOutboundBlockKind::for_packet(&ServerPacket::ObjectRemove { object_id: 50041 }),
        ZoneOutboundBlockKind::ObjectRemove
    ));
    assert!(matches!(
        ZoneOutboundBlockKind::for_packet(&ServerPacket::KeepAlive { time: 123 }),
        ZoneOutboundBlockKind::Other
    ));
}

#[test]
fn zone_outbound_block_threshold_and_schema_are_numeric_or_fixed_enums() {
    for kind in [
        ZoneOutboundBlockKind::OwnerHealth,
        ZoneOutboundBlockKind::UserLocation,
        ZoneOutboundBlockKind::ObjectPlayer,
        ZoneOutboundBlockKind::ObjectRemove,
        ZoneOutboundBlockKind::Other,
    ] {
        for lane in [ZoneOutboundBlockLane::Owner, ZoneOutboundBlockLane::Normal] {
            for stage in [
                ZoneOutboundBlockStage::ReadGate,
                ZoneOutboundBlockStage::HealthSchedule,
                ZoneOutboundBlockStage::HealthRun,
                ZoneOutboundBlockStage::HealthAwait,
                ZoneOutboundBlockStage::Send,
                ZoneOutboundBlockStage::CompletionToPop,
            ] {
                let context = ZoneOutboundBlockContext {
                    registration: 49,
                    batch_id: 3,
                    batch_index: 1,
                    kind,
                    lane,
                    normal_depth: 7,
                    owner_depth: 2,
                };
                assert!(context.line(stage, Duration::ZERO, 123).is_none());
                assert!(context
                    .line(stage, Duration::from_micros(49_999), 123)
                    .is_none());
                for elapsed_us in [50_000_u64, 50_001, 1_495_209] {
                    let line = context
                        .line(stage, Duration::from_micros(elapsed_us), 123)
                        .unwrap();
                    assert!(line.starts_with("[gateway-zone-outbound-block] "));
                    let fields = line
                        .split_whitespace()
                        .skip(1)
                        .map(|field| field.split_once('=').unwrap())
                        .collect::<std::collections::BTreeMap<_, _>>();
                    assert_eq!(fields.len(), 10);
                    assert_eq!(fields["stage"], stage.label());
                    assert_eq!(fields["kind"], kind.label());
                    assert_eq!(fields["lane"], lane.label());
                    for (key, expected) in [
                        ("registration", 49_u128),
                        ("batchID", 3),
                        ("batchIndex", 1),
                        ("normalDepth", 7),
                        ("ownerDepth", 2),
                        ("elapsedUs", u128::from(elapsed_us)),
                        ("recordedUnixMs", 123),
                    ] {
                        assert_eq!(fields[key].parse::<u128>().unwrap(), expected);
                    }
                    for sensitive in [
                        "account",
                        "character",
                        "credential",
                        "payload",
                        "name",
                        "received",
                    ] {
                        assert!(!line.contains(sensitive));
                    }
                }
            }
        }
    }
}

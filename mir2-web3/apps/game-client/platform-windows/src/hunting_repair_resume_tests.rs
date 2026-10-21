// Included in hunting_repair_gateway::tests. Real bounded command ownership,
// two physical sockets and the normal strict handshake/source/sender paths.
// The server is synthetic; no database, GUI, bearer log or commit is claimed.

fn resume_test_realm() -> Value {
    json!({"type":"realmInfo","payload":{"schema":"mir2-realm-handshake/1",
        "realmId":"synthetic-native-realm","npcRepairMode":"hunting-v1"}})
}

fn resume_test_grant(account: &str, session: &str, expires: u64) -> Value {
    json!({"type":"identitySession","token":"synthetic-native-test-token",
        "session":{"accountId":account,"sessionId":session,"expiresAtMs":expires,
            "revokedAtMs":null}})
}

fn resume_test_identity(
    account: &str,
    session: &str,
    expires: u64,
) -> crate::hunting_repair_connection::NativeResumeIdentity {
    let mut connection = HuntingRepairConnection::new();
    connection
        .observe(&resume_test_realm().to_string())
        .unwrap();
    connection
        .observe(&resume_test_grant(account, session, expires).to_string())
        .unwrap();
    connection.resume_identity(gateway_unix_ms()).unwrap()
}

#[test]
fn native_hunting_repair_resume_context_requires_original_live_same_socket_identity_and_decision() {
    let expires = gateway_unix_ms() + 60_000;
    let owner = resume_test_identity("synthetic-player", "synthetic-session", expires);
    let prior = GatewaySessionContext {
        account_id: Some("SYNTHETIC-PLAYER".into()),
        character_index: Some(0),
        ..Default::default()
    };
    let mut resume = NativeResumeClientState::default();
    assert!(resume.observe_connection_identity(owner.clone(), ConnectionPhase::Normal, &prior));
    resume.character_index = Some(0);
    resume.connection_identity = None;
    let mut fresh = GatewaySessionContext::default();
    assert!(
        !resume.has_current_resume_owner(Some(0)),
        "a prior socket is no fresh grant"
    );
    assert!(!resume.publish_resumed_context(&mut fresh));
    for changed in [
        resume_test_identity("other-player", "synthetic-session", expires),
        resume_test_identity("synthetic-player", "other-session", expires),
        resume_test_identity("synthetic-player", "synthetic-session", expires + 1),
    ] {
        assert!(!resume.observe_connection_identity(
            changed,
            ConnectionPhase::AwaitingResume,
            &fresh
        ));
        assert!(fresh.account_id.is_none());
    }
    assert!(resume.observe_connection_identity(owner, ConnectionPhase::AwaitingResume, &fresh));
    for index in [None, Some(-1), Some(1)] {
        assert!(!resume.has_current_resume_owner(index));
    }
    assert!(resume.has_current_resume_owner(Some(0)));
    assert!(
        !resume.publish_resumed_context(&mut fresh),
        "a grant alone is no resume decision"
    );
    resume.resumed_owner_accepted = true;
    assert!(resume.publish_resumed_context(&mut fresh));
    assert_eq!(fresh.account_id.as_deref(), Some("synthetic-player"));
    assert_eq!(fresh.character_index, Some(0));
    resume.clear();
    assert!(!resume.publish_resumed_context(&mut GatewaySessionContext::default()));

    let mut expired = HuntingRepairConnection::new();
    expired.observe(&resume_test_realm().to_string()).unwrap();
    expired
        .observe(&resume_test_grant("synthetic-player", "synthetic-session", 1).to_string())
        .unwrap();
    assert!(expired.resume_identity(gateway_unix_ms()).is_none());
}

async fn resume_test_receive(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    kind: &str,
) -> Value {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            match socket
                .next()
                .await
                .expect("synthetic peer must remain open")
                .unwrap()
            {
                Message::Text(raw) => {
                    let value: Value = serde_json::from_str(&raw).unwrap();
                    if value["type"].as_str() == Some(kind) {
                        return value;
                    }
                    assert!(
                        matches!(
                            value["type"].as_str(),
                            Some("keepAlive" | "npcPurchaseOwner")
                        ),
                        "only unrelated controls may precede the expected request"
                    );
                }
                Message::Ping(payload) => socket.send(Message::Pong(payload)).await.unwrap(),
                _ => {}
            }
        }
    })
    .await
    .expect("expected native request must reach the physical peer")
}

#[test]
fn native_hunting_repair_resume_owned_bootstrap_requires_fresh_map_identity() {
    let _guard = native_queue_test_guard();
    let f = ready();
    let mut ingress = OrdinaryRepairIngress::new(&f);
    let expires = gateway_unix_ms() + 60_000;
    let identity = resume_test_identity("synthetic-player", "synthetic-session", expires);
    assert!(ingress.resume.observe_connection_identity(
        identity,
        ConnectionPhase::Normal,
        &ingress.context,
    ));
    ingress.resume.character_index = Some(0);
    ingress
        .resume
        .record_credential(&"R".repeat(MAX_CREDENTIAL_LENGTH), Some(expires), Some(1));
    f.fence.socket_lost();
    f.fence.begin_connection().unwrap();
    ingress.context = GatewaySessionContext::default();
    ingress.resume.connection_identity = None;
    ingress.phase = ConnectionPhase::AwaitingResume;
    // Read a separate strict physical grant instead of retaining old reader
    // authority. This fixture has no MapInformation on the new socket.
    assert!(ingress.resume.observe_connection_identity(
        resume_test_identity("synthetic-player", "synthetic-session", expires),
        ConnectionPhase::AwaitingResume,
        &ingress.context,
    ));
    ingress.raw(&json!({"type":"sessionResumed","characterIndex":0,"generation":2}).to_string());
    let owner = ordinary_repair_owner();
    assert!(owner.get("mapIndex").is_none());
    ingress.snapshot(&owner);
    assert_eq!(ingress.phase, ConnectionPhase::Resumed);
    assert!(!ingress.bootstrapped);
    assert!(ingress.adapter.last_full_producer_stamp.is_none());
    assert!(f.fence.stamp().unwrap().map.is_none());
    assert!(ingress.context.account_id.is_none());
    let mut complete = owner;
    complete["mapIndex"] = json!(0);
    ingress.snapshot(&complete);
    assert_eq!(ingress.phase, ConnectionPhase::Normal);
    assert!(ingress.bootstrapped);
    assert!(f
        .fence
        .accepts(ingress.adapter.last_full_producer_stamp.unwrap(), true));
    assert!(ingress.resume.publish_resumed_context(&mut ingress.context));
    assert_eq!(
        ingress.context.account_id.as_deref(),
        Some("synthetic-player")
    );
    assert_eq!(ingress.context.character_index, Some(0));
}

async fn resume_test_wait(mut ready: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(4), async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("bounded native owner transition must complete");
}

async fn resume_test_producer(
    receiver: &std::sync::mpsc::Receiver<NativeGameplaySnapshot>,
    marker: &str,
) -> NativeCommandStamp {
    let mut accepted = None;
    resume_test_wait(|| {
        for snapshot in receiver.try_iter() {
            if snapshot.producer_models.is_some()
                && snapshot
                    .entity_render_payload
                    .as_ref()
                    .is_some_and(|payload| payload["entities"][0]["name"].as_str() == Some(marker))
            {
                accepted = snapshot.command_stamp;
            }
        }
        accepted.is_some()
    })
    .await;
    accepted.unwrap()
}

fn resume_test_snapshot(marker: &str) -> Value {
    let mut owner = ordinary_repair_owner();
    owner["mapIndex"] = json!(0);
    owner["entities"][0]["name"] = json!(marker);
    json!({"type":"worldSnapshot","payload":owner})
}

#[tokio::test]
async fn native_hunting_repair_resume_real_owned_sockets_keep_pending_close_and_query_original() {
    resume_test_owned_socket_fixture(false).await;
}

#[tokio::test]
async fn native_hunting_repair_resume_closed_gameplay_receiver_terminates_without_bootstrap_or_query(
) {
    resume_test_owned_socket_fixture(true).await;
}

async fn resume_test_owned_socket_fixture(close_gameplay_receiver: bool) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::oneshot;
    let _guard = native_queue_test_guard();
    let mut runtime = bevy::prelude::App::new();
    runtime.add_plugins(mir2_bevy_runtime::Mir2NativeSessionBoundaryPlugin);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (grant_sent_tx, grant_sent_rx) = oneshot::channel();
    let (confirm_seen_tx, confirm_seen_rx) = oneshot::channel();
    let (break_tx, break_rx) = oneshot::channel();
    let (allow_full_tx, allow_full_rx) = oneshot::channel();
    let (idle_tx, idle_rx) = oneshot::channel();
    let (query_seen_tx, query_seen_rx) = oneshot::channel();
    let (closed_tx, closed_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let expires = gateway_unix_ms() + 60_000;
        let grant = resume_test_grant("synthetic-player", "synthetic-session", expires);
        let credential = "R".repeat(MAX_CREDENTIAL_LENGTH);
        let (tcp, _) = listener.accept().await.unwrap();
        let mut first = tokio_tungstenite::accept_async(tcp).await.unwrap();
        resume_test_receive(&mut first, "clientCapabilities").await;
        first
            .send(Message::Text(resume_test_realm().to_string().into()))
            .await
            .unwrap();
        resume_test_receive(&mut first, "login").await;
        first
            .send(Message::Text(grant.to_string().into()))
            .await
            .unwrap();
        first
            .send(Message::Text(
                json!({"type":"packet","packet":"LoginSuccess","payload":{"characters":[]}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        grant_sent_tx.send(()).unwrap();
        resume_test_receive(&mut first, "startGame").await;
        first
            .send(Message::Text(
                json!({"type":"packet","packet":"StartGame","payload":{"result":4}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        first
            .send(Message::Text(
                resume_test_snapshot("initial-source").to_string().into(),
            ))
            .await
            .unwrap();
        first.send(Message::Text(json!({"type":"resumeCredential","credential":credential,"generation":1,"expiresAtMs":expires}).to_string().into())).await.unwrap();
        // A service belongs to the already-entered actor. StartGame's first
        // complete owner deliberately retires any earlier NPC source.
        let interaction = resume_test_receive(&mut first, "interact").await;
        assert_eq!(interaction["objectId"], json!(3001));
        first
            .send(Message::Text(
                json!({"type":"packet","packet":"NPCResponse","payload":{"page":["Repair NPC","<Repair/@repair>"]}})
                    .to_string().into(),
            ))
            .await.unwrap();
        first
            .send(Message::Text(
                resume_test_snapshot("npc-dialog-source").to_string().into(),
            ))
            .await
            .unwrap();
        let selection = resume_test_receive(&mut first, "selectNpcDialog").await;
        assert_eq!(selection["target"], json!("@repair"));
        first
            .send(Message::Text(
                json!({"type":"packet","packet":"NPCRepair","payload":{"rate":1.0}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        first
            .send(Message::Text(
                resume_test_snapshot("repair-service-source")
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        let quote_raw = resume_test_receive(&mut first, "huntingRepairOwner").await;
        let quote_request = wire::parse_client_request(&quote_raw.to_string()).unwrap();
        assert!(matches!(quote_request.action, wire::Action::Quote { .. }));
        let now = gateway_unix_ms();
        first
            .send(Message::Text(
                frame(
                    &quote_request,
                    wire::Reply::Quoted {
                        quote: wire::Quote {
                            quote_id: quote_id(),
                            unique_id: mir2_client_wire::U64::new(ORDINARY_REPAIR_UID),
                            special: false,
                            cost_bound_gold: mir2_client_wire::U64::new(6),
                            created_at_ms: now as i64,
                            expires_at_ms: now as i64 + 30_000,
                        },
                    },
                    None,
                )
                .into(),
            ))
            .await
            .unwrap();
        first
            .send(Message::Text(
                resume_test_snapshot("quoted-source").to_string().into(),
            ))
            .await
            .unwrap();
        let confirm_raw = resume_test_receive(&mut first, "huntingRepairOwner").await;
        let confirm_request = wire::parse_client_request(&confirm_raw.to_string()).unwrap();
        assert!(
            matches!(&confirm_request.action, wire::Action::Confirm {quote_id:id} if id==&quote_id())
        );
        confirm_seen_tx.send(()).unwrap();
        break_rx.await.unwrap();
        // Intentionally omit this one Confirm answer, then lose the physical
        // socket. This proves custody only, not a real database commit.
        first.close(None).await.unwrap();
        drop(first);
        let (tcp, _) = listener.accept().await.unwrap();
        let mut second = tokio_tungstenite::accept_async(tcp).await.unwrap();
        resume_test_receive(&mut second, "clientCapabilities").await;
        let resume = resume_test_receive(&mut second, "resumeSession").await;
        assert!(resume["credential"] == json!(credential));
        second
            .send(Message::Text(resume_test_realm().to_string().into()))
            .await
            .unwrap();
        second
            .send(Message::Text(
                resume_test_snapshot("quarantined-before-resume")
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        second
            .send(Message::Text(grant.to_string().into()))
            .await
            .unwrap();
        second
            .send(Message::Text(
                json!({"type":"sessionResumed","characterIndex":0,"generation":2})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        second
            .send(Message::Text(
                resume_test_snapshot("backpressured-resume")
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        allow_full_rx.await.unwrap();
        second
            .send(Message::Text(
                resume_test_snapshot("accepted-resume").to_string().into(),
            ))
            .await
            .unwrap();
        if close_gameplay_receiver {
            // A complete producer cannot reach the closed main-thread queue.
            // The current physical connection must terminate without issuing
            // Query or silently authorizing a new owner context.
            tokio::time::timeout(Duration::from_secs(4), async {
                while let Some(message) = second.next().await {
                    match message {
                        Ok(Message::Text(raw)) => {
                            let event: Value = serde_json::from_str(&raw).unwrap();
                            assert_ne!(event["type"].as_str(), Some("huntingRepairOwner"));
                        }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
            })
            .await
            .expect("closed producer receiver must terminate the resume socket");
            closed_tx.send(()).unwrap();
            return;
        }
        // No automatic Query or second Confirm may follow an accepted resume.
        let quiet = tokio::time::timeout(Duration::from_millis(120), async {
            loop {
                match second.next().await.unwrap().unwrap() {
                    Message::Text(raw) => {
                        let event: Value = serde_json::from_str(&raw).unwrap();
                        assert_ne!(event["type"].as_str(), Some("huntingRepairOwner"));
                    }
                    Message::Ping(payload) => second.send(Message::Pong(payload)).await.unwrap(),
                    _ => {}
                }
            }
        })
        .await;
        assert!(quiet.is_err());
        idle_tx.send(()).unwrap();
        let query_raw = resume_test_receive(&mut second, "huntingRepairOwner").await;
        let query_request = wire::parse_client_request(&query_raw.to_string()).unwrap();
        assert!(
            matches!(&query_request.action,wire::Action::Query {quote_id:id} if id==&quote_id())
        );
        assert_ne!(query_request.request_id, confirm_request.request_id);
        second
            .send(Message::Text(
                frame(
                    &query_request,
                    wire::Reply::Pending {
                        quote_id: quote_id(),
                    },
                    None,
                )
                .into(),
            ))
            .await
            .unwrap();
        query_seen_tx.send(()).unwrap();
        while let Some(message) = second.next().await {
            match message {
                Ok(Message::Text(raw)) => {
                    let event: Value = serde_json::from_str(&raw).unwrap();
                    assert_ne!(event["type"].as_str(), Some("huntingRepairOwner"));
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    });
    let (sender, receiver) = command_channel(8);
    let fence = sender.ownership_fence().unwrap();
    let (shell_tx, shell_rx) = std::sync::mpsc::channel();
    let shell = NativeShellEventSender::Owned {
        sender: shell_tx,
        fence: fence.clone(),
    };
    let (game_tx, game_rx) = std::sync::mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed_calls = calls.clone();
    let client = tokio::spawn(async move {
        run_gateway_client_with_world_ingest(
            &format!("ws://{address}"),
            receiver,
            shell,
            game_tx,
            NativeReconnectConfig {
                resume_deadline: Duration::from_secs(4),
                initial_backoff: Duration::from_millis(10),
                max_backoff: Duration::from_millis(20),
                jitter_percent: 0,
                command_batch_limit: 8,
                max_attempts: 3,
            },
            move |raw| {
                let snapshot: Value = serde_json::from_str(&raw).unwrap();
                if snapshot["entities"][0]["name"].as_str() == Some("backpressured-resume") {
                    observed_calls.fetch_add(1, Ordering::SeqCst);
                    false
                } else {
                    mir2_bevy_runtime::native_ingest::push_native_world_state(raw)
                }
            },
        )
        .await
    });
    resume_test_wait(|| fence.0.lock().unwrap().connected).await;
    sender
        .send_with_stamp(
            GatewayCommand::Wire(NativeOutboundCommand::Login {
                account_id: "synthetic-player".into(),
                password: "synthetic-password".into(),
            }),
            fence.stamp(),
        )
        .unwrap();
    grant_sent_rx.await.unwrap();
    sender
        .send_with_stamp(
            GatewayCommand::Wire(NativeOutboundCommand::StartGame { character_index: 0 }),
            fence.stamp(),
        )
        .unwrap();
    let first_stamp = resume_test_producer(&game_rx, "initial-source").await;
    sender
        .send_with_stamp(
            GatewayCommand::Wire(NativeOutboundCommand::Interact { object_id: 3001 }),
            Some(first_stamp),
        )
        .unwrap();
    let dialog_stamp = resume_test_producer(&game_rx, "npc-dialog-source").await;
    sender
        .send_with_stamp(
            GatewayCommand::Wire(NativeOutboundCommand::SelectNpcDialog {
                target: "@repair".into(),
            }),
            Some(dialog_stamp),
        )
        .unwrap();
    let service_stamp = resume_test_producer(&game_rx, "repair-service-source").await;
    resume_test_wait(|| fence.0.lock().unwrap().hunting_repair_gate.is_some()).await;
    let selection = fence
        .0
        .lock()
        .unwrap()
        .hunting_repair_source
        .selection(service_stamp)
        .expect("post-entry NPCRepair plus complete current inventory must grant its source");
    sender
        .send_with_stamp(
            GatewayCommand::HuntingRepair(HuntingRepairCommand::Quote {
                unique_id: ORDINARY_REPAIR_UID,
                special: false,
                selection: Some(selection.clone()),
            }),
            Some(service_stamp),
        )
        .unwrap();
    let quoted_stamp = resume_test_producer(&game_rx, "quoted-source").await;
    sender
        .send_with_stamp(
            GatewayCommand::HuntingRepair(HuntingRepairCommand::Confirm {
                quote_id: quote_id(),
                selection: Some(selection),
            }),
            Some(quoted_stamp),
        )
        .unwrap();
    confirm_seen_rx.await.unwrap();
    sender
        .send_with_stamp(
            GatewayCommand::HuntingRepair(HuntingRepairCommand::Close),
            Some(quoted_stamp),
        )
        .unwrap();
    resume_test_wait(|| fence.0.lock().unwrap().outstanding.is_empty()).await;
    assert!(fence.0.lock().unwrap().hunting_repair_busy);
    break_tx.send(()).unwrap();
    resume_test_wait(|| calls.load(Ordering::SeqCst) >= 1).await;
    assert!(!fence.accepts(first_stamp, true));
    let before_full = fence.stamp().unwrap();
    assert!(
        !fence.accepts(before_full, true),
        "sessionResumed plus a backpressured snapshot cannot reopen world commands"
    );
    assert!(sender
        .send_with_stamp(
            GatewayCommand::HuntingRepair(HuntingRepairCommand::Query {
                quote_id: quote_id()
            }),
            Some(first_stamp)
        )
        .is_err());
    if close_gameplay_receiver {
        drop(game_rx);
        allow_full_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(4), closed_rx)
            .await
            .unwrap()
            .unwrap();
        resume_test_wait(|| {
            fence
                .stamp()
                .is_some_and(|current| current.connection > before_full.connection)
        })
        .await;
        assert!(fence.stamp().unwrap().actor.is_none());
        assert_eq!(
            shell_rx
                .try_iter()
                .filter(|envelope| matches!(
                    envelope.event,
                    ShellGatewayEvent::PlayerBootstrapped { .. }
                ))
                .count(),
            1,
            "only the original socket bootstrap may reach the visible shell"
        );
        sender
            .send_with_stamp(GatewayCommand::Shutdown, fence.stamp())
            .unwrap();
        tokio::time::timeout(Duration::from_secs(4), client)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(4), server)
            .await
            .unwrap()
            .unwrap();
        return;
    }
    allow_full_tx.send(()).unwrap();
    let resumed_stamp = resume_test_producer(&game_rx, "accepted-resume").await;
    assert_ne!(resumed_stamp.connection, first_stamp.connection);
    resume_test_wait(|| {
        fence
            .0
            .lock()
            .unwrap()
            .hunting_repair_gate
            .as_ref()
            .is_some_and(|gate| gate.is_current(epoch(resumed_stamp).unwrap()))
    })
    .await;
    idle_rx.await.unwrap();
    sender
        .send_with_stamp(
            GatewayCommand::HuntingRepair(HuntingRepairCommand::Query {
                quote_id: quote_id(),
            }),
            Some(resumed_stamp),
        )
        .unwrap();
    query_seen_rx.await.unwrap();
    sender
        .send_with_stamp(GatewayCommand::Shutdown, fence.stamp())
        .unwrap();
    tokio::time::timeout(Duration::from_secs(4), client)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(4), server)
        .await
        .unwrap()
        .unwrap();
}

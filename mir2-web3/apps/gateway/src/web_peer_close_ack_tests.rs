use super::*;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message as ClientMessage;

/// Exercise the production reader, including its real split WebSocket, without
/// another writer accidentally flushing tungstenite's automatic Close reply.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_close_receives_wire_ack_before_reader_and_sink_are_dropped() {
    let (ready_tx, ready_rx) = oneshot::channel();
    let (done_tx, done_rx) = oneshot::channel();
    let setup = Arc::new(Mutex::new(Some((ready_tx, done_tx))));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/ws",
        get({
            let setup = Arc::clone(&setup);
            move |upgrade: WebSocketUpgrade| {
                let (ready, done) = setup.lock().unwrap().take().unwrap();
                async move {
                    upgrade.on_upgrade(move |socket| async move {
                        let terminal = transport::TransportSignal::new();
                        let (sink, stream) = socket.split();
                        let sender = Arc::new(transport::SocketSender::new(sink, terminal.clone()));
                        let (inputs, reader, _pending) = spawn_socket_reader(
                            stream,
                            Arc::clone(&sender),
                            Arc::new(RwLock::new(None)),
                            Arc::new(AsyncRwLock::new(())),
                            Arc::new(AtomicBool::new(false)),
                            terminal.clone(),
                            Arc::new(transport::MovementDrain::default()),
                            "peer-close-ack-fixture".to_owned(),
                            None,
                        );
                        ready.send(()).unwrap();
                        let reason = transport::wait_for_end(terminal.subscribe()).await;
                        drop(reader);
                        drop(inputs);
                        drop(sender);
                        let _ = done.send(reason);
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
    client.close(None).await.unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(2), client.next())
        .await
        .expect("peer Close ACK must arrive within the existing client allowance");
    let reason = tokio::time::timeout(Duration::from_secs(2), done_rx)
        .await
        .unwrap()
        .unwrap();
    server.abort();
    let _ = server.await;
    assert_eq!(reason, transport::TransportEnd::Closed);
    assert!(
        matches!(reply, Some(Ok(ClientMessage::Close(_)))),
        "reader dropped the connection before a real Close ACK: {reply:?}"
    );
}

/// A producer can run between the split reader receiving Close and flushing
/// tungstenite's queued reply. Its rejected application write must not erase
/// that reply, nor may completing the reply erase the first terminal reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_close_receives_wire_ack_after_a_concurrent_producer_is_rejected() {
    rejected_producer_keeps_wire_close_ack(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_close_receives_wire_ack_after_a_concurrent_batch_is_rejected() {
    rejected_producer_keeps_wire_close_ack(true).await;
}

async fn rejected_producer_keeps_wire_close_ack(batch: bool) {
    let (done_tx, done_rx) = oneshot::channel();
    let setup = Arc::new(Mutex::new(Some(done_tx)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/ws",
        get({
            let setup = Arc::clone(&setup);
            move |upgrade: WebSocketUpgrade| {
                let done = setup.lock().unwrap().take().unwrap();
                async move {
                    upgrade.on_upgrade(move |socket| async move {
                        let terminal = transport::TransportSignal::new();
                        let (sink, mut stream) = socket.split();
                        let sender = Arc::new(transport::SocketSender::new(sink, terminal.clone()));
                        assert!(matches!(stream.next().await, Some(Ok(Message::Close(_)))));
                        let producer = Arc::clone(&sender);
                        let error = tokio::spawn(async move {
                            if batch {
                                producer
                                    .send_batch(vec![Message::Text("post-logout-packet".into())])
                                    .await
                                    .map(|_| ())
                            } else {
                                producer
                                    .send(Message::Text("post-logout-snapshot".into()))
                                    .await
                            }
                        })
                        .await
                        .unwrap()
                        .expect_err("a gameplay producer must be rejected after peer Close");
                        eprintln!("rejected post-Close producer: {error:?}");
                        let first_reason = terminal.reason();
                        let flush = sender.flush_peer_close_reply().await;
                        drop(stream);
                        drop(sender);
                        let _ = done.send((first_reason, terminal.reason(), flush.is_ok()));
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
    client.close(None).await.unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(2), client.next())
        .await
        .expect("peer Close ACK must not be abandoned by the rejected producer");
    let (first, final_reason, flushed) = tokio::time::timeout(Duration::from_secs(2), done_rx)
        .await
        .unwrap()
        .unwrap();
    server.abort();
    let _ = server.await;
    assert_eq!(first, Some(transport::TransportEnd::WriteError));
    assert_eq!(
        final_reason, first,
        "Close completion must preserve the first cause"
    );
    assert!(
        matches!(reply, Some(Ok(ClientMessage::Close(_)))),
        "rejected producer abandoned the queued Close ACK: reply={reply:?}, flushed={flushed}"
    );
}

/// Keep the real production reader suspended immediately before its normal
/// Close flush, then let the rejected producer end the real owner work loop.
/// The reply must already be on the wire when that loop aborts the reader.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_close_wire_ack_survives_owner_loop_aborting_the_suspended_reader() {
    let (ready_tx, ready_rx) = oneshot::channel();
    let (done_tx, done_rx) = oneshot::channel();
    let setup = Arc::new(Mutex::new(Some((ready_tx, done_tx))));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/ws",
        get({
            let setup = Arc::clone(&setup);
            move |upgrade: WebSocketUpgrade| {
                let (ready, done) = setup.lock().unwrap().take().unwrap();
                async move {
                    upgrade.on_upgrade(move |socket| async move {
                        let terminal = transport::TransportSignal::new();
                        let (sink, stream) = socket.split();
                        let sender = Arc::new(transport::SocketSender::new(sink, terminal.clone()));
                        let close_gate = sender.pause_peer_close_reply_for_test().await;
                        let (inputs, reader, _pending) = spawn_socket_reader(
                            stream,
                            Arc::clone(&sender),
                            Arc::new(RwLock::new(None)),
                            Arc::new(AsyncRwLock::new(())),
                            Arc::new(AtomicBool::new(false)),
                            terminal.clone(),
                            Arc::new(transport::MovementDrain::default()),
                            "peer-close-producer-race-fixture".to_owned(),
                            None,
                        );
                        let reader_abort = reader.handle.abort_handle();
                        ready.send(()).unwrap();
                        tokio::time::timeout(Duration::from_secs(2), async {
                            while !sender.peer_close_reply_started_for_test() {
                                tokio::task::yield_now().await;
                            }
                        })
                        .await
                        .unwrap();
                        let work = async {
                            let _reader = reader;
                            let _inputs = inputs;
                            let error = sender
                                .send(Message::Text("post-logout-snapshot".into()))
                                .await
                                .expect_err("ordinary producer must reject the closing transport");
                            eprintln!("owner-loop rejected producer: {error:?}");
                            std::future::pending::<()>().await;
                        };
                        let (_overload_tx, overload_rx) = tokio::sync::watch::channel(0_u64);
                        let result = run_until_transport_end(
                            work,
                            overload_rx,
                            Arc::new(AtomicU64::new(0)),
                            terminal.clone(),
                        )
                        .await;
                        tokio::time::timeout(Duration::from_secs(2), async {
                            while !reader_abort.is_finished() {
                                tokio::task::yield_now().await;
                            }
                        })
                        .await
                        .unwrap();
                        drop(close_gate);
                        drop(sender);
                        let _ = done.send((result, terminal.reason()));
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
    client.close(None).await.unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(2), client.next())
        .await
        .unwrap();
    let (result, reason) = tokio::time::timeout(Duration::from_secs(2), done_rx)
        .await
        .unwrap()
        .unwrap();
    server.abort();
    let _ = server.await;
    assert_eq!(result, Err(transport::TransportEnd::WriteError));
    assert_eq!(reason, Some(transport::TransportEnd::WriteError));
    assert!(
        matches!(reply, Some(Ok(ClientMessage::Close(_)))),
        "owner-loop cancellation lost Close ACK: {reply:?}"
    );
}

use super::*;
use tokio::sync::oneshot;

/// Real socket reader observes Close even while the ordinary owner task is
/// awaiting a permanently occupied writer lock. No gameplay queue polling is
/// needed, and no protocol logout is invented to release the transport.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_close_reader_ends_stalled_real_socket_work() {
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
                        let writer_terminal = terminal.clone();
                        let movement_drain = Arc::new(transport::MovementDrain::default());
                        let reader_drain = Arc::clone(&movement_drain);
                        let (_overload, overloads) = tokio::sync::watch::channel(0);
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
                                reader_drain,
                                "close-reader-fixture".to_owned(),
                            );
                            let _occupied = sender.lock_for_test().await;
                            ready.send(()).unwrap();
                            send_server_packet(&sender, &ServerPacket::KeepAlive { time: 7 }).await
                        };
                        let result = run_until_transport_end(
                            work,
                            overloads,
                            Arc::new(AtomicU64::new(0)),
                            terminal,
                        )
                        .await;
                        movement_drain.close_and_wait().await;
                        let _ = done.send(result);
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
    let result = tokio::time::timeout(Duration::from_secs(2), done_rx)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(result, Err(transport::TransportEnd::Closed)),
        "unexpected close result: {result:?}"
    );
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn transport_close_first_signal_before_wait_is_not_lost() {
    let terminal = transport::TransportSignal::new();
    terminal.end(transport::TransportEnd::ReadError);
    let (_tx, rx) = tokio::sync::watch::channel(0);
    let result = run_until_transport_end(
        std::future::pending::<()>(),
        rx,
        Arc::new(AtomicU64::new(0)),
        terminal,
    )
    .await;
    assert_eq!(result, Err(transport::TransportEnd::ReadError));
}

#[tokio::test]
async fn transport_close_signal_during_last_work_poll_preserves_first_reason() {
    let terminal = transport::TransportSignal::new();
    let closer = terminal.clone();
    let (_tx, rx) = tokio::sync::watch::channel(0);
    let work = async move {
        closer.end(transport::TransportEnd::ReadError);
        "work completed after the reader closed"
    };
    let result = run_until_transport_end(work, rx, Arc::new(AtomicU64::new(0)), terminal).await;
    assert_eq!(result, Err(transport::TransportEnd::ReadError));
}

#[tokio::test]
async fn transport_close_preserves_only_actually_completed_explicit_leave() {
    for completed in [false, true] {
        let terminal = transport::TransportSignal::new();
        let closer = terminal.clone();
        let (started_tx, started_rx) = oneshot::channel();
        let (_overload, overloads) = tokio::sync::watch::channel(0);
        let mut explicit_leave = ExplicitWorldLeaveState::default();
        let work = async {
            if completed {
                explicit_leave.completed = true;
            }
            started_tx.send(()).unwrap();
            std::future::pending::<()>().await;
        };
        let close = tokio::spawn(async move {
            started_rx.await.unwrap();
            closer.end(transport::TransportEnd::Closed);
        });
        let result =
            run_until_transport_end(work, overloads, Arc::new(AtomicU64::new(0)), terminal).await;
        close.await.unwrap();
        assert_eq!(result, Err(transport::TransportEnd::Closed));
        assert_eq!(explicit_leave.completed, completed);
    }
}

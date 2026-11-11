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

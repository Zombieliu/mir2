//! Preserve individual protocol frames while bounding catalogue write batches.
use std::time::{Duration, Instant};

use axum::extract::ws::Message;
use futures_util::Sink;
use mir2_protocol::ServerPacket;

pub(super) use super::transport::{MAX_BATCH_BYTES, MAX_BATCH_MESSAGES};
use super::{server_packet_to_event, transport::SocketSender};

#[derive(Debug, Default)]
pub(super) struct ResponseWriteMetrics {
    pub(super) messages: usize,
    pub(super) bytes: usize,
    pub(super) flushes: usize,
    pub(super) serialization: Duration,
    pub(super) lock_wait: Duration,
    pub(super) feed: Duration,
    pub(super) flush: Duration,
    pub(super) elapsed: Duration,
}

pub(super) async fn send_responses<S>(
    sender: &SocketSender<S>,
    responses: &[ServerPacket],
) -> Result<ResponseWriteMetrics, axum::Error>
where
    S: Sink<Message, Error = axum::Error> + Unpin,
{
    let started = Instant::now();
    let mut metrics = ResponseWriteMetrics::default();
    let mut batch = Vec::with_capacity(MAX_BATCH_MESSAGES);
    let mut batch_bytes = 0;
    for response in responses {
        let serialize = Instant::now();
        let text = server_packet_to_event(response).to_string();
        metrics.serialization += serialize.elapsed();
        let bytes = text.len();
        if !batch.is_empty()
            && (batch.len() == MAX_BATCH_MESSAGES || batch_bytes + bytes > MAX_BATCH_BYTES)
        {
            let timings = sender.send_batch(std::mem::take(&mut batch)).await?;
            metrics.record(timings);
            batch_bytes = 0;
        }
        metrics.bytes += bytes;
        metrics.messages += 1;
        batch.push(Message::Text(text.into()));
        batch_bytes += bytes;
        // Preserve the existing single-frame policy without retaining another
        // serialized frame beside an individually oversized response.
        if batch_bytes > MAX_BATCH_BYTES {
            let timings = sender.send_batch(std::mem::take(&mut batch)).await?;
            metrics.record(timings);
            batch_bytes = 0;
        }
    }
    if !batch.is_empty() {
        let timings = sender.send_batch(batch).await?;
        metrics.record(timings);
    }
    metrics.elapsed = started.elapsed();
    if metrics.messages > MAX_BATCH_MESSAGES
        || super::gateway_slow_stage_threshold()
            .is_some_and(|threshold| metrics.elapsed >= threshold)
    {
        // Counts and durations only: never log protocol bodies, identity or
        // resume credentials while diagnosing bootstrap backpressure.
        eprintln!(
            "[gateway-response-write] messages={} bytes={} flushes={} serialization_us={} lock_wait_us={} feed_us={} flush_us={} elapsed_us={}",
            metrics.messages, metrics.bytes, metrics.flushes,
            metrics.serialization.as_micros(), metrics.lock_wait.as_micros(),
            metrics.feed.as_micros(), metrics.flush.as_micros(), metrics.elapsed.as_micros()
        );
    }
    Ok(metrics)
}

impl ResponseWriteMetrics {
    fn record(&mut self, timings: super::transport::BatchWriteTimings) {
        self.flushes += 1;
        self.lock_wait += timings.lock_wait;
        self.feed += timings.feed;
        self.flush += timings.flush;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};

    use axum::serve::Listener;
    use futures_util::StreamExt;
    use mir2_protocol::ChatType;
    use tokio::sync::oneshot;

    #[derive(Default)]
    struct Captured {
        frames: Vec<String>,
        groups: Vec<(usize, usize)>,
        pending_count: usize,
        pending_bytes: usize,
    }

    struct CaptureSink(Arc<Mutex<Captured>>);

    impl Sink<Message> for CaptureSink {
        type Error = axum::Error;

        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, message: Message) -> Result<(), Self::Error> {
            let Message::Text(text) = message else {
                return Err(axum::Error::new(io::Error::other("expected protocol text")));
            };
            let mut state = self.0.lock().unwrap();
            state.pending_count += 1;
            state.pending_bytes += text.len();
            state.frames.push(text.to_string());
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            let mut state = self.0.lock().unwrap();
            if state.pending_count > 0 {
                let group = (state.pending_count, state.pending_bytes);
                state.groups.push(group);
                state.pending_count = 0;
                state.pending_bytes = 0;
            }
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.poll_flush(cx)
        }
    }

    fn packets(count: usize) -> Vec<ServerPacket> {
        (0..count as i64)
            .map(|time| ServerPacket::KeepAlive { time })
            .collect()
    }

    fn capture_sender() -> (SocketSender<CaptureSink>, Arc<Mutex<Captured>>) {
        let captured = Arc::new(Mutex::new(Captured::default()));
        let sender = SocketSender::new(
            CaptureSink(Arc::clone(&captured)),
            super::super::transport::TransportSignal::new(),
        );
        (sender, captured)
    }

    #[tokio::test]
    async fn catalogue_frames_keep_exact_json_and_order_with_bounded_flushes() {
        let packets = packets(97);
        let expected: Vec<_> = packets
            .iter()
            .map(|packet| server_packet_to_event(packet).to_string())
            .collect();
        let (sender, captured) = capture_sender();
        let metrics = send_responses(&sender, &packets).await.unwrap();
        let state = captured.lock().unwrap();
        assert_eq!(state.frames, expected);
        assert_eq!(metrics.messages, 97);
        assert_eq!(
            metrics.bytes,
            expected.iter().map(String::len).sum::<usize>()
        );
        assert_eq!(
            state.groups.len(),
            4,
            "one flush per bounded batch, not per frame"
        );
        assert_eq!(metrics.flushes, state.groups.len());
        assert!(state
            .groups
            .iter()
            .all(|&(count, bytes)| count <= MAX_BATCH_MESSAGES && bytes <= MAX_BATCH_BYTES));
    }

    #[tokio::test]
    async fn catalogue_utf8_bytes_and_oversized_single_frames_are_preserved() {
        let packets: Vec<_> = ["é".repeat(7_000), "z".repeat(7_000), "界".repeat(40_000)]
            .into_iter()
            .chain(std::iter::once("tail".to_owned()))
            .map(|message| ServerPacket::Chat {
                message,
                chat_type: ChatType::System,
            })
            .collect();
        let expected: Vec<_> = packets
            .iter()
            .map(|packet| server_packet_to_event(packet).to_string())
            .collect();
        let (sender, captured) = capture_sender();
        let metrics = send_responses(&sender, &packets).await.unwrap();
        let state = captured.lock().unwrap();
        assert_eq!(state.frames, expected);
        assert_eq!(
            metrics.bytes,
            expected.iter().map(String::len).sum::<usize>()
        );
        assert!(state
            .groups
            .iter()
            .all(|&(count, bytes)| count == 1 || bytes <= MAX_BATCH_BYTES));
        assert!(state
            .groups
            .iter()
            .any(|&(_, bytes)| bytes > MAX_BATCH_BYTES));
    }

    #[tokio::test]
    async fn catalogue_empty_response_list_does_not_touch_writer() {
        let (sender, captured) = capture_sender();
        let metrics = send_responses(&sender, &[]).await.unwrap();
        assert_eq!(metrics.messages, 0);
        assert_eq!(metrics.flushes, 0);
        assert!(captured.lock().unwrap().frames.is_empty());
    }

    #[tokio::test]
    async fn accepted_tcp_stream_has_nodelay_before_http_or_websocket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut listener = super::super::transport::low_latency_listener(listener);
        let connect = tokio::spawn(tokio::net::TcpStream::connect(address));
        let (stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
            .await
            .unwrap();
        assert!(
            stream.nodelay().unwrap(),
            "actual accepted socket must disable Nagle"
        );
        connect.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn catalogue_real_websocket_keeps_all_individual_frames() {
        let packets = Arc::new(packets(97));
        let expected: Vec<_> = packets
            .iter()
            .map(|packet| server_packet_to_event(packet).to_string())
            .collect();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (done, completed) = oneshot::channel();
        let done = Arc::new(Mutex::new(Some(done)));
        let router = axum::Router::new().route(
            "/ws",
            axum::routing::get(move |upgrade: axum::extract::ws::WebSocketUpgrade| {
                let packets = Arc::clone(&packets);
                let done = done.lock().unwrap().take().unwrap();
                async move {
                    upgrade.on_upgrade(move |socket| async move {
                        let (sink, _stream) = socket.split();
                        let sender = SocketSender::new(
                            sink,
                            super::super::transport::TransportSignal::new(),
                        );
                        let result = send_responses(&sender, &packets).await;
                        let _ = done.send(result.map(|metrics| metrics.messages));
                    })
                }
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(
                super::super::transport::low_latency_listener(listener),
                router,
            )
            .await
            .unwrap();
        });
        let (mut client, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
            .await
            .unwrap();
        for expected in expected {
            let frame = tokio::time::timeout(Duration::from_secs(2), client.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(frame.into_text().unwrap().as_str(), expected);
        }
        assert_eq!(completed.await.unwrap().unwrap(), 97);
        server.abort();
        let _ = server.await;
    }
}

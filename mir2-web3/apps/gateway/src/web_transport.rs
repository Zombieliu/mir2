//! A socket failure ends transport work, never authoritative persistence.
use std::io;
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::Duration;

use axum::extract::ws::Message;
use futures_util::{Sink, SinkExt};
use tokio::sync::{watch, Mutex, Notify};

pub(super) const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const MAX_BATCH_MESSAGES: usize = 32;
pub(super) const MAX_BATCH_BYTES: usize = 64 * 1024;

pub(super) fn low_latency_listener(
    listener: tokio::net::TcpListener,
) -> axum::serve::TapIo<tokio::net::TcpListener, fn(&mut tokio::net::TcpStream)> {
    use axum::serve::ListenerExt;
    listener.tap_io(configure_accepted_stream as fn(&mut tokio::net::TcpStream))
}

fn configure_accepted_stream(stream: &mut tokio::net::TcpStream) {
    if let Err(error) = stream.set_nodelay(true) {
        eprintln!("[gateway-transport] failed to set accepted TCP_NODELAY: {error}");
    }
}

#[derive(Debug, Default)]
pub(super) struct BatchWriteTimings {
    pub(super) lock_wait: Duration,
    pub(super) feed: Duration,
    pub(super) flush: Duration,
}

/// Cancellation must fence the socket before releasing its writer lock. A
/// partially fed batch is never safe for a waiting producer to reuse.
struct InFlightBatch<'a, S> {
    sink: tokio::sync::MutexGuard<'a, S>,
    terminal: &'a TransportSignal,
    completed: bool,
}

impl<S> Drop for InFlightBatch<'_, S> {
    fn drop(&mut self) {
        if !self.completed {
            self.terminal.end(TransportEnd::WriteError);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransportEnd {
    Closed,
    ReadError,
    ReaderStopped,
    WriteError,
    WriteTimeout,
    ZoneOverload(u64),
}

#[derive(Clone)]
pub(super) struct TransportSignal(watch::Sender<Option<TransportEnd>>);

impl TransportSignal {
    pub(super) fn new() -> Self {
        Self(watch::channel(None).0)
    }

    pub(super) fn subscribe(&self) -> watch::Receiver<Option<TransportEnd>> {
        self.0.subscribe()
    }

    pub(super) fn end(&self, reason: TransportEnd) {
        self.0.send_if_modified(|current| {
            if current.is_some() {
                return false;
            }
            *current = Some(reason);
            true
        });
    }

    pub(super) fn ended(&self) -> bool {
        self.0.borrow().is_some()
    }

    pub(super) fn reason(&self) -> Option<TransportEnd> {
        *self.0.borrow()
    }
}

pub(super) async fn wait_for_end(
    mut receiver: watch::Receiver<Option<TransportEnd>>,
) -> TransportEnd {
    loop {
        if let Some(reason) = *receiver.borrow_and_update() {
            return reason;
        }
        if receiver.changed().await.is_err() {
            // A dropped producer is not evidence that a live session should
            // be discarded. The owner work future still controls its lifetime.
            return std::future::pending().await;
        }
    }
}

/// Signals abnormal reader completion even on an early return or panic. This
/// bypasses the bounded gameplay queue; no Disconnect/LogOut is fabricated.
pub(super) struct ReaderLifetime(pub(super) TransportSignal);

impl Drop for ReaderLifetime {
    fn drop(&mut self) {
        self.0.end(TransportEnd::ReaderStopped);
    }
}

pub(super) struct SocketSender<S> {
    sink: Mutex<S>,
    terminal: TransportSignal,
    timeout: Duration,
}

impl<S> SocketSender<S>
where
    S: Sink<Message, Error = axum::Error> + Unpin,
{
    pub(super) fn new(sink: S, terminal: TransportSignal) -> Self {
        Self {
            sink: Mutex::new(sink),
            terminal,
            timeout: WRITE_TIMEOUT,
        }
    }

    pub(super) async fn send(&self, message: Message) -> Result<(), axum::Error> {
        if self.terminal.ended() {
            return Err(ended_error());
        }
        let deadline = tokio::time::Instant::now() + self.timeout;
        let mut sink = match tokio::time::timeout_at(deadline, self.sink.lock()).await {
            Ok(sink) => sink,
            Err(_) => {
                self.terminal.end(TransportEnd::WriteTimeout);
                return Err(timeout_error());
            }
        };
        if self.terminal.ended() {
            return Err(ended_error());
        }
        // Latch a failed/partial write while still holding the sink guard.
        // A waiting producer must not acquire and reuse a partially flushed
        // sink in the interval between cancellation and the terminal signal.
        match tokio::time::timeout_at(deadline, sink.send(message)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.terminal.end(TransportEnd::WriteError);
                Err(error)
            }
            Err(_) => {
                self.terminal.end(TransportEnd::WriteTimeout);
                Err(timeout_error())
            }
        }
    }

    pub(super) async fn send_batch(
        &self,
        messages: Vec<Message>,
    ) -> Result<BatchWriteTimings, axum::Error> {
        if messages.is_empty() {
            return Ok(BatchWriteTimings::default());
        }
        let bytes = messages.iter().map(message_bytes).sum::<usize>();
        if messages.len() > MAX_BATCH_MESSAGES || (messages.len() > 1 && bytes > MAX_BATCH_BYTES) {
            return Err(axum::Error::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                "WebSocket batch exceeds message/byte bound",
            )));
        }
        if self.terminal.ended() {
            return Err(ended_error());
        }
        let deadline = tokio::time::Instant::now() + self.timeout;
        let lock_started = std::time::Instant::now();
        let sink = match tokio::time::timeout_at(deadline, self.sink.lock()).await {
            Ok(sink) => sink,
            Err(_) => {
                self.terminal.end(TransportEnd::WriteTimeout);
                return Err(timeout_error());
            }
        };
        let mut timings = BatchWriteTimings {
            lock_wait: lock_started.elapsed(),
            ..BatchWriteTimings::default()
        };
        if self.terminal.ended() {
            return Err(ended_error());
        }
        let mut write = InFlightBatch {
            sink,
            terminal: &self.terminal,
            completed: false,
        };
        // The same ten-second deadline covers lock, readiness, feed and final
        // flush. Every message remains an individual WebSocket frame.
        let result = tokio::time::timeout_at(deadline, async {
            let feed_started = std::time::Instant::now();
            for message in messages {
                if self.terminal.ended() {
                    return Err(ended_error());
                }
                write.sink.feed(message).await?;
            }
            timings.feed = feed_started.elapsed();
            if self.terminal.ended() {
                return Err(ended_error());
            }
            let flush_started = std::time::Instant::now();
            write.sink.flush().await?;
            timings.flush = flush_started.elapsed();
            Ok(())
        })
        .await;
        match result {
            Ok(Ok(())) => {
                write.completed = true;
                Ok(timings)
            }
            Ok(Err(error)) => {
                self.terminal.end(TransportEnd::WriteError);
                Err(error)
            }
            Err(_) => {
                self.terminal.end(TransportEnd::WriteTimeout);
                Err(timeout_error())
            }
        }
    }

    #[cfg(test)]
    pub(super) async fn lock_for_test(&self) -> tokio::sync::MutexGuard<'_, S> {
        self.sink.lock().await
    }
}

fn message_bytes(message: &Message) -> usize {
    match message {
        Message::Text(text) => text.len(),
        Message::Binary(bytes) | Message::Ping(bytes) | Message::Pong(bytes) => bytes.len(),
        Message::Close(frame) => frame.as_ref().map_or(0, |frame| 2 + frame.reason.len()),
    }
}

#[cfg(test)]
#[path = "web_transport_batch_tests.rs"]
mod batch_tests;

fn ended_error() -> axum::Error {
    axum::Error::new(io::Error::new(
        io::ErrorKind::BrokenPipe,
        "WebSocket transport has ended",
    ))
}

fn timeout_error() -> axum::Error {
    axum::Error::new(io::Error::new(
        io::ErrorKind::TimedOut,
        "WebSocket lock/write deadline exceeded",
    ))
}

#[derive(Default)]
struct MovementDrainState {
    closed: bool,
    pending: usize,
}

/// Abort does not cancel spawn_blocking work. Stop admission and let already
/// submitted calls finish before fencing/saving the shared owner's checkpoint.
#[derive(Default)]
pub(super) struct MovementDrain {
    state: SyncMutex<MovementDrainState>,
    completed: Notify,
}

impl MovementDrain {
    pub(super) fn begin(self: &Arc<Self>) -> Option<MovementCall> {
        let mut state = self.state.lock().expect("movement drain state poisoned");
        if state.closed {
            return None;
        }
        state.pending += 1;
        Some(MovementCall(Arc::clone(self)))
    }

    pub(super) async fn close_and_wait(&self) {
        loop {
            {
                let mut state = self.state.lock().expect("movement drain state poisoned");
                state.closed = true;
                if state.pending == 0 {
                    return;
                }
            }
            // notify_one retains a permit if completion races this await.
            self.completed.notified().await;
        }
    }
}

pub(super) struct MovementCall(Arc<MovementDrain>);

impl Drop for MovementCall {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().expect("movement drain state poisoned");
        state.pending -= 1;
        if state.pending == 0 {
            self.0.completed.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll};

    struct ProbeSink {
        ready: bool,
        flush: bool,
        fail: bool,
        sent: Arc<AtomicUsize>,
    }

    impl Sink<Message> for ProbeSink {
        type Error = axum::Error;

        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.fail {
                Poll::Ready(Err(axum::Error::new(io::Error::other("closed sink"))))
            } else if self.ready {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }

        fn start_send(self: Pin<&mut Self>, _: Message) -> Result<(), Self::Error> {
            self.sent.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.flush {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.poll_flush(cx)
        }
    }

    fn sender(ready: bool, flush: bool, fail: bool) -> (SocketSender<ProbeSink>, Arc<AtomicUsize>) {
        let sent = Arc::new(AtomicUsize::new(0));
        let mut sender = SocketSender::new(
            ProbeSink {
                ready,
                flush,
                fail,
                sent: Arc::clone(&sent),
            },
            TransportSignal::new(),
        );
        sender.timeout = Duration::from_millis(10);
        (sender, sent)
    }

    fn message() -> Message {
        Message::Text("test".into())
    }

    #[tokio::test]
    async fn transport_ready_and_flush_complete_without_ending_socket() {
        let (sender, sent) = sender(true, true, false);
        sender.send(message()).await.unwrap();
        sender.send(message()).await.unwrap();
        assert_eq!(sent.load(Ordering::SeqCst), 2);
        assert!(!sender.terminal.ended());
    }

    #[tokio::test]
    async fn transport_pending_readiness_expires_before_start_send() {
        let (sender, sent) = sender(false, true, false);
        assert!(sender.send(message()).await.is_err());
        assert_eq!(sent.load(Ordering::SeqCst), 0);
        assert_eq!(
            wait_for_end(sender.terminal.subscribe()).await,
            TransportEnd::WriteTimeout
        );
    }

    #[tokio::test]
    async fn transport_partial_flush_expires_and_never_reuses_sink() {
        let (sender, sent) = sender(true, false, false);
        assert!(sender.send(message()).await.is_err());
        assert_eq!(sent.load(Ordering::SeqCst), 1);
        assert!(sender.send(message()).await.is_err());
        assert_eq!(sent.load(Ordering::SeqCst), 1);
        assert_eq!(
            wait_for_end(sender.terminal.subscribe()).await,
            TransportEnd::WriteTimeout
        );
    }

    #[tokio::test]
    async fn transport_lock_wait_shares_write_deadline() {
        let (sender, sent) = sender(true, true, false);
        let guard = sender.sink.lock().await;
        assert!(sender.send(message()).await.is_err());
        drop(guard);
        assert!(sender.send(message()).await.is_err());
        assert_eq!(sent.load(Ordering::SeqCst), 0);
        assert_eq!(
            wait_for_end(sender.terminal.subscribe()).await,
            TransportEnd::WriteTimeout
        );
    }

    #[tokio::test]
    async fn transport_sink_error_ends_all_other_producers() {
        let (sender, sent) = sender(true, true, true);
        assert!(sender.send(message()).await.is_err());
        assert!(sender.send(message()).await.is_err());
        assert_eq!(sent.load(Ordering::SeqCst), 0);
        assert_eq!(
            wait_for_end(sender.terminal.subscribe()).await,
            TransportEnd::WriteError
        );
    }

    #[tokio::test]
    async fn transport_close_bypasses_full_gameplay_queue_and_preserves_first_reason() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        tx.try_send("unconsumed action").unwrap();
        let terminal = TransportSignal::new();
        let receiver = terminal.subscribe();
        terminal.end(TransportEnd::Closed);
        terminal.end(TransportEnd::WriteError);
        assert_eq!(wait_for_end(receiver).await, TransportEnd::Closed);
        assert!(tx.try_send("close notification would block").is_err());
    }

    #[tokio::test]
    async fn transport_reader_early_return_is_terminal() {
        let terminal = TransportSignal::new();
        let receiver = terminal.subscribe();
        drop(ReaderLifetime(terminal));
        assert_eq!(wait_for_end(receiver).await, TransportEnd::ReaderStopped);
    }

    #[tokio::test]
    async fn transport_movement_drain_waits_for_blocking_work_and_rejects_new_calls() {
        let drain = Arc::new(MovementDrain::default());
        let call = drain.begin().unwrap();
        let waiting = drain.close_and_wait();
        tokio::pin!(waiting);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut waiting)
                .await
                .is_err()
        );
        assert!(drain.begin().is_none());
        drop(call);
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .unwrap();
        assert!(drain.begin().is_none());
    }
}

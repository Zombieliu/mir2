use super::*;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};

struct BatchSink {
    sent: Arc<AtomicUsize>,
    flushes: Arc<AtomicUsize>,
    ready_until: usize,
    error_at_limit: bool,
    flush_ready: bool,
}

impl Sink<Message> for BatchSink {
    type Error = axum::Error;

    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.sent.load(Ordering::SeqCst) < self.ready_until {
            Poll::Ready(Ok(()))
        } else if self.error_at_limit {
            Poll::Ready(Err(axum::Error::new(io::Error::other(
                "fixture write error",
            ))))
        } else {
            Poll::Pending
        }
    }

    fn start_send(self: Pin<&mut Self>, _: Message) -> Result<(), Self::Error> {
        self.sent.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        if self.flush_ready {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}

fn sender(
    ready_until: usize,
    error_at_limit: bool,
    flush_ready: bool,
) -> (SocketSender<BatchSink>, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let sent = Arc::new(AtomicUsize::new(0));
    let flushes = Arc::new(AtomicUsize::new(0));
    let mut sender = SocketSender::new(
        BatchSink {
            sent: Arc::clone(&sent),
            flushes: Arc::clone(&flushes),
            ready_until,
            error_at_limit,
            flush_ready,
        },
        TransportSignal::new(),
    );
    sender.timeout = Duration::from_millis(15);
    (sender, sent, flushes)
}

fn messages() -> Vec<Message> {
    (0..3)
        .map(|index| Message::Text(index.to_string().into()))
        .collect()
}

#[tokio::test]
async fn batch_success_preserves_frames_and_flushes_once() {
    let (sender, sent, flushes) = sender(usize::MAX, false, true);
    sender.send_batch(messages()).await.unwrap();
    assert_eq!(sent.load(Ordering::SeqCst), 3);
    assert_eq!(flushes.load(Ordering::SeqCst), 1);
    assert!(!sender.terminal.ended());
    sender.send(Message::Text("next".into())).await.unwrap();
    assert_eq!(sent.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn batch_partial_readiness_timeout_fences_all_producers() {
    let (sender, sent, flushes) = sender(2, false, true);
    assert!(sender.send_batch(messages()).await.is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 2);
    assert_eq!(flushes.load(Ordering::SeqCst), 0);
    assert_eq!(sender.terminal.reason(), Some(TransportEnd::WriteTimeout));
    assert!(sender.send_batch(messages()).await.is_err());
    assert!(sender
        .send(Message::Text("must not write".into()))
        .await
        .is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn batch_partial_flush_timeout_fences_waiting_single_writer() {
    let (sender, sent, _) = sender(usize::MAX, false, false);
    let sender = Arc::new(sender);
    let first = Arc::clone(&sender);
    let batch = tokio::spawn(async move { first.send_batch(messages()).await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while sent.load(Ordering::SeqCst) != 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let other = Arc::clone(&sender);
    let waiting = tokio::spawn(async move { other.send(Message::Text("waiting".into())).await });
    assert!(batch.await.unwrap().is_err());
    assert!(waiting.await.unwrap().is_err());
    assert_eq!(sender.terminal.reason(), Some(TransportEnd::WriteTimeout));
    assert_eq!(sent.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn batch_lock_wait_uses_original_write_deadline() {
    let (sender, sent, _) = sender(usize::MAX, false, true);
    let guard = sender.sink.lock().await;
    assert!(sender.send_batch(messages()).await.is_err());
    assert_eq!(sender.terminal.reason(), Some(TransportEnd::WriteTimeout));
    drop(guard);
    assert!(sender.send_batch(messages()).await.is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 0);
    assert_eq!(WRITE_TIMEOUT, Duration::from_secs(10));
}

#[tokio::test]
async fn batch_feed_error_cannot_flush_or_reuse_partial_frames() {
    let (sender, sent, flushes) = sender(2, true, true);
    assert!(sender.send_batch(messages()).await.is_err());
    assert_eq!(sender.terminal.reason(), Some(TransportEnd::WriteError));
    assert_eq!(sent.load(Ordering::SeqCst), 2);
    assert_eq!(flushes.load(Ordering::SeqCst), 0);
    assert!(sender
        .send(Message::Text("after failure".into()))
        .await
        .is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn batch_external_cancellation_fences_sink_before_unlock() {
    let (mut sender, sent, _) = sender(usize::MAX, false, false);
    sender.timeout = Duration::from_secs(1);
    assert!(
        tokio::time::timeout(Duration::from_millis(5), sender.send_batch(messages()))
            .await
            .is_err()
    );
    assert_eq!(sent.load(Ordering::SeqCst), 3);
    assert_eq!(sender.terminal.reason(), Some(TransportEnd::WriteError));
    assert!(sender
        .send(Message::Text("after cancellation".into()))
        .await
        .is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn batch_size_rejection_is_before_any_sink_write() {
    let (sender, sent, flushes) = sender(usize::MAX, false, true);
    let too_many = (0..=MAX_BATCH_MESSAGES)
        .map(|_| Message::Text("x".into()))
        .collect();
    assert!(sender.send_batch(too_many).await.is_err());
    let too_large = vec![
        Message::Text("x".repeat(MAX_BATCH_BYTES).into()),
        Message::Text("y".into()),
    ];
    assert!(sender.send_batch(too_large).await.is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 0);
    assert_eq!(flushes.load(Ordering::SeqCst), 0);
    assert!(!sender.terminal.ended());
    sender
        .send(Message::Text("ordinary frame".into()))
        .await
        .unwrap();
}

#[tokio::test]
async fn batch_reader_close_keeps_original_terminal_reason() {
    let (sender, sent, _) = sender(usize::MAX, false, true);
    sender.terminal.end(TransportEnd::Closed);
    assert!(sender.send_batch(messages()).await.is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 0);
    assert_eq!(sender.terminal.reason(), Some(TransportEnd::Closed));
}

use super::super::transport::TransportEnd;
use super::*;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

fn key() -> GatewaySessionCacheKey {
    GatewaySessionCacheKey {
        account_id: "handoff-fixture".into(),
        character_index: 7,
    }
}

fn pending<F: Future>(future: Pin<&mut F>) {
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(future.poll(&mut context), Poll::Pending));
}

fn closed() -> TransportSignal {
    let terminal = TransportSignal::new();
    terminal.end(TransportEnd::Closed);
    terminal
}

#[test]
fn live_and_unknown_tcp_or_foreign_owner_cannot_wait() {
    let registry = Arc::new(HandoffRegistry::default());
    let terminal = TransportSignal::new();
    let _binding = registry
        .bind(key(), "web-owner".into(), terminal.clone())
        .unwrap();
    assert!(registry.waiter(&key(), "web-owner").is_none());
    assert!(registry.waiter(&key(), "tcp-owner").is_none());
    terminal.end(TransportEnd::Closed);
    assert!(registry.waiter(&key(), "tcp-owner").is_none());
    assert!(registry.waiter(&key(), "Web-owner").is_none());
    assert!(registry
        .waiter(
            &GatewaySessionCacheKey {
                character_index: 8,
                ..key()
            },
            "web-owner"
        )
        .is_none());
    assert!(registry
        .waiter(
            &GatewaySessionCacheKey {
                account_id: "other".into(),
                ..key()
            },
            "web-owner"
        )
        .is_none());
    assert!(registry.waiter(&key(), "web-owner").is_some());
}

#[tokio::test]
async fn close_waits_for_ready_without_holding_registry_lock() {
    let registry = Arc::new(HandoffRegistry::default());
    let terminal = TransportSignal::new();
    let binding = registry
        .bind(key(), "web-owner".into(), terminal.clone())
        .unwrap();
    let token = binding.token();
    terminal.end(TransportEnd::Closed);
    let waiter = registry.waiter(&key(), "web-owner").unwrap();
    assert!(waiter.token().same(&token));
    let mut waiting = Box::pin(waiter.wait());
    pending(waiting.as_mut());
    assert!(registry.entries.try_lock().is_ok());
    binding.ready();
    assert!(waiting.await.unwrap().same(&token));
    assert!(registry.waiter(&key(), "web-owner").is_none());
}

#[tokio::test]
async fn ready_before_first_poll_is_retained() {
    let registry = Arc::new(HandoffRegistry::default());
    let binding = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let waiter = registry.waiter(&key(), "web-owner").unwrap();
    let token = binding.token();
    binding.ready();
    assert!(waiter.wait().await.unwrap().same(&token));
}

#[tokio::test]
async fn ready_broadcasts_to_all_existing_waiters_without_consuming_the_token() {
    let registry = Arc::new(HandoffRegistry::default());
    let binding = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let token = binding.token();
    let waiters: Vec<_> = (0..4)
        .map(|_| registry.waiter(&key(), "web-owner").unwrap())
        .collect();
    binding.ready();
    for waiter in waiters {
        assert!(waiter.wait().await.unwrap().same(&token));
    }
}

#[tokio::test]
async fn unfinished_guard_drop_fails_every_waiter_and_removes_its_binding() {
    let registry = Arc::new(HandoffRegistry::default());
    let binding = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let token = binding.token();
    let first = registry.waiter(&key(), "web-owner").unwrap();
    let second = registry.waiter(&key(), "web-owner").unwrap();
    let mut waiting = Box::pin(first.wait());
    pending(waiting.as_mut());
    drop(binding);
    assert_eq!(waiting.await.unwrap_err(), HandoffFailed);
    assert_eq!(second.wait().await.unwrap_err(), HandoffFailed);
    let next = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    assert!(!next.token().same(&token));
}

#[tokio::test]
async fn same_key_and_owner_get_a_new_identity_and_old_waiter_cannot_complete_it() {
    let registry = Arc::new(HandoffRegistry::default());
    let old = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let old_token = old.token();
    let old_waiter = registry.waiter(&key(), "web-owner").unwrap();
    old.ready();
    let next = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let next_token = next.token();
    assert_eq!(old_token.owner(), next_token.owner());
    assert!(!old_token.same(&next_token));
    assert!(old_token.same(&old_token.clone()));
    assert!(old_waiter.wait().await.unwrap().same(&old_token));
    let mut waiting = Box::pin(registry.waiter(&key(), "web-owner").unwrap().wait());
    pending(waiting.as_mut());
    next.ready();
    assert!(waiting.await.unwrap().same(&next_token));
}

#[tokio::test]
async fn stale_guard_drop_cannot_remove_same_key_and_owner_successor() {
    let registry = Arc::new(HandoffRegistry::default());
    let old = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let old_waiter = registry.waiter(&key(), "web-owner").unwrap();
    // Model delayed cleanup after an entry was invalidated. Replacement is not a public API.
    registry.entries.lock().unwrap().0.remove(&key());
    let next = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let next_token = next.token();
    drop(old);
    assert_eq!(old_waiter.wait().await.unwrap_err(), HandoffFailed);
    let next_waiter = registry.waiter(&key(), "web-owner").unwrap();
    assert!(next_waiter.token().same(&next_token));
    next.ready();
    assert!(next_waiter.wait().await.unwrap().same(&next_token));
}

#[tokio::test]
async fn stale_guard_ready_fails_closed_and_cannot_complete_successor() {
    let registry = Arc::new(HandoffRegistry::default());
    let old = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let old_waiter = registry.waiter(&key(), "web-owner").unwrap();
    registry.entries.lock().unwrap().0.remove(&key());
    let next = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let next_token = next.token();
    let mut waiting = Box::pin(registry.waiter(&key(), "web-owner").unwrap().wait());
    old.ready();
    assert_eq!(old_waiter.wait().await.unwrap_err(), HandoffFailed);
    pending(waiting.as_mut());
    assert!(registry
        .waiter(&key(), "web-owner")
        .unwrap()
        .token()
        .same(&next_token));
    next.ready();
    assert!(waiting.await.unwrap().same(&next_token));
}

#[tokio::test]
async fn canceling_waiter_does_not_consume_pending_or_ready_for_another_waiter() {
    let registry = Arc::new(HandoffRegistry::default());
    let binding = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let token = binding.token();
    let mut canceled = Box::pin(registry.waiter(&key(), "web-owner").unwrap().wait());
    pending(canceled.as_mut());
    drop(canceled);
    let after_ready_cancel = registry.waiter(&key(), "web-owner").unwrap();
    let remaining = registry.waiter(&key(), "web-owner").unwrap();
    binding.ready();
    drop(after_ready_cancel);
    assert!(remaining.wait().await.unwrap().same(&token));
}

#[test]
fn registration_conflict_never_replaces_live_or_closed_binding() {
    let registry = Arc::new(HandoffRegistry::default());
    let terminal = TransportSignal::new();
    let binding = registry
        .bind(key(), "web-owner".into(), terminal.clone())
        .unwrap();
    let token = binding.token();
    for same_owner in [true, false] {
        let owner = if same_owner {
            "web-owner"
        } else {
            "other-owner"
        };
        assert!(registry.bind(key(), owner.into(), closed()).is_none());
    }
    terminal.end(TransportEnd::ReadError);
    assert!(registry.bind(key(), "web-owner".into(), closed()).is_none());
    assert!(registry
        .waiter(&key(), "web-owner")
        .unwrap()
        .token()
        .same(&token));
    let other_key = GatewaySessionCacheKey {
        character_index: 8,
        ..key()
    };
    let other = registry
        .bind(other_key, "web-owner".into(), closed())
        .unwrap();
    assert!(!other.token().same(&token));
    drop(binding);
    assert!(registry.bind(key(), "web-owner".into(), closed()).is_some());
}

#[tokio::test]
async fn a_closed_notification_channel_without_ready_is_failure() {
    let registry = Arc::new(HandoffRegistry::default());
    let binding = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let (sender, state) = watch::channel(HandoffState::Pending);
    let waiter = HandoffWaiter {
        token: binding.token(),
        state,
    };
    drop(sender);
    assert_eq!(waiter.wait().await.unwrap_err(), HandoffFailed);
}

#[tokio::test]
async fn poisoned_registry_cannot_announce_ready_or_register_waiters() {
    let registry = Arc::new(HandoffRegistry::default());
    let binding = registry.bind(key(), "web-owner".into(), closed()).unwrap();
    let waiter = registry.waiter(&key(), "web-owner").unwrap();
    let poison = Arc::clone(&registry);
    let _ = std::panic::catch_unwind(move || {
        let _lock = poison.entries.lock().unwrap();
        panic!("fixture poison");
    });
    assert!(registry.bind(key(), "web-owner".into(), closed()).is_none());
    assert!(registry.waiter(&key(), "web-owner").is_none());
    binding.ready();
    assert_eq!(waiter.wait().await.unwrap_err(), HandoffFailed);
}

#[test]
fn debug_and_failure_display_do_not_expose_identity_fields() {
    let registry = Arc::new(HandoffRegistry::default());
    let binding = registry
        .bind(key(), "private-route-owner".into(), closed())
        .unwrap();
    let debug = format!("{registry:?} {:?} {HandoffFailed}", binding.token());
    assert!(!debug.contains("private-route-owner"));
    assert!(!debug.contains("handoff-fixture"));
    assert!(!debug.contains("character_index"));
}

#[test]
fn registry_tokens_bindings_and_waiters_can_cross_executor_threads() {
    fn send<T: Send>() {}
    fn send_sync<T: Send + Sync>() {}
    send_sync::<HandoffRegistry>();
    send_sync::<HandoffToken>();
    send::<HandoffBinding>();
    send::<HandoffWaiter>();
}

//! Store-level handoff regression tests. No listener, live data, admin packet or demo login.
use super::reconnect_handoff::HandoffWaiter;
use super::transport::{TransportEnd, TransportSignal};
use super::*;
use crate::cache::{GatewaySessionCache, InMemoryGatewaySessionCache};
use mir2_protocol::{MirClass, MirGender};
use mir2_simulation::SimulationConfig;
use std::future::Future;
use std::pin::Pin;
use std::sync::Barrier;
use std::task::{Context, Poll, Waker};

const ACCOUNT: &str = "handoffowner";

struct Fixture {
    store: Arc<ReconnectSessionStore>,
    capacity: Arc<GatewayCapacityState>,
    key: GatewaySessionCacheKey,
    owner: String,
    identity: IdentityService,
    verified: VerifiedIdentitySession,
    session: Option<crate::GatewaySession>,
}

fn issue_identity(identity: &IdentityService, account: &str) -> VerifiedIdentitySession {
    let grant = identity
        .issue_session(
            account,
            "password",
            account,
            "127.0.0.1",
            "handoff-store-unit",
        )
        .expect("issue a real memory identity grant");
    identity
        .verify_session_token(&grant.token)
        .expect("verify the issued grant")
}

impl Fixture {
    fn new() -> Self {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        let registration = session.handle_packet(ClientPacket::NewAccount {
            account_id: ACCOUNT.into(),
            password: "HandoffFixture42".into(),
            birth_date_binary: 0,
            user_name: "Handoff Fixture".into(),
            secret_question: "fixture".into(),
            secret_answer: "fixture".into(),
            email_address: "handoff@example.test".into(),
        });
        assert!(registration
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })));
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: ACCOUNT.into(),
                password: "HandoffFixture42".into(),
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let created = session.handle_packet(ClientPacket::NewCharacter {
            name: "HandoffHero".into(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        });
        let character_index = created
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("ordinary creation returns the actual character index");
        assert!(session
            .handle_packet(ClientPacket::StartGame { character_index })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        let active = session
            .active_identity()
            .expect("ordinary StartGame is active");
        assert_eq!(active.account_id, ACCOUNT);
        assert_eq!(active.character_index, character_index);
        let owner = session.session_id().to_owned();
        let identity = IdentityService::local_for_tests();
        let verified = issue_identity(&identity, ACCOUNT);
        Self {
            store: Arc::new(ReconnectSessionStore::default()),
            capacity: Arc::new(GatewayCapacityState::with_limits(None, Some(1), Some(1))),
            key: GatewaySessionCacheKey {
                account_id: ACCOUNT.into(),
                character_index,
            },
            owner,
            identity,
            verified,
            session: Some(session),
        }
    }

    fn authorization(&self) -> HandoffAuthorization {
        self.store
            .capture_handoff_authorization(&self.verified, gateway_unix_ms())
            .expect("current verified identity may begin waiting")
    }

    fn bind(&self, terminal: TransportSignal) -> HandoffBinding {
        self.store
            .handoffs
            .bind(self.key.clone(), self.owner.clone(), terminal)
            .expect("bind this server-owned socket")
    }

    fn waiter(&self) -> HandoffWaiter {
        self.store
            .handoffs
            .waiter(&self.key, &self.owner)
            .expect("the closed socket has a waitable binding")
    }

    fn publish(&mut self, binding: Option<HandoffBinding>) {
        let session = self
            .session
            .take()
            .expect("first teardown owns the session");
        let active = self.capacity.try_acquire_active_session().unwrap();
        self.publish_parts(session, Some(active), binding);
    }

    fn publish_parts(
        &self,
        session: crate::GatewaySession,
        active: Option<GatewayCapacityPermit>,
        binding: Option<HandoffBinding>,
    ) {
        let reconnect = self.capacity.try_acquire_reconnect_lease().unwrap();
        self.store.store_with_handoff(
            self.key.clone(),
            session,
            active,
            reconnect,
            None,
            Duration::from_secs(30),
            binding,
        );
    }

    fn ready(&mut self) -> HandoffToken {
        let binding = self.bind(closed());
        let token = binding.token();
        self.publish(Some(binding));
        token
    }

    fn take(
        &self,
        token: &HandoffToken,
        authorization: &HandoffAuthorization,
    ) -> Option<ReconnectSessionRestore> {
        self.store
            .take_after_handoff(&self.key, token, authorization, gateway_unix_ms())
    }
}

fn closed() -> TransportSignal {
    let terminal = TransportSignal::new();
    terminal.end(TransportEnd::Closed);
    terminal
}

fn pending<F: Future>(future: Pin<&mut F>) {
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(future.poll(&mut context), Poll::Pending));
}

fn snapshot(session: &crate::GatewaySession) -> Value {
    serde_json::to_value(session.world_snapshot())
        .expect("serialize the actual authoritative world")
}

fn stored_snapshot(fixture: &Fixture) -> Value {
    let state = fixture.store.state.lock().unwrap();
    snapshot(
        &state
            .sessions
            .get(&fixture.key)
            .expect("saved lease")
            .session,
    )
}

#[tokio::test]
async fn notification_follows_publication_consumes_once_and_preserves_the_world() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let before = snapshot(f.session.as_ref().unwrap());
    let binding = f.bind(closed());
    let token = binding.token();
    let mut wait = Box::pin(f.waiter().wait());
    pending(wait.as_mut());
    assert!(
        f.take(&token, &authorization).is_none(),
        "a closed socket is not a saved lease"
    );
    f.publish(Some(binding));
    assert_eq!(stored_snapshot(&f), before);
    let notified = wait.await.unwrap();
    assert!(notified.same(&token));
    assert_eq!(
        stored_snapshot(&f),
        before,
        "observing Ready does not mutate the world"
    );
    let restored = f.take(&notified, &authorization).expect("one saved lease");
    assert_eq!(restored.session.session_id(), f.owner);
    assert!(restored.active_session_permit.is_some());
    assert_eq!(
        snapshot(&restored.session),
        before,
        "consumption only moves the same session"
    );
    assert!(f.take(&notified, &authorization).is_none());
    assert_eq!(f.store.len(), 0);
    assert_eq!(
        f.capacity.current_active_sessions.load(Ordering::Acquire),
        1
    );
    assert_eq!(
        f.capacity.current_reconnect_leases.load(Ordering::Acquire),
        0
    );
    drop(restored);
    assert_eq!(
        f.capacity.current_active_sessions.load(Ordering::Acquire),
        0
    );
}

#[tokio::test]
async fn two_notified_waiters_compete_for_exactly_one_saved_session() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let binding = f.bind(closed());
    let first = f.waiter();
    let second = f.waiter();
    f.publish(Some(binding));
    let first = first.wait().await.unwrap();
    let second = second.wait().await.unwrap();
    assert!(first.same(&second));
    let store = Arc::clone(&f.store);
    let key = f.key.clone();
    let barrier = Barrier::new(3);
    let winners = std::thread::scope(|threads| {
        let one = threads.spawn(|| {
            barrier.wait();
            store
                .take_after_handoff(&key, &first, &authorization, gateway_unix_ms())
                .is_some()
        });
        let two = threads.spawn(|| {
            barrier.wait();
            store
                .take_after_handoff(&key, &second, &authorization, gateway_unix_ms())
                .is_some()
        });
        barrier.wait();
        usize::from(one.join().unwrap()) + usize::from(two.join().unwrap())
    });
    assert_eq!(winners, 1);
    assert_eq!(f.store.len(), 0);
    assert_eq!(
        f.capacity.current_active_sessions.load(Ordering::Acquire),
        0
    );
    assert_eq!(
        f.capacity.current_reconnect_leases.load(Ordering::Acquire),
        0
    );
}

#[test]
fn an_old_token_with_the_same_owner_cannot_consume_a_new_teardown() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let old = f.ready();
    let restored = f.take(&old, &authorization).unwrap();
    let before = snapshot(&restored.session);
    let next_binding = f.bind(closed());
    let next = next_binding.token();
    assert_eq!(old.owner(), next.owner());
    assert!(!old.same(&next));
    f.publish_parts(
        restored.session,
        restored.active_session_permit,
        Some(next_binding),
    );
    assert!(f.take(&old, &authorization).is_none());
    assert_eq!(f.store.len(), 1);
    assert_eq!(stored_snapshot(&f), before);
    assert_eq!(
        snapshot(&f.take(&next, &authorization).unwrap().session),
        before
    );
}

#[test]
fn wrong_key_account_owner_or_token_never_consumes_the_legitimate_lease() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let token = f.ready();
    let before = stored_snapshot(&f);
    let mut wrong = f.key.clone();
    wrong.character_index += 1;
    assert!(f
        .store
        .take_after_handoff(&wrong, &token, &authorization, gateway_unix_ms())
        .is_none());
    wrong.account_id = "otherowner".into();
    assert!(f
        .store
        .take_after_handoff(&wrong, &token, &authorization, gateway_unix_ms())
        .is_none());
    let foreign_identity = issue_identity(&f.identity, "otherowner");
    let foreign_authorization = f
        .store
        .capture_handoff_authorization(&foreign_identity, gateway_unix_ms())
        .unwrap();
    assert!(f.take(&token, &foreign_authorization).is_none());
    let other_registry = Arc::new(HandoffRegistry::default());
    let wrong_token = other_registry
        .bind(f.key.clone(), f.owner.clone(), closed())
        .unwrap();
    assert_eq!(wrong_token.token().owner(), token.owner());
    assert!(f.take(&wrong_token.token(), &authorization).is_none());
    let foreign_registry = Arc::new(HandoffRegistry::default());
    let wrong_owner = foreign_registry
        .bind(f.key.clone(), "foreign-socket".into(), closed())
        .unwrap();
    assert!(f.take(&wrong_owner.token(), &authorization).is_none());
    assert!(f.store.handoffs.waiter(&f.key, "foreign-socket").is_none());
    assert_eq!(f.store.len(), 1);
    assert_eq!(stored_snapshot(&f), before);
    assert!(f.take(&token, &authorization).is_some());
}

#[tokio::test]
async fn a_ready_notification_cannot_authorize_a_lease_without_its_bound_token() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let binding = f.bind(closed());
    let waiter = f.waiter();
    f.publish(None);
    let before = stored_snapshot(&f);
    binding.ready();
    let token = waiter.wait().await.unwrap();
    assert!(
        f.take(&token, &authorization).is_none(),
        "Ready alone conveys no lease authority"
    );
    assert_eq!(f.store.len(), 1);
    assert_eq!(stored_snapshot(&f), before);
}

#[test]
fn expired_saved_lease_cannot_be_consumed_even_with_a_current_identity_and_token() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let token = f.ready();
    // Deterministic private lease-clock boundary, without sleeping or changing the world clock.
    f.store
        .state
        .lock()
        .unwrap()
        .sessions
        .get_mut(&f.key)
        .unwrap()
        .expires_at = Instant::now()
        .checked_sub(Duration::from_millis(1))
        .unwrap();
    assert!(f
        .store
        .handoff_authorization_current(&authorization, gateway_unix_ms()));
    assert!(f.take(&token, &authorization).is_none());
    assert_eq!(f.store.len(), 0);
    assert_eq!(
        f.capacity.current_active_sessions.load(Ordering::Acquire),
        0
    );
    assert_eq!(
        f.capacity.current_reconnect_leases.load(Ordering::Acquire),
        0
    );
}

#[test]
fn identity_expiry_is_checked_at_capture_revalidation_and_consumption() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let token = f.ready();
    let expired_at = f.verified.expires_at_ms;
    assert!(f
        .store
        .capture_handoff_authorization(&f.verified, expired_at)
        .is_none());
    assert!(!f
        .store
        .handoff_authorization_current(&authorization, expired_at));
    assert!(f
        .store
        .take_after_handoff(&f.key, &token, &authorization, expired_at)
        .is_none());
    assert_eq!(
        f.store.len(),
        1,
        "an invalid authorization cannot consume the saved session"
    );
}

#[test]
fn account_and_identity_revocation_fences_invalidate_the_old_revision_permanently() {
    for account_scope in [true, false] {
        let mut f = Fixture::new();
        let old = f.authorization();
        let token = f.ready();
        let before = stored_snapshot(&f);
        let fence = if account_scope {
            f.store.begin_account_identity_revocation(ACCOUNT)
        } else {
            f.store
                .begin_identity_session_revocation(&f.verified.session_id)
        };
        assert!(f
            .store
            .capture_handoff_authorization(&f.verified, gateway_unix_ms())
            .is_none());
        assert!(!f
            .store
            .handoff_authorization_current(&old, gateway_unix_ms()));
        assert!(f.take(&token, &old).is_none());
        drop(fence);
        assert!(!f
            .store
            .handoff_authorization_current(&old, gateway_unix_ms()));
        assert!(
            f.take(&token, &old).is_none(),
            "ending a mutation cannot revive an earlier revision"
        );
        assert_eq!(stored_snapshot(&f), before);
        // This fence performed no identity mutation. A freshly captured authorization is distinct.
        let fresh = f.authorization();
        assert!(fresh.revision != old.revision);
        assert!(f
            .store
            .handoff_authorization_current(&fresh, gateway_unix_ms()));
        assert!(f.take(&token, &fresh).is_some());
    }
}

#[test]
fn overlapping_revocations_do_not_reopen_capture_when_only_one_fence_ends() {
    let f = Fixture::new();
    let old = f.authorization();
    let first = f.store.begin_account_identity_revocation(ACCOUNT);
    let second = f.store.begin_account_identity_revocation(ACCOUNT);
    drop(first);
    assert!(f
        .store
        .capture_handoff_authorization(&f.verified, gateway_unix_ms())
        .is_none());
    drop(second);
    assert!(f
        .store
        .capture_handoff_authorization(&f.verified, gateway_unix_ms())
        .is_some());
    assert!(!f
        .store
        .handoff_authorization_current(&old, gateway_unix_ms()));
}

#[tokio::test]
async fn a_fresh_socket_binding_can_publish_a_second_handoff_without_reusing_the_first_token() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let first = f.ready();
    let restored = f.take(&first, &authorization).unwrap();
    let before = snapshot(&restored.session);
    let next_transport = TransportSignal::new();
    let binding = f.bind(next_transport.clone());
    assert!(!binding.token().same(&first));
    assert!(
        f.store.handoffs.waiter(&f.key, &f.owner).is_none(),
        "a live second socket is not waitable"
    );
    next_transport.end(TransportEnd::ReadError);
    let mut waiter = Box::pin(f.waiter().wait());
    pending(waiter.as_mut());
    f.publish_parts(
        restored.session,
        restored.active_session_permit,
        Some(binding),
    );
    let second = waiter.await.unwrap();
    assert!(f.take(&first, &authorization).is_none());
    let second_restore = f.take(&second, &authorization).unwrap();
    assert_eq!(snapshot(&second_restore.session), before);
    assert!(f
        .store
        .handoffs
        .bind(f.key.clone(), f.owner.clone(), TransportSignal::new())
        .is_some());
}

fn identity_current(
    f: &Fixture,
    authorization: &HandoffAuthorization,
    cache: &dyn GatewaySessionCache,
) -> bool {
    handoff_identity_still_current(
        Some(authorization),
        &f.store,
        true,
        Some(ACCOUNT),
        Some(&f.verified),
        cache,
        &f.identity,
        gateway_unix_ms(),
    )
}

#[test]
fn identity_helper_rejects_auth_account_and_current_identity_changes() {
    let f = Fixture::new();
    let authorization = f.authorization();
    let cache = InMemoryGatewaySessionCache::default();
    assert!(identity_current(&f, &authorization, &cache));
    for (authenticated, account, verified) in [
        (false, Some(ACCOUNT), Some(&f.verified)),
        (true, None, Some(&f.verified)),
        (true, Some("otherowner"), Some(&f.verified)),
        (true, Some(ACCOUNT), None),
    ] {
        assert!(!handoff_identity_still_current(
            Some(&authorization),
            &f.store,
            authenticated,
            account,
            verified,
            &cache,
            &f.identity,
            gateway_unix_ms(),
        ));
    }
    let next_identity = issue_identity(&f.identity, ACCOUNT);
    assert_ne!(next_identity.session_id, f.verified.session_id);
    assert!(!handoff_identity_still_current(
        Some(&authorization),
        &f.store,
        true,
        Some(ACCOUNT),
        Some(&next_identity),
        &cache,
        &f.identity,
        gateway_unix_ms(),
    ));
    assert!(
        identity_current(&f, &authorization, &cache),
        "the other grant did not revoke this identity"
    );
}

#[test]
fn identity_helper_rejects_real_identity_revocation_without_mutating_the_saved_world() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let _token = f.ready();
    let before = stored_snapshot(&f);
    let cache = InMemoryGatewaySessionCache::default();
    assert!(identity_current(&f, &authorization, &cache));
    assert!(f
        .identity
        .revoke_session(&f.verified, &f.verified.session_id, "handoff regression")
        .unwrap());
    assert!(f
        .store
        .handoff_authorization_current(&authorization, gateway_unix_ms()));
    assert!(
        !identity_current(&f, &authorization, &cache),
        "the live identity service is checked, not only the captured revision"
    );
    assert_eq!(f.store.len(), 1);
    assert_eq!(stored_snapshot(&f), before);
}

#[test]
fn identity_helper_observes_shared_cache_revocation_without_consuming_the_lease() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let _token = f.ready();
    let before = stored_snapshot(&f);
    let cache = Arc::new(InMemoryGatewaySessionCache::default());
    assert!(identity_current(&f, &authorization, cache.as_ref()));
    let remote_cache_handle = Arc::clone(&cache);
    remote_cache_handle
        .revoke_identity_session(&f.verified.session_id, 60)
        .unwrap();
    assert!(f
        .store
        .handoff_authorization_current(&authorization, gateway_unix_ms()));
    assert!(!identity_current(&f, &authorization, cache.as_ref()));
    assert_eq!(f.store.len(), 1);
    assert_eq!(stored_snapshot(&f), before);
}

#[test]
fn shared_authority_revoked_after_local_consumption_cannot_authorize_bootstrap_or_restore_again() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let token = f.ready();
    let restored = f.take(&token, &authorization).unwrap();
    let before = snapshot(&restored.session);
    let cache = InMemoryGatewaySessionCache::default();
    assert!(identity_current(&f, &authorization, &cache));
    cache
        .revoke_identity_session(&f.verified.session_id, 60)
        .unwrap();
    assert!(!identity_current(&f, &authorization, &cache));
    assert!(f.take(&token, &authorization).is_none());
    assert_eq!(snapshot(&restored.session), before);
}

#[tokio::test]
async fn capacity_wait_does_not_preserve_authorization_revoked_after_consumption() {
    let mut f = Fixture::new();
    let authorization = f.authorization();
    let token = f.ready();
    let restored = f.take(&token, &authorization).unwrap();
    let before = snapshot(&restored.session);
    let capacity = Arc::new(GatewayCapacityState::with_action_limits(None, None, Some(1)));
    let first = capacity
        .try_acquire_action(GatewayCapacityKind::StartGame)
        .unwrap();
    let mut waiting = Box::pin(
        capacity.acquire_action_with_wait(GatewayCapacityKind::StartGame, Duration::from_secs(30)),
    );
    pending(waiting.as_mut());
    let cache = InMemoryGatewaySessionCache::default();
    cache
        .revoke_identity_session(&f.verified.session_id, 60)
        .unwrap();
    drop(first);
    let _next = waiting.await.unwrap();
    assert!(!identity_current(&f, &authorization, &cache));
    assert!(f.take(&token, &authorization).is_none());
    assert_eq!(snapshot(&restored.session), before);
}

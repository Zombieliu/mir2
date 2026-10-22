//! Deterministic in-process handoff scheduling. These tests use real personal
//! Session save/retained/prepare/commit code, not PostgreSQL or a physical GUI.
use super::*;
use crate::cache::InMemoryGatewaySessionCache;
use std::sync::atomic::AtomicU64;
use tokio::sync::oneshot;

struct Fixture {
    store: ReconnectSessionStore,
    identity: IdentityService,
    verified: VerifiedIdentitySession,
    cache: InMemoryGatewaySessionCache,
    credential: ResumeCredential,
    binding: ResumeBinding,
    native: NativeResumeConnectionState,
    closing: Option<ReconnectSessionLease>,
}

impl Fixture {
    fn new() -> Self {
        let store = ReconnectSessionStore::default();
        let (identity, verified) = super::tests::issue_test_identity();
        let (credential, binding) = super::tests::store_native_resume_fixture(
            &store,
            &verified,
            None,
            None,
            Duration::from_secs(15),
        );
        // Remove only the test's already constructed retained carrier without
        // consuming its credential; it now models the original closing socket.
        let key = GatewaySessionCacheKey {
            account_id: binding.account_id.clone(),
            character_index: binding.character_index,
        };
        let closing = store.state.lock().unwrap().sessions.remove(&key).unwrap();
        let mut native = NativeResumeConnectionState::new();
        native.opted_in = true;
        native.family_id = Some(binding.family_id.clone());
        native.connection_nonce = binding.source_connection_nonce.clone();
        native.last_issued_binding = Some(binding.clone());
        native.last_issued_at_ms = Some(binding.issued_at_ms);
        Self {
            store,
            identity,
            verified,
            cache: InMemoryGatewaySessionCache::default(),
            credential,
            binding,
            native,
            closing: Some(closing),
        }
    }

    fn begin(&self) -> NativeResumeHandoffGuard<'_> {
        self.store
            .begin_native_resume_handoff(
                &self.native,
                &self.closing.as_ref().unwrap().session,
                gateway_unix_ms(),
            )
            .unwrap()
    }

    fn commit(&self) -> bool {
        validate_and_commit_native_resume_for_test(
            &self.store,
            &self.cache,
            &self.identity,
            &self.credential,
            gateway_unix_ms(),
        )
        .is_some()
    }
}

fn publish_saved(store: &ReconnectSessionStore, mut closing: ReconnectSessionLease) {
    assert!(matches!(
        closing.session.try_persist_teardown_once(),
        GatewayTeardownPersistenceOutcome::Saved
    ));
    closing.session.release_teardown_for_resume().unwrap();
    let key = session_cache_key(&closing.session).unwrap();
    store.store(
        key,
        closing.session,
        closing.active_session_permit,
        closing._reconnect_lease_permit,
        closing.resume_family_id,
        Duration::from_secs(15),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handoff_waits_for_saved_owner_then_existing_prepare_commit_consumes_once() {
    let mut fixture = Fixture::new();
    let closing = fixture.closing.take().unwrap();
    let guard = fixture
        .store
        .begin_native_resume_handoff(&fixture.native, &closing.session, gateway_unix_ms())
        .unwrap();
    let (started, observed) = oneshot::channel();
    let mut started = Some(started);
    let waiting = await_native_resume_handoff_with_budget(
        &fixture.store,
        &fixture.credential,
        Duration::from_secs(2),
        gateway_unix_ms,
        |binding, now| {
            if let Some(started) = started.take() {
                started.send(()).unwrap();
            }
            assert_eq!(fixture.store.len(), 0, "unsaved Session is not published");
            native_resume_identity_is_active(
                &fixture.cache,
                &fixture.identity,
                &fixture.verified,
                now,
            ) && binding == &fixture.binding
        },
    );
    let save = async {
        observed.await.unwrap();
        publish_saved(&fixture.store, closing);
        drop(guard);
    };
    let (ready, ()) = tokio::join!(waiting, save);
    ready.unwrap();
    assert!(fixture.commit());
    assert!(!fixture.commit());
    assert!(fixture
        .store
        .state
        .lock()
        .unwrap()
        .native_handoffs
        .is_empty());
}

#[tokio::test]
async fn online_credential_without_registered_handoff_is_immediately_unavailable() {
    let fixture = Fixture::new();
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        await_native_resume_handoff_with_budget(
            &fixture.store,
            &fixture.credential,
            NATIVE_RESUME_HANDOFF_MAX_WAIT,
            gateway_unix_ms,
            |_, _| panic!("an online credential does not authorize waiting"),
        ),
    )
    .await
    .unwrap();
    assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
    assert!(fixture
        .store
        .resume_binding(&fixture.credential, gateway_unix_ms())
        .is_some());
}

#[tokio::test]
async fn handoff_guard_drop_wakes_failed_save_and_removes_only_its_marker() {
    let fixture = Fixture::new();
    let guard = fixture.begin();
    let (started, observed) = oneshot::channel();
    let mut started = Some(started);
    let waiting = await_native_resume_handoff_with_budget(
        &fixture.store,
        &fixture.credential,
        Duration::from_secs(2),
        gateway_unix_ms,
        |_, _| {
            if let Some(started) = started.take() {
                started.send(()).unwrap();
            }
            true
        },
    );
    let failed = async {
        observed.await.unwrap();
        drop(guard);
    };
    let (result, ()) = tokio::join!(waiting, failed);
    assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
    assert_eq!(fixture.store.len(), 0);
    assert!(fixture
        .store
        .state
        .lock()
        .unwrap()
        .native_handoffs
        .is_empty());
    // A failed release is not a consumed credential and cannot retry waiting
    // after its private closing-socket marker has gone away.
    assert!(fixture
        .store
        .resume_binding(&fixture.credential, gateway_unix_ms())
        .is_some());
}

#[tokio::test]
async fn journaled_handoff_wakes_waiter_revokes_family_and_never_publishes() {
    let mut fixture = Fixture::new();
    let closing = fixture.closing.take().unwrap();
    let guard = fixture
        .store
        .begin_native_resume_handoff(&fixture.native, &closing.session, gateway_unix_ms())
        .unwrap();
    let (started, observed) = oneshot::channel();
    let mut started = Some(started);
    let waiting = await_native_resume_handoff_with_budget(
        &fixture.store,
        &fixture.credential,
        Duration::from_secs(2),
        gateway_unix_ms,
        |_, _| {
            if let Some(started) = started.take() {
                started.send(()).unwrap();
            }
            true
        },
    );
    let journaled = async {
        observed.await.unwrap();
        assert!(!apply_teardown_persistence_to_resume(
            WebTeardownPersistenceOutcome::Journaled,
            &mut fixture.native,
            &fixture.store,
            Some(&fixture.binding.account_id)
        ));
        drop(guard);
        drop(closing);
    };
    let (result, ()) = tokio::join!(waiting, journaled);
    assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
    assert!(fixture
        .store
        .resume_binding(&fixture.credential, gateway_unix_ms())
        .is_none());
    assert!(fixture
        .store
        .state
        .lock()
        .unwrap()
        .native_handoffs
        .is_empty());
}

#[tokio::test]
async fn failed_persistence_stops_waiting_while_original_save_guard_remains_alive() {
    let fixture = Fixture::new();
    let guard = fixture.begin();
    let (started, observed) = oneshot::channel();
    let mut started = Some(started);
    let waiting = await_native_resume_handoff_with_budget(
        &fixture.store,
        &fixture.credential,
        Duration::from_secs(2),
        gateway_unix_ms,
        |_, _| {
            if let Some(started) = started.take() {
                started.send(()).unwrap();
            }
            true
        },
    );
    let failure = async {
        observed.await.unwrap();
        guard.stop_waiting();
    };
    let (result, ()) = tokio::join!(waiting, failure);
    assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
    assert_eq!(fixture.store.len(), 0);
    assert_eq!(fixture.store.state.lock().unwrap().native_handoffs.len(), 1);
    drop(guard);
    assert!(fixture
        .store
        .state
        .lock()
        .unwrap()
        .native_handoffs
        .is_empty());
}

#[tokio::test]
async fn local_account_and_identity_revocation_wake_saving_handoff() {
    for account in [true, false] {
        let fixture = Fixture::new();
        let guard = fixture.begin();
        let (started, observed) = oneshot::channel();
        let mut started = Some(started);
        let waiting = await_native_resume_handoff_with_budget(
            &fixture.store,
            &fixture.credential,
            Duration::from_secs(2),
            gateway_unix_ms,
            |_, _| {
                if let Some(started) = started.take() {
                    started.send(()).unwrap();
                }
                true
            },
        );
        let revoke = async {
            observed.await.unwrap();
            if account {
                drop(
                    fixture
                        .store
                        .begin_account_identity_revocation(&fixture.binding.account_id),
                );
            } else {
                drop(
                    fixture
                        .store
                        .begin_identity_session_revocation(&fixture.binding.identity_session_id),
                );
            }
        };
        let (result, ()) = tokio::join!(waiting, revoke);
        assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
        assert!(!fixture.commit());
        drop(guard);
    }
}

#[tokio::test]
async fn shared_identity_revocation_is_rechecked_while_saving() {
    let fixture = Fixture::new();
    let guard = fixture.begin();
    let (started, observed) = oneshot::channel();
    let mut started = Some(started);
    let waiting = await_native_resume_handoff_with_budget(
        &fixture.store,
        &fixture.credential,
        Duration::from_secs(2),
        gateway_unix_ms,
        |_, now| {
            if let Some(started) = started.take() {
                started.send(()).unwrap();
            }
            native_resume_identity_is_active(
                &fixture.cache,
                &fixture.identity,
                &fixture.verified,
                now,
            )
        },
    );
    let revoke = async {
        observed.await.unwrap();
        fixture
            .identity
            .revoke_session(
                &fixture.verified,
                &fixture.verified.session_id,
                "handoff test",
            )
            .unwrap();
    };
    let (result, ()) = tokio::join!(waiting, revoke);
    assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
    assert!(!fixture.commit());
    drop(guard);
}

#[tokio::test]
async fn credential_expiry_during_handoff_is_terminal_without_ttl_extension() {
    let fixture = Fixture::new();
    let guard = fixture.begin();
    let clock = AtomicU64::new(fixture.binding.issued_at_ms);
    let (started, observed) = oneshot::channel();
    let mut started = Some(started);
    let waiting = await_native_resume_handoff_with_budget(
        &fixture.store,
        &fixture.credential,
        Duration::from_secs(2),
        || clock.load(Ordering::Acquire),
        |_, _| {
            if let Some(started) = started.take() {
                started.send(()).unwrap();
            }
            true
        },
    );
    let expire = async {
        observed.await.unwrap();
        clock.store(fixture.binding.expires_at_ms, Ordering::Release);
        // A private scheduling notification changes no credential or lease.
        guard
            .handoff
            .changed
            .send_replace(NativeResumeHandoffProgress::Saving);
    };
    let (result, ()) = tokio::join!(waiting, expire);
    assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
    assert!(fixture
        .store
        .resume_binding(&fixture.credential, fixture.binding.expires_at_ms)
        .is_none());
    assert_eq!(fixture.store.len(), 0);
    drop(guard);
}

#[tokio::test]
async fn handoff_wait_has_one_fixed_bounded_budget_and_no_published_owner() {
    let fixture = Fixture::new();
    let guard = fixture.begin();
    assert!(NATIVE_RESUME_HANDOFF_MAX_WAIT <= Duration::from_secs(10));
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        await_native_resume_handoff_with_budget(
            &fixture.store,
            &fixture.credential,
            Duration::from_millis(20),
            gateway_unix_ms,
            |_, _| true,
        ),
    )
    .await
    .unwrap();
    assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
    assert_eq!(fixture.store.len(), 0);
    assert_eq!(
        fixture
            .store
            .resume_binding(&fixture.credential, gateway_unix_ms())
            .unwrap(),
        fixture.binding
    );
    drop(guard);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_handoff_waiters_still_have_exactly_one_existing_commit_winner() {
    let mut fixture = Fixture::new();
    let closing = fixture.closing.take().unwrap();
    let guard = fixture
        .store
        .begin_native_resume_handoff(&fixture.native, &closing.session, gateway_unix_ms())
        .unwrap();
    let (started_a, observed_a) = oneshot::channel();
    let (started_b, observed_b) = oneshot::channel();
    let contender = |started: oneshot::Sender<()>| {
        let fixture = &fixture;
        async move {
            let mut started = Some(started);
            let ready = await_native_resume_handoff_with_budget(
                &fixture.store,
                &fixture.credential,
                Duration::from_secs(2),
                gateway_unix_ms,
                |_, _| {
                    if let Some(started) = started.take() {
                        started.send(()).unwrap();
                    }
                    true
                },
            )
            .await;
            ready.is_ok() && fixture.commit()
        }
    };
    let save = async {
        observed_a.await.unwrap();
        observed_b.await.unwrap();
        publish_saved(&fixture.store, closing);
        drop(guard);
    };
    let (a, b, ()) = tokio::join!(contender(started_a), contender(started_b), save);
    assert_eq!(usize::from(a) + usize::from(b), 1);
    assert_eq!(fixture.store.len(), 0);
    assert!(fixture
        .store
        .resume_binding(&fixture.credential, gateway_unix_ms())
        .is_none());
}

#[test]
fn handoff_registration_requires_exact_private_binding_and_cleans_up_on_unwind() {
    let mut fixture = Fixture::new();
    fixture.native.connection_nonce = ResumeConnectionNonce::generate();
    assert!(fixture
        .store
        .begin_native_resume_handoff(
            &fixture.native,
            &fixture.closing.as_ref().unwrap().session,
            gateway_unix_ms()
        )
        .is_none());
    fixture.native.connection_nonce = fixture.binding.source_connection_nonce.clone();
    let original = fixture.native.last_issued_binding.clone().unwrap();
    for field in 0..4 {
        let mut forged = original.clone();
        match field {
            0 => forged.gateway_session_id.push_str("-other"),
            1 => forged.identity_session_id.push_str("-other"),
            2 => forged.auth_revision.account = forged.auth_revision.account.saturating_add(1),
            _ => forged.character_index = forged.character_index.saturating_add(1),
        }
        fixture.native.last_issued_binding = Some(forged);
        assert!(fixture
            .store
            .begin_native_resume_handoff(
                &fixture.native,
                &fixture.closing.as_ref().unwrap().session,
                gateway_unix_ms()
            )
            .is_none());
    }
    fixture.native.last_issued_binding = Some(original);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = fixture.begin();
        assert!(
            fixture
                .store
                .begin_native_resume_handoff(
                    &fixture.native,
                    &fixture.closing.as_ref().unwrap().session,
                    gateway_unix_ms()
                )
                .is_none(),
            "second marker cannot replace the first"
        );
        panic!("test closing-socket unwind");
    }));
    assert!(panic.is_err());
    assert!(fixture
        .store
        .state
        .lock()
        .unwrap()
        .native_handoffs
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn older_live_rotation_generation_waits_for_exact_family_and_commit_revokes_siblings() {
    let mut fixture = Fixture::new();
    let closing = fixture.closing.take().unwrap();
    let newer = fixture
        .store
        .issue_resume_credential(
            Some(&fixture.binding.family_id),
            ResumeIssueContext {
                account_id: &fixture.binding.account_id,
                character_index: fixture.binding.character_index,
                gateway_session_id: &fixture.binding.gateway_session_id,
                identity_session_id: &fixture.binding.identity_session_id,
                identity_expires_at_ms: fixture.binding.identity_expires_at_ms,
                source_connection_nonce: &fixture.binding.source_connection_nonce,
            },
            gateway_unix_ms(),
            fixture.binding.generation + 1,
            || true,
        )
        .unwrap();
    assert_eq!(newer.binding.family_id, fixture.binding.family_id);
    fixture.native.last_issued_binding = Some(newer.binding.clone());
    let guard = fixture
        .store
        .begin_native_resume_handoff(&fixture.native, &closing.session, gateway_unix_ms())
        .unwrap();
    let (started, observed) = oneshot::channel();
    let mut started = Some(started);
    let waiting = await_native_resume_handoff_with_budget(
        &fixture.store,
        &fixture.credential,
        Duration::from_secs(2),
        gateway_unix_ms,
        |binding, _| {
            assert_eq!(
                binding, &fixture.binding,
                "the request keeps its original TTL and generation"
            );
            if let Some(started) = started.take() {
                started.send(()).unwrap();
            }
            true
        },
    );
    let save = async {
        observed.await.unwrap();
        publish_saved(&fixture.store, closing);
        drop(guard);
    };
    let (result, ()) = tokio::join!(waiting, save);
    result.unwrap();
    assert!(fixture.commit());
    assert!(fixture
        .store
        .resume_binding(&newer.credential, gateway_unix_ms())
        .is_none());
}

#[tokio::test]
async fn other_valid_family_cannot_wait_on_same_account_character_and_session() {
    let fixture = Fixture::new();
    let guard = fixture.begin();
    let other = fixture
        .store
        .issue_resume_credential(
            None,
            ResumeIssueContext {
                account_id: &fixture.binding.account_id,
                character_index: fixture.binding.character_index,
                gateway_session_id: &fixture.binding.gateway_session_id,
                identity_session_id: &fixture.binding.identity_session_id,
                identity_expires_at_ms: fixture.binding.identity_expires_at_ms,
                source_connection_nonce: &fixture.binding.source_connection_nonce,
            },
            gateway_unix_ms(),
            1,
            || true,
        )
        .unwrap();
    assert_ne!(other.binding.family_id, fixture.binding.family_id);
    assert_eq!(
        await_native_resume_handoff_with_budget(
            &fixture.store,
            &other.credential,
            NATIVE_RESUME_HANDOFF_MAX_WAIT,
            gateway_unix_ms,
            |_, _| panic!("a different family has no handoff authority"),
        )
        .await,
        Err(NativeResumePrepareError::Unavailable)
    );
    drop(guard);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aborted_waiter_does_not_become_success_when_a_later_retry_saves() {
    let mut fixture = Fixture::new();
    let closing = fixture.closing.take().unwrap();
    let guard = fixture
        .store
        .begin_native_resume_handoff(&fixture.native, &closing.session, gateway_unix_ms())
        .unwrap();
    let (started, observed) = oneshot::channel();
    let mut started = Some(started);
    let waiting = await_native_resume_handoff_with_budget(
        &fixture.store,
        &fixture.credential,
        Duration::from_secs(2),
        gateway_unix_ms,
        |_, _| {
            if let Some(started) = started.take() {
                started.send(()).unwrap();
            }
            true
        },
    );
    let retry = async {
        observed.await.unwrap();
        guard.stop_waiting();
        publish_saved(&fixture.store, closing);
        drop(guard);
    };
    let (result, ()) = tokio::join!(waiting, retry);
    assert_eq!(result, Err(NativeResumePrepareError::Unavailable));
    assert_eq!(
        fixture.store.len(),
        1,
        "a later actual save remains governed by ordinary retained-lease rules"
    );
    assert!(fixture
        .store
        .state
        .lock()
        .unwrap()
        .native_handoffs
        .is_empty());
}

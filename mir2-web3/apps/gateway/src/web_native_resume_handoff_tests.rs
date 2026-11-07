//! Store-level tests of the actual credential-first handoff/prepare/commit path.
//! No listener, external persistence, admin command or live account is used.
use super::native_resume_handoff::PendingNativeResumeHandoff;
use super::transport::{TransportEnd, TransportSignal};
use super::*;
use crate::cache::{GatewaySessionCache, InMemoryGatewaySessionCache};
use mir2_protocol::{MirClass, MirGender};
use mir2_simulation::SimulationConfig;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    store: Arc<ReconnectSessionStore>,
    cache: InMemoryGatewaySessionCache,
    capacity: Arc<GatewayCapacityState>,
    identity: IdentityService,
    verified: VerifiedIdentitySession,
    key: GatewaySessionCacheKey,
    terminal: TransportSignal,
    guard: Option<HandoffBinding>,
    session: Option<GatewaySession>,
    credential: ResumeCredential,
    issued: ResumeBinding,
}

impl Fixture {
    fn new() -> Self {
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let account = format!("nrhsave{sequence}");
        let mut session = GatewaySession::new(SimulationConfig::default());
        let registration = session.handle_packet(ClientPacket::NewAccount {
            account_id: account.clone(),
            password: "NativeHandoff42".into(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        });
        assert!(registration
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })));
        let login = session.handle_packet(ClientPacket::Login {
            account_id: account.clone(),
            password: "NativeHandoff42".into(),
        });
        assert!(login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let created = session.handle_packet(ClientPacket::NewCharacter {
            name: format!("NhWarrior{sequence}"),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        });
        let index = created
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            })
            .expect("ordinary character creation returns its protocol identity");
        let start = session.handle_packet(ClientPacket::StartGame {
            character_index: index,
        });
        assert!(start
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
        let key = session_cache_key(&session).unwrap();
        assert_eq!(key.account_id, account);
        assert_eq!(key.character_index, index);
        let identity = IdentityService::local_for_tests();
        let grant = identity
            .issue_session(
                &account,
                "password",
                &account,
                "127.0.0.1",
                "native-handoff-unit",
            )
            .unwrap();
        let verified = identity.verify_session_token(&grant.token).unwrap();
        let cache = InMemoryGatewaySessionCache::default();
        refresh_session_cache_with_route_lease(&cache, &session, 60).unwrap();
        let store = Arc::new(ReconnectSessionStore::default());
        let terminal = TransportSignal::new();
        let guard = store
            .handoffs
            .bind(
                key.clone(),
                session.session_id().to_owned(),
                terminal.clone(),
            )
            .unwrap();
        let nonce = ResumeConnectionNonce::generate();
        let issued = store
            .issue_resume_credential(
                None,
                ResumeIssueContext {
                    account_id: &account,
                    character_index: index,
                    gateway_session_id: session.session_id(),
                    identity_session_id: &verified.session_id,
                    identity_expires_at_ms: verified.expires_at_ms,
                    source_connection_nonce: &nonce,
                },
                gateway_unix_ms(),
                1,
                || {
                    native_resume_identity_is_active(
                        &cache,
                        &identity,
                        &verified,
                        gateway_unix_ms(),
                    )
                },
            )
            .unwrap();
        Self {
            store,
            cache,
            capacity: Arc::new(GatewayCapacityState::with_limits(None, Some(4), Some(4))),
            identity,
            verified,
            key,
            terminal,
            guard: Some(guard),
            session: Some(session),
            credential: issued.credential,
            issued: issued.binding,
        }
    }

    fn close(&self) {
        self.terminal.end(TransportEnd::Closed);
    }

    fn pending(&self) -> PendingNativeResumeHandoff {
        prepare_native_resume_handoff(
            &self.store,
            &self.cache,
            &self.identity,
            &self.credential,
            gateway_unix_ms(),
        )
        .expect("valid credential and identity")
        .expect("only the actually ended, server-bound socket supplies a waiter")
    }

    /// Exercise the production publication seam. Durable save barriers have separate tests.
    fn publish(&mut self) {
        let session = self.session.take().unwrap();
        let guard = self.guard.take().unwrap();
        self.store.store_with_handoff(
            self.key.clone(),
            session,
            Some(self.capacity.try_acquire_active_session().unwrap()),
            self.capacity.try_acquire_reconnect_lease().unwrap(),
            Some(self.issued.family_id.clone()),
            Duration::from_secs(60),
            Some(guard),
        );
    }

    fn prepared<'a>(
        &'a self,
        completed: &CompletedNativeResumeHandoff,
    ) -> Result<
        (ReconnectSessionReservation<'a>, VerifiedIdentitySession, ()),
        NativeResumePrepareError,
    > {
        validate_and_prepare_native_resume_with_handoff(
            &self.store,
            &self.cache,
            &self.identity,
            &self.credential,
            gateway_unix_ms(),
            Some(completed),
            |session| refresh_session_cache_with_route_lease(&self.cache, session, 60).map(|_| ()),
            |session| {
                let active = session.active_identity().unwrap();
                assert_eq!(active.account_id, self.key.account_id);
                assert_eq!(active.character_index, self.key.character_index);
                Ok(())
            },
        )
    }

    fn snapshot(&self) -> Value {
        if let Some(session) = self.session.as_ref() {
            serde_json::to_value(session.world_snapshot()).unwrap()
        } else {
            let state = self.store.state.lock().unwrap();
            serde_json::to_value(
                state
                    .sessions
                    .get(&self.key)
                    .unwrap()
                    .session
                    .world_snapshot(),
            )
            .unwrap()
        }
    }

    fn assert_unconsumed(&self) {
        assert!(self
            .store
            .resume_binding(&self.credential, gateway_unix_ms())
            .is_some());
    }
}

fn assert_pending<F: Future>(future: Pin<&mut F>) {
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(future.poll(&mut context), Poll::Pending));
}

#[test]
fn live_socket_and_unknown_credential_do_not_acquire_waiters() {
    let f = Fixture::new();
    assert!(prepare_native_resume_handoff(
        &f.store,
        &f.cache,
        &f.identity,
        &f.credential,
        gateway_unix_ms()
    )
    .unwrap()
    .is_none());
    assert_eq!(f.store.handoffs.waiter_count_for_test(&f.key), Some(0));
    let unknown: ResumeCredential =
        serde_json::from_value(json!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")).unwrap();
    f.close();
    assert!(matches!(
        prepare_native_resume_handoff(&f.store, &f.cache, &f.identity, &unknown, gateway_unix_ms()),
        Err(NativeResumePrepareError::Unavailable)
    ));
    assert_eq!(f.store.handoffs.waiter_count_for_test(&f.key), Some(0));
    f.assert_unconsumed();
    assert_eq!(f.store.len(), 0);
}

#[tokio::test]
async fn pending_save_retains_credential_and_early_ready_allows_one_real_commit() {
    let mut f = Fixture::new();
    let before = f.snapshot();
    f.close();
    let pending = f.pending();
    assert_eq!(f.store.handoffs.waiter_count_for_test(&f.key), Some(1));
    assert_eq!(f.store.len(), 0);
    f.assert_unconsumed();
    // Ready arrives before this waiter first polls, rather than relying on a sleep.
    f.publish();
    let completed = pending.wait().await.unwrap();
    let (reservation, verified, ()) = f.prepared(&completed).unwrap();
    assert_eq!(f.store.len(), 0);
    f.assert_unconsumed();
    let (mut restored, binding, _, ()) = revalidate_and_commit_prepared_native_resume(
        &f.store,
        &f.cache,
        &f.identity,
        reservation,
        &f.credential,
        verified,
        (),
        gateway_unix_ms(),
    )
    .unwrap();
    assert_eq!(binding, f.issued);
    assert_eq!(
        serde_json::to_value(restored.session.world_snapshot()).unwrap(),
        before
    );
    let bootstrap = restored
        .session
        .replay_retained_start_game_bootstrap(&f.key.account_id, f.key.character_index, true)
        .unwrap();
    assert!(bootstrap
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert!(f
        .store
        .resume_binding(&f.credential, gateway_unix_ms())
        .is_none());
    assert_eq!(f.store.len(), 0);
    assert_eq!(f.capacity.status().current_reconnect_leases, 0);
    assert_eq!(f.capacity.status().current_active_sessions, 1);
}

#[tokio::test]
async fn revocation_during_wait_never_becomes_authority_when_ready_arrives() {
    for variant in 0..3 {
        let mut f = Fixture::new();
        f.close();
        let mut waiting = Box::pin(f.pending().wait());
        assert_pending(waiting.as_mut());
        match variant {
            0 => {
                f.identity
                    .revoke_session(
                        &f.verified,
                        &f.verified.session_id,
                        "native handoff fixture",
                    )
                    .unwrap();
            }
            1 => {
                drop(
                    f.store
                        .begin_identity_session_revocation(&f.verified.session_id),
                );
            }
            _ => {
                f.cache
                    .revoke_identity_session(&f.verified.session_id, 60)
                    .unwrap();
            }
        }
        f.publish();
        let before = f.snapshot();
        let completed = waiting.await.unwrap();
        assert!(matches!(
            f.prepared(&completed),
            Err(NativeResumePrepareError::Unavailable)
        ));
        assert_eq!(f.store.len(), 1);
        assert_eq!(f.snapshot(), before);
        f.assert_unconsumed();
    }
}

#[tokio::test]
async fn two_ready_waiters_cannot_commit_the_same_family_twice() {
    let mut f = Fixture::new();
    f.close();
    let first = f.pending();
    let second = f.pending();
    assert_eq!(f.store.handoffs.waiter_count_for_test(&f.key), Some(2));
    f.publish();
    let first = first.wait().await.unwrap();
    let second = second.wait().await.unwrap();
    let (reservation, verified, ()) = f.prepared(&first).unwrap();
    assert!(matches!(
        f.prepared(&second),
        Err(NativeResumePrepareError::Unavailable)
    ));
    let committed = revalidate_and_commit_prepared_native_resume(
        &f.store,
        &f.cache,
        &f.identity,
        reservation,
        &f.credential,
        verified,
        (),
        gateway_unix_ms(),
    )
    .unwrap();
    assert!(matches!(
        f.prepared(&second),
        Err(NativeResumePrepareError::Unavailable)
    ));
    assert!(f
        .store
        .resume_binding(&f.credential, gateway_unix_ms())
        .is_none());
    assert_eq!(f.store.len(), 0);
    drop(committed);
    assert_eq!(f.capacity.status().current_active_sessions, 0);
    assert_eq!(f.capacity.status().current_reconnect_leases, 0);
}

#[tokio::test]
async fn same_owner_successor_requires_its_own_handoff_token() {
    let mut f = Fixture::new();
    f.close();
    let old = f.pending();
    f.publish();
    let old = old.wait().await.unwrap();
    let successor_guard = f
        .store
        .handoffs
        .bind(
            f.key.clone(),
            f.issued.gateway_session_id.clone(),
            f.terminal.clone(),
        )
        .unwrap();
    let successor = f.pending();
    // Preserve the exact session, family and owner to isolate the binding-token ABA fence.
    let lease = f
        .store
        .state
        .lock()
        .unwrap()
        .sessions
        .remove(&f.key)
        .unwrap();
    f.store.store_with_handoff(
        f.key.clone(),
        lease.session,
        lease.active_session_permit,
        lease._reconnect_lease_permit,
        lease.resume_family_id,
        Duration::from_secs(60),
        Some(successor_guard),
    );
    let successor = successor.wait().await.unwrap();
    let before = f.snapshot();
    assert!(matches!(
        f.prepared(&old),
        Err(NativeResumePrepareError::Unavailable)
    ));
    assert_eq!(f.store.len(), 1);
    assert_eq!(f.snapshot(), before);
    f.assert_unconsumed();
    let prepared = f.prepared(&successor).unwrap();
    drop(prepared);
    assert_eq!(f.store.len(), 1);
    f.assert_unconsumed();
}

#[tokio::test]
async fn route_and_zone_prepare_failures_restore_the_exact_lease_and_credential() {
    for fail_route in [true, false] {
        let mut f = Fixture::new();
        f.close();
        let pending = f.pending();
        f.publish();
        let completed = pending.wait().await.unwrap();
        let before = f.snapshot();
        let route_calls = std::cell::Cell::new(0);
        let zone_calls = std::cell::Cell::new(0);
        let result = validate_and_prepare_native_resume_with_handoff(
            &f.store,
            &f.cache,
            &f.identity,
            &f.credential,
            gateway_unix_ms(),
            Some(&completed),
            |_| {
                route_calls.set(route_calls.get() + 1);
                if fail_route {
                    Err("fixture route failure".into())
                } else {
                    Ok(())
                }
            },
            |_| {
                zone_calls.set(zone_calls.get() + 1);
                Err::<(), _>("fixture zone failure".into())
            },
        );
        assert!(
            matches!(result, Err(NativeResumePrepareError::Route(_)) if fail_route)
                || matches!(result, Err(NativeResumePrepareError::Zone(_)) if !fail_route)
        );
        assert_eq!(route_calls.get(), 1);
        assert_eq!(zone_calls.get(), usize::from(!fail_route));
        assert_eq!(f.store.len(), 1);
        assert_eq!(f.snapshot(), before);
        assert_eq!(f.capacity.status().current_active_sessions, 1);
        assert_eq!(f.capacity.status().current_reconnect_leases, 1);
        f.assert_unconsumed();
        let prepared = f.prepared(&completed).unwrap();
        drop(prepared);
        assert_eq!(f.store.len(), 1);
        f.assert_unconsumed();
    }
}

#[tokio::test]
async fn unfinished_guard_failure_does_not_consume_or_publish_the_credential() {
    let mut f = Fixture::new();
    f.close();
    let pending = f.pending();
    drop(f.guard.take());
    assert!(matches!(
        pending.wait().await,
        Err(NativeResumePrepareError::Unavailable)
    ));
    assert_eq!(f.store.len(), 0);
    f.assert_unconsumed();
    assert!(f.session.as_ref().unwrap().active_identity().is_some());
}

#[tokio::test]
async fn canceled_waiter_does_not_consume_a_later_ready_lease() {
    let mut f = Fixture::new();
    f.close();
    let mut waiting = Box::pin(f.pending().wait());
    assert_pending(waiting.as_mut());
    assert_eq!(f.store.handoffs.waiter_count_for_test(&f.key), Some(1));
    drop(waiting);
    assert_eq!(f.store.handoffs.waiter_count_for_test(&f.key), Some(0));
    f.assert_unconsumed();
    f.publish();
    let prepared = validate_and_prepare_native_resume(
        &f.store,
        &f.cache,
        &f.identity,
        &f.credential,
        gateway_unix_ms(),
        |_| Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    drop(prepared);
    assert_eq!(f.store.len(), 1);
    f.assert_unconsumed();
}

#[tokio::test]
async fn credential_expiry_after_wait_cannot_reserve_a_ready_lease() {
    let mut f = Fixture::new();
    f.close();
    let pending = f.pending();
    f.publish();
    let completed = pending.wait().await.unwrap();
    let before = f.snapshot();
    let result = validate_and_prepare_native_resume_with_handoff(
        &f.store,
        &f.cache,
        &f.identity,
        &f.credential,
        f.issued.expires_at_ms,
        Some(&completed),
        |_| Ok(()),
        |_| Ok(()),
    );
    assert!(matches!(result, Err(NativeResumePrepareError::Unavailable)));
    assert_eq!(f.store.len(), 1);
    assert_eq!(f.snapshot(), before);
    assert!(f
        .store
        .resume_binding(&f.credential, f.issued.expires_at_ms)
        .is_none());
}

#[tokio::test]
async fn changed_nonce_generation_or_family_cannot_match_the_completed_binding() {
    let mut f = Fixture::new();
    f.close();
    let pending = f.pending();
    f.publish();
    let completed = pending.wait().await.unwrap();
    let other = Fixture::new();
    for variant in 0..3 {
        let mut changed = f.issued.clone();
        match variant {
            0 => changed.source_connection_nonce = ResumeConnectionNonce::generate(),
            1 => changed.generation += 1,
            _ => changed.family_id = other.issued.family_id.clone(),
        }
        assert!(f
            .store
            .reserve_by_credential_after_handoff(
                &f.credential,
                &changed,
                gateway_unix_ms(),
                Some(&completed)
            )
            .is_none());
        assert_eq!(f.store.len(), 1);
        f.assert_unconsumed();
    }
    let prepared = f.prepared(&completed).unwrap();
    drop(prepared);
    assert_eq!(f.store.len(), 1);
    f.assert_unconsumed();
}

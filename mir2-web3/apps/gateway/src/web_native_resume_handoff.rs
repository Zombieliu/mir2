//! Pending-save notification for credential-first resume. A notification is never authority.
use super::reconnect_handoff::HandoffWaiter;
use super::*;

pub(super) struct PendingNativeResumeHandoff {
    binding: ResumeBinding,
    waiter: HandoffWaiter,
}

pub(super) struct CompletedNativeResumeHandoff {
    binding: ResumeBinding,
    token: HandoffToken,
}

impl PendingNativeResumeHandoff {
    pub(super) async fn wait(
        self,
    ) -> Result<CompletedNativeResumeHandoff, NativeResumePrepareError> {
        let remaining_ms = self
            .binding
            .expires_at_ms
            .min(self.binding.identity_expires_at_ms)
            .saturating_sub(gateway_unix_ms())
            .min(30_000);
        if remaining_ms == 0 {
            return Err(NativeResumePrepareError::Unavailable);
        }
        let token = self.waiter.token();
        let completed =
            tokio::time::timeout(Duration::from_millis(remaining_ms), self.waiter.wait()).await;
        match completed {
            Ok(Ok(ready)) if ready.same(&token) => Ok(CompletedNativeResumeHandoff {
                binding: self.binding,
                token,
            }),
            _ => Err(NativeResumePrepareError::Unavailable),
        }
    }
}

impl CompletedNativeResumeHandoff {
    /// Called under the store's reservation mutex, before the lease can move.
    pub(super) fn matches_lease(
        &self,
        binding: &ResumeBinding,
        lease: &ReconnectSessionLease,
    ) -> bool {
        &self.binding == binding
            && lease
                .handoff_token
                .as_ref()
                .is_some_and(|token| token.same(&self.token))
    }
}

pub(super) fn prepare_native_resume_handoff(
    store: &ReconnectSessionStore,
    cache: &dyn crate::cache::GatewaySessionCache,
    identity: &IdentityService,
    credential: &ResumeCredential,
    now_ms: u64,
) -> Result<Option<PendingNativeResumeHandoff>, NativeResumePrepareError> {
    let pending = {
        let mut state = store
            .state
            .lock()
            .map_err(|_| NativeResumePrepareError::Unavailable)?;
        ReconnectSessionStore::purge_expired_locked(&mut state);
        let binding = state
            .credentials
            .binding_for_credential(credential.as_str(), now_ms)
            .ok_or(NativeResumePrepareError::Unavailable)?;
        // Compare with the revision captured at issuance. Capturing a fresh
        // revision here would incorrectly rehabilitate an old credential.
        if binding.identity_expires_at_ms <= now_ms
            || binding.auth_revision
                != ReconnectSessionStore::auth_revision_locked(
                    &state,
                    &binding.account_id,
                    &binding.identity_session_id,
                )
            || ReconnectSessionStore::auth_revocation_in_progress_locked(
                &state,
                &binding.account_id,
                &binding.identity_session_id,
            )
            || ReconnectSessionStore::auth_revision_blocked_locked(
                &state,
                &binding.account_id,
                &binding.identity_session_id,
            )
        {
            return Err(NativeResumePrepareError::Unavailable);
        }
        let key = GatewaySessionCacheKey {
            account_id: binding.account_id.clone(),
            character_index: binding.character_index,
        };
        // Only a server-bound, actually ended WebSocket can supply a waiter.
        // The store -> registry lock order matches publication of a saved lease.
        store
            .handoffs
            .waiter(&key, &binding.gateway_session_id)
            .map(|waiter| PendingNativeResumeHandoff { binding, waiter })
    };
    if let Some(pending) = pending.as_ref() {
        let binding = &pending.binding;
        let key = GatewaySessionCacheKey {
            account_id: binding.account_id.clone(),
            character_index: binding.character_index,
        };
        let verified = VerifiedIdentitySession {
            account_id: binding.account_id.clone(),
            session_id: binding.identity_session_id.clone(),
            expires_at_ms: binding.identity_expires_at_ms,
        };
        if !cache.get(&key).is_some_and(|record| {
            record.route_lease_owner.as_deref() == Some(binding.gateway_session_id.as_str())
        }) || !native_resume_identity_is_active(cache, identity, &verified, now_ms)
        {
            return Err(NativeResumePrepareError::Unavailable);
        }
    }
    Ok(pending)
}

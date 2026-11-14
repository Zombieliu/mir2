//! Process-local WebSocket handoff notification; never identity or route authority.
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use super::transport::TransportSignal;
use crate::cache::GatewaySessionCacheKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandoffState {
    Pending,
    Ready,
    Failed,
}

struct TokenIdentity {
    owner: String,
}

/// The host compares this exact binding with the token in its saved session lease.
#[derive(Clone)]
pub(super) struct HandoffToken(Arc<TokenIdentity>);

impl HandoffToken {
    pub(super) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(super) fn owner(&self) -> &str {
        &self.0.owner
    }
}

impl fmt::Debug for HandoffToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandoffToken")
            .finish_non_exhaustive()
    }
}

struct Entry {
    token: HandoffToken,
    terminal: TransportSignal,
    state: watch::Sender<HandoffState>,
}

#[derive(Default)]
struct Entries(BTreeMap<GatewaySessionCacheKey, Entry>);

impl fmt::Debug for Entries {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Debug output must not expose account keys or route-owner identities.
        formatter
            .debug_struct("Entries")
            .field("count", &self.0.len())
            .finish()
    }
}

#[derive(Debug, Default)]
pub(super) struct HandoffRegistry {
    entries: Mutex<Entries>,
}

impl HandoffRegistry {
    /// The host publishes only a server-owned Web binding. An existing binding is never replaced.
    pub(super) fn bind(
        self: &Arc<Self>,
        key: GatewaySessionCacheKey,
        owner: String,
        terminal: TransportSignal,
    ) -> Option<HandoffBinding> {
        let mut entries = self.entries.lock().ok()?;
        if entries.0.contains_key(&key) {
            return None;
        }
        let token = HandoffToken(Arc::new(TokenIdentity { owner }));
        let (state, _) = watch::channel(HandoffState::Pending);
        entries.0.insert(
            key.clone(),
            Entry {
                token: token.clone(),
                terminal,
                state: state.clone(),
            },
        );
        Some(HandoffBinding {
            registry: Arc::clone(self),
            key,
            token,
            state,
            finished: false,
        })
    }

    /// A live or foreign owner is not a handoff. This does not inspect or relax any lease.
    pub(super) fn waiter(
        &self,
        key: &GatewaySessionCacheKey,
        expected_owner: &str,
    ) -> Option<HandoffWaiter> {
        let entries = self.entries.lock().ok()?;
        let entry = entries.0.get(key)?;
        if entry.token.owner() != expected_owner || entry.terminal.reason().is_none() {
            return None;
        }
        Some(HandoffWaiter {
            token: entry.token.clone(),
            state: entry.state.subscribe(),
        })
    }

    #[cfg(test)]
    pub(super) fn waiter_count_for_test(&self, key: &GatewaySessionCacheKey) -> Option<usize> {
        self.entries
            .lock()
            .ok()?
            .0
            .get(key)
            .map(|entry| entry.state.receiver_count())
    }

    fn remove_owned(&self, key: &GatewaySessionCacheKey, token: &HandoffToken) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        if !entries
            .0
            .get(key)
            .is_some_and(|entry| entry.token.same(token))
        {
            return false;
        }
        entries.0.remove(key);
        true
    }
}

/// Must remain alive through teardown; dropping it cannot announce a successful save.
pub(super) struct HandoffBinding {
    registry: Arc<HandoffRegistry>,
    key: GatewaySessionCacheKey,
    token: HandoffToken,
    state: watch::Sender<HandoffState>,
    finished: bool,
}

impl HandoffBinding {
    pub(super) fn token(&self) -> HandoffToken {
        self.token.clone()
    }

    /// Host calls only after authoritative save, thaw and publication of the token-bound lease.
    pub(super) fn ready(mut self) {
        let state = if self.registry.remove_owned(&self.key, &self.token) {
            HandoffState::Ready
        } else {
            HandoffState::Failed
        };
        self.finished = true;
        // Retain the result even when a receiver has not polled yet. No registry lock is held.
        self.state.send_replace(state);
    }
}

impl Drop for HandoffBinding {
    fn drop(&mut self) {
        if !self.finished {
            self.registry.remove_owned(&self.key, &self.token);
            self.state.send_replace(HandoffState::Failed);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HandoffFailed;

impl fmt::Display for HandoffFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Web reconnect handoff is unavailable")
    }
}

impl std::error::Error for HandoffFailed {}

pub(super) struct HandoffWaiter {
    token: HandoffToken,
    state: watch::Receiver<HandoffState>,
}

impl HandoffWaiter {
    pub(super) fn token(&self) -> HandoffToken {
        self.token.clone()
    }

    /// Notification only. Cancellation never consumes the host's saved session or another waiter.
    pub(super) async fn wait(mut self) -> Result<HandoffToken, HandoffFailed> {
        loop {
            let state = *self.state.borrow_and_update();
            match state {
                HandoffState::Ready => return Ok(self.token),
                HandoffState::Failed => return Err(HandoffFailed),
                HandoffState::Pending => {}
            }
            if self.state.changed().await.is_err() {
                return Err(HandoffFailed);
            }
        }
    }
}

#[cfg(test)]
#[path = "web_reconnect_handoff_tests.rs"]
mod tests;

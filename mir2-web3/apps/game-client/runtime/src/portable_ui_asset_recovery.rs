//! Bounded recovery for the shared UI's retained font and skin handles.
use bevy::asset::{Asset, AssetPath, AssetServer, Handle, LoadState};
use std::{collections::HashMap, time::Duration};

#[derive(Default)]
pub(super) struct UiAssetRecovery {
    paths: HashMap<AssetPath<'static>, RetryState>,
}

#[derive(Default)]
struct RetryState {
    attempts: u8,
    failed_since: Option<Duration>,
}

impl RetryState {
    fn observe(&mut self, failed: bool, now: Duration) -> bool {
        if !failed {
            self.failed_since = None;
            return false;
        }
        if self.attempts >= 2 {
            return false;
        }
        let since = *self.failed_since.get_or_insert(now);
        if now.saturating_sub(since) < Duration::from_secs(1 << self.attempts) {
            return false;
        }
        self.attempts += 1;
        self.failed_since = None;
        true
    }
}

impl UiAssetRecovery {
    pub(super) fn retry_failed<A: Asset>(
        &mut self,
        server: &AssetServer,
        handle: &Handle<A>,
        now: Duration,
    ) {
        let Some(path) = server.get_path(handle.id()).map(AssetPath::into_owned) else {
            return;
        };
        let failed = matches!(
            server.get_load_state(handle.id()),
            Some(LoadState::Failed(_))
        );
        if !failed && !self.paths.contains_key(&path) {
            return;
        }
        if self
            .paths
            .entry(path.clone())
            .or_default()
            .observe(failed, now)
        {
            // Bevy re-enters Loading for Failed without changing the handle ID.
            // Do not reload Loading/Loaded assets or replace the current App.
            let retried = server.load::<A>(path);
            debug_assert_eq!(retried.id(), handle.id());
        }
    }
}

#[cfg(test)]
#[path = "portable_ui_asset_recovery_tests.rs"]
mod tests;

//! Presentation-only state for the native Dubhe Wallet login dialog.
//!
//! The Windows host owns the challenge, polling, signature verification and
//! gateway authentication. No authentication secrets belong in this resource.

use std::collections::VecDeque;

use bevy::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeWalletLoginStage {
    #[default]
    Creating,
    Pending,
    Approved,
    Rejected,
    Expired,
    Error,
}

impl NativeWalletLoginStage {
    pub fn can_refresh(self) -> bool {
        matches!(self, Self::Rejected | Self::Expired | Self::Error)
    }
}

/// Locale-neutral presentation reasons. Translate at render time so changing
/// language never restarts a live wallet challenge or freezes its old copy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeWalletLoginMessage {
    #[default]
    None,
    Creating,
    RestartRequired,
    ServiceUnavailable,
    ConnectionChanged,
    Cancelling,
    Expired,
    QrGenerationFailed,
    ConfirmOnPhone,
    CredentialExpired,
    Rejected,
    Retry,
}

#[derive(Clone, Default, PartialEq, Resource)]
pub struct NativeWalletLogin {
    pub visible: bool,
    pub stage: NativeWalletLoginStage,
    pub qr_image: Option<Handle<Image>>,
    pub remaining_seconds: u32,
    pub message: NativeWalletLoginMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum NativeWalletLoginAction {
    Begin,
    Refresh,
    Cancel,
}

#[derive(Debug, Default, Resource)]
pub struct NativeWalletLoginActionQueue {
    pending: VecDeque<NativeWalletLoginAction>,
}

impl NativeWalletLoginActionQueue {
    pub fn push(&mut self, action: NativeWalletLoginAction) {
        self.pending.push_back(action);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = NativeWalletLoginAction> + '_ {
        self.pending.drain(..)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_failed_or_finished_challenges_offer_refresh() {
        for stage in [
            NativeWalletLoginStage::Creating,
            NativeWalletLoginStage::Pending,
            NativeWalletLoginStage::Approved,
        ] {
            assert!(!stage.can_refresh());
        }
        for stage in [
            NativeWalletLoginStage::Rejected,
            NativeWalletLoginStage::Expired,
            NativeWalletLoginStage::Error,
        ] {
            assert!(stage.can_refresh());
        }
    }

    #[test]
    fn login_actions_preserve_cancel_before_restart_order() {
        let mut queue = NativeWalletLoginActionQueue::default();
        queue.push(NativeWalletLoginAction::Begin);
        queue.push(NativeWalletLoginAction::Cancel);
        queue.push(NativeWalletLoginAction::Begin);
        assert_eq!(
            queue.drain().collect::<Vec<_>>(),
            [
                NativeWalletLoginAction::Begin,
                NativeWalletLoginAction::Cancel,
                NativeWalletLoginAction::Begin,
            ]
        );
        assert!(queue.drain().next().is_none());
    }
}

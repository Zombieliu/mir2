//! Login and wallet presentation adapted to the canonical native locale.
//!
//! This value is an immutable render context, never another language resource.
use crate::native_i18n::{self, Locale};
use crate::native_wallet_login::{NativeWalletLoginMessage, NativeWalletLoginStage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiLocale(pub Locale);

impl Default for NativeUiLocale {
    fn default() -> Self {
        Self(native_i18n::locale())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeLoginCaption {
    Title,
    Account,
    Password,
    Login,
    NewAccount,
    ChangePassword,
    SafeKey,
    Cancel,
}

impl NativeUiLocale {
    fn text(self, key: &str, fallback: &str) -> String {
        native_i18n::for_locale(self.0, key, fallback)
    }

    pub fn login_caption(self, caption: NativeLoginCaption) -> String {
        use NativeLoginCaption::*;
        let (key, fallback) = match caption {
            Title => ("shell.login", "Login"),
            Account => ("shell.account", "Account"),
            Password => ("shell.password", "Password"),
            Login => ("shell.ok", "OK"),
            NewAccount => ("shell.new_account", "New Account"),
            ChangePassword => ("shell.change_password", "Change Password"),
            SafeKey => ("shell.safe_key", "Safe Key"),
            Cancel => ("shell.cancel", "Cancel"),
        };
        self.text(key, fallback)
    }
    pub fn wallet_title(self) -> String {
        self.text("wallet.login.title", "Dubhe Wallet QR login")
    }
    pub fn wallet_entry(self) -> String {
        format!("{}  [F8]", self.wallet_title())
    }
    pub fn wallet_instructions(self) -> String {
        self.text(
            "wallet.login.instructions",
            "Scan the code with your phone,\nthen sign the login message in Dubhe Wallet.",
        )
    }
    pub fn wallet_stage(self, stage: NativeWalletLoginStage) -> String {
        use NativeWalletLoginStage::*;
        let (key, fallback) = match stage {
            Creating => ("wallet.login.stage.creating", "Creating your login code"),
            Pending => ("wallet.login.stage.pending", "Waiting for phone approval"),
            Approved => (
                "wallet.login.stage.approved",
                "Wallet approved. Signing in…",
            ),
            Rejected => ("wallet.login.stage.rejected", "Login declined in wallet"),
            Expired => ("wallet.login.stage.expired", "This code has expired"),
            Error => ("wallet.login.stage.error", "QR login is unavailable"),
        };
        self.text(key, fallback)
    }
    pub fn wallet_countdown(self, seconds: u32) -> String {
        native_i18n::format_for(
            self.0,
            "wallet.login.countdown",
            "Expires in {seconds}s",
            &[("seconds", &seconds.to_string())],
        )
    }
    pub fn wallet_refresh(self) -> String {
        self.text("wallet.login.refresh", "Refresh code [Enter]")
    }
    pub fn wallet_cancel(self) -> String {
        self.text("wallet.login.cancel", "Cancel [Esc]")
    }
    pub fn message(self, message: NativeWalletLoginMessage) -> String {
        use NativeWalletLoginMessage::*;
        let (key, fallback) = match message {
            None => return String::new(),
            Creating => ("wallet.login.message.creating", "Creating your login code…"),
            RestartRequired => (
                "wallet.login.message.restart",
                "This login request has ended. Restart the game.",
            ),
            ServiceUnavailable => (
                "wallet.login.message.unavailable",
                "Wallet login is not configured or is unavailable.",
            ),
            ConnectionChanged => (
                "wallet.login.message.connection",
                "The game connection changed. Reconnect and retry.",
            ),
            Cancelling => (
                "wallet.login.message.cancelling",
                "The previous request is cancelling. Refresh shortly.",
            ),
            Expired => (
                "wallet.login.message.expired",
                "This code has expired. Refresh to retry.",
            ),
            QrGenerationFailed => (
                "wallet.login.message.qr_failed",
                "Code generation failed. Refresh to retry.",
            ),
            ConfirmOnPhone => (
                "wallet.login.message.confirm",
                "Confirm your login in the wallet on your phone.",
            ),
            CredentialExpired => (
                "wallet.login.message.credential_expired",
                "The login credential expired. Refresh to retry.",
            ),
            Rejected => (
                "wallet.login.message.rejected",
                "Login was declined in the wallet. Refresh to retry.",
            ),
            Retry => (
                "wallet.login.message.retry",
                "Wallet login is unavailable. Refresh to retry.",
            ),
        };
        self.text(key, fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wallet_catalog_covers_the_canonical_nine_locales() {
        for language in Locale::ALL {
            let locale = NativeUiLocale(language);
            assert!(!locale.wallet_title().is_empty());
            assert!(!locale
                .wallet_stage(NativeWalletLoginStage::Pending)
                .is_empty());
            assert!(locale.wallet_countdown(42).contains("42"));
            for reason in [
                NativeWalletLoginMessage::Creating,
                NativeWalletLoginMessage::RestartRequired,
                NativeWalletLoginMessage::ServiceUnavailable,
                NativeWalletLoginMessage::ConnectionChanged,
                NativeWalletLoginMessage::Cancelling,
                NativeWalletLoginMessage::Expired,
                NativeWalletLoginMessage::QrGenerationFailed,
                NativeWalletLoginMessage::ConfirmOnPhone,
                NativeWalletLoginMessage::CredentialExpired,
                NativeWalletLoginMessage::Rejected,
                NativeWalletLoginMessage::Retry,
            ] {
                assert!(!locale.message(reason).is_empty());
            }
            assert!(locale.message(NativeWalletLoginMessage::None).is_empty());
            native_i18n::with_locale(language, || {
                assert_eq!(NativeUiLocale::default().0, language)
            });
        }
    }
}

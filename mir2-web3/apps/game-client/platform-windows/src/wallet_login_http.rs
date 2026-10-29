//! Bounded native Dubhe Wallet authentication transport.
//!
//! Only the configured first-party origin receives the private intent and proof.
//! The public approval URL is displayed as a QR; it is never used as an HTTP endpoint.

use std::{
    fmt,
    io::Read,
    net::IpAddr,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Map, Value};
use url::Url;

const MAX_RESPONSE_BYTES: usize = 32_768;
const MAX_TOKEN_BYTES: usize = 12_000;
const MAX_LOGIN_TTL_MS: u64 = 300_000;
// Match the Gateway's configurable upper bound, including small clock skew.
const MAX_SESSION_TTL_MS: u64 = 30 * 24 * 60 * 60 * 1_000 + 30_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const WALLET_ORIGIN: &str = "https://wallet.dubhe.obelisk.build";
const INVALID_RESPONSE: &str = "Dubhe Wallet returned an invalid login response";
const CANCELLED: &str = "Dubhe Wallet login cancelled";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeWalletAuthConfig {
    pub origin: String,
}

pub struct CreatedWalletLogin {
    pub approval_url: String,
    pub expires_at_ms: u64,
    intent: String,
    // Bind the capability to the origin that created it, even if configuration changes.
    origin: String,
}

impl fmt::Debug for CreatedWalletLogin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreatedWalletLogin")
            .field("approval_url", &self.approval_url)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("intent", &"<redacted>")
            .finish()
    }
}

pub struct VerifiedWalletLogin {
    pub account_id: String,
    pub token: String,
    pub expires_at_ms: u64,
}

impl fmt::Debug for VerifiedWalletLogin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedWalletLogin")
            .field("account_id", &self.account_id)
            .field("token", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

#[derive(Debug)]
pub enum WalletLoginPoll {
    Pending,
    Rejected,
    Expired,
    Approved(VerifiedWalletLogin),
}

impl NativeWalletAuthConfig {
    /// Accept an already-normalized origin, with plaintext limited to loopback development.
    pub fn from_origin(origin: &str) -> Result<Self, String> {
        validate_origin(origin).map_err(|_| {
            "wallet auth URL must be a normalized https:// origin (http:// only on loopback)"
                .to_owned()
        })?;
        Ok(Self {
            origin: origin.to_owned(),
        })
    }

    pub fn create(&self, cancel: &AtomicBool) -> Result<CreatedWalletLogin, String> {
        let body = self.post_json("/api/wallet/dubhe/session", None, None, cancel)?;
        let body = object(&body)?;
        let approval_url = string(body, "approvalUrl", 4_096)?;
        validate_approval_url(approval_url)?;
        let intent = token(body, "intentToken")?;
        let expires_at_ms = timestamp(body, "expiresAt")?;
        let now = unix_ms();
        if expires_at_ms <= now || expires_at_ms > now.saturating_add(MAX_LOGIN_TTL_MS) {
            return Err(INVALID_RESPONSE.to_owned());
        }
        ensure_current(cancel)?;
        Ok(CreatedWalletLogin {
            approval_url: approval_url.to_owned(),
            expires_at_ms,
            intent: intent.to_owned(),
            origin: self.origin.clone(),
        })
    }

    /// Only returns a gateway token after the verified wallet proof has been exchanged
    /// for the server's canonical player identity. A signed wallet address is not a player id.
    pub fn poll(
        &self,
        created: &CreatedWalletLogin,
        cancel: &AtomicBool,
    ) -> Result<WalletLoginPoll, String> {
        ensure_current(cancel)?;
        if created.origin != self.origin {
            return Err("Dubhe Wallet login origin changed; please retry".to_owned());
        }
        if unix_ms() >= created.expires_at_ms {
            return Ok(WalletLoginPoll::Expired);
        }
        let response = self.post_json(
            "/api/wallet/dubhe/session/status",
            Some(&created.intent),
            None,
            cancel,
        )?;
        if unix_ms() >= created.expires_at_ms {
            return Ok(WalletLoginPoll::Expired);
        }
        let response = object(&response)?;
        match response.get("status").and_then(Value::as_str) {
            Some("pending") => Ok(WalletLoginPoll::Pending),
            Some("rejected") => Ok(WalletLoginPoll::Rejected),
            Some("expired") => Ok(WalletLoginPoll::Expired),
            Some("approved") => {
                let proof = object(response.get("login").ok_or(INVALID_RESPONSE)?)?;
                let account = string(proof, "accountId", 70)?;
                let expires_at = timestamp(proof, "expiresAt")?;
                if proof.get("provider").and_then(Value::as_str) != Some("suiWallet")
                    || !valid_sui_account(account)
                    || expires_at <= unix_ms()
                    || expires_at > created.expires_at_ms
                {
                    return Err(INVALID_RESPONSE.to_owned());
                }
                let credential = token(proof, "token")?;
                // Cancellation is checked again immediately before the single-use claim.
                ensure_current(cancel)?;
                if unix_ms() >= created.expires_at_ms {
                    return Ok(WalletLoginPoll::Expired);
                }
                let exchanged = self.post_json(
                    "/api/channels/session/exchange",
                    None,
                    Some(&json!({ "provider": "suiWallet", "credential": credential })),
                    cancel,
                )?;
                let exchanged = object(&exchanged)?;
                let account_id = string(exchanged, "accountId", 128)?;
                if !valid_player_id(account_id)
                    || exchanged.get("provider").and_then(Value::as_str) != Some("suiWallet")
                    || exchanged.get("playerId").and_then(Value::as_str) != Some(account_id)
                {
                    return Err(INVALID_RESPONSE.to_owned());
                }
                let session_token = token(exchanged, "token")?;
                let expires_at_ms = timestamp(exchanged, "expiresAt")?;
                let now = unix_ms();
                if expires_at_ms <= now || expires_at_ms > now.saturating_add(MAX_SESSION_TTL_MS) {
                    return Err(INVALID_RESPONSE.to_owned());
                }
                ensure_current(cancel)?;
                Ok(WalletLoginPoll::Approved(VerifiedWalletLogin {
                    account_id: account_id.to_owned(),
                    token: session_token.to_owned(),
                    expires_at_ms,
                }))
            }
            _ => Err(INVALID_RESPONSE.to_owned()),
        }
    }

    fn post_json(
        &self,
        path: &str,
        intent: Option<&str>,
        body: Option<&Value>,
        cancel: &AtomicBool,
    ) -> Result<Value, String> {
        ensure_current(cancel)?;
        // The field is public configuration; validate again before sending capabilities.
        Self::from_origin(&self.origin)?;
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout(REQUEST_TIMEOUT)
            .timeout_connect(REQUEST_TIMEOUT)
            .timeout_read(REQUEST_TIMEOUT)
            .timeout_write(REQUEST_TIMEOUT)
            .build();
        let mut request = agent
            .post(&format!("{}{path}", self.origin))
            .set("Origin", &self.origin)
            .set("Accept", "application/json")
            .set("Cache-Control", "no-store");
        if let Some(intent) = intent {
            request = request.set("Authorization", &format!("Bearer {intent}"));
        }
        let result = match body {
            Some(body) => request
                .set("Content-Type", "application/json")
                .send_string(&body.to_string()),
            None => request.call(),
        };
        // Never interpolate ureq errors: they can contain URLs or private response bodies.
        let response = result
            .map_err(|_| "Dubhe Wallet login service is unavailable; please retry".to_owned())?;
        ensure_current(cancel)?;
        if !(200..300).contains(&response.status()) {
            return Err("Dubhe Wallet login service refused the request".to_owned());
        }
        if response.header("Content-Length").is_some_and(|length| {
            length
                .parse::<u64>()
                .map_or(true, |length| length > MAX_RESPONSE_BYTES as u64)
        }) {
            return Err(INVALID_RESPONSE.to_owned());
        }
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_RESPONSE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| INVALID_RESPONSE.to_owned())?;
        ensure_current(cancel)?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(INVALID_RESPONSE.to_owned());
        }
        serde_json::from_slice(&bytes).map_err(|_| INVALID_RESPONSE.to_owned())
    }
}

fn validate_origin(origin: &str) -> Result<(), ()> {
    if origin.is_empty() || origin.len() > 2_048 || origin.bytes().any(|b| b <= b' ' || b >= 127) {
        return Err(());
    }
    let url = Url::parse(origin).map_err(|_| ())?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.origin().ascii_serialization() != origin
    {
        return Err(());
    }
    let host = url.host_str().ok_or(())?.trim_matches(['[', ']']);
    let loopback = host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    match url.scheme() {
        "https" => Ok(()),
        "http" if loopback => Ok(()),
        _ => Err(()),
    }
}

fn validate_approval_url(value: &str) -> Result<(), String> {
    let url = Url::parse(value).map_err(|_| INVALID_RESPONSE.to_owned())?;
    if url.origin().ascii_serialization() != WALLET_ORIGIN
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/connect/approve"
        || url.fragment().is_some()
        || value.bytes().any(|b| b <= b' ' || b >= 127)
    {
        return Err(INVALID_RESPONSE.to_owned());
    }
    let parameters: Vec<_> = url.query_pairs().collect();
    let session = parameters
        .iter()
        .find(|(key, _)| key == "session")
        .map(|(_, value)| value.as_ref());
    let relay = parameters
        .iter()
        .find(|(key, _)| key == "relay")
        .map(|(_, value)| value.as_ref());
    if parameters.len() != 2
        || !session.is_some_and(|id| {
            !id.is_empty()
                && id.len() <= 256
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        || !relay.is_some_and(|origin| validate_origin(origin).is_ok())
    {
        return Err(INVALID_RESPONSE.to_owned());
    }
    Ok(())
}

fn ensure_current(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err(CANCELLED.to_owned())
    } else {
        Ok(())
    }
}

fn object(value: &Value) -> Result<&Map<String, Value>, String> {
    value.as_object().ok_or_else(|| INVALID_RESPONSE.to_owned())
}

fn string<'a>(body: &'a Map<String, Value>, key: &str, limit: usize) -> Result<&'a str, String> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= limit
                && *value == value.trim()
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| INVALID_RESPONSE.to_owned())
}

fn token<'a>(body: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let token = string(body, key, MAX_TOKEN_BYTES)?;
    if token
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        Ok(token)
    } else {
        Err(INVALID_RESPONSE.to_owned())
    }
}

fn timestamp(body: &Map<String, Value>, key: &str) -> Result<u64, String> {
    body.get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value <= 9_007_199_254_740_991)
        .ok_or_else(|| INVALID_RESPONSE.to_owned())
}

fn valid_sui_account(account: &str) -> bool {
    account
        .strip_prefix("sui:0x")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn valid_player_id(account: &str) -> bool {
    account
        .strip_prefix("obl_")
        .is_some_and(|hex| hex.len() == 32 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
#[path = "wallet_login_http_tests.rs"]
mod tests;

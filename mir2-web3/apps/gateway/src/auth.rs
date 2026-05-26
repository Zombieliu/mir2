use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasskeyGatewayTokenPayload {
    auth: String,
    account_id: String,
    exp_ms: u64,
}

type HmacSha256 = Hmac<Sha256>;

pub(crate) fn verify_passkey_gateway_token(account_id: &str, token: &str) -> Result<(), String> {
    let (payload_b64, signature_b64) = token
        .split_once('.')
        .ok_or_else(|| "invalid passkey token".to_string())?;
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| "invalid passkey token payload".to_string())?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature_b64)
        .map_err(|_| "invalid passkey token signature".to_string())?;
    let payload: PasskeyGatewayTokenPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| "invalid passkey token payload".to_string())?;
    if payload.auth != "sui-passkey-v1" || payload.account_id != account_id {
        return Err("invalid passkey token".to_string());
    }
    if payload.exp_ms < unix_now_ms() {
        return Err("expired passkey token".to_string());
    }

    let secret = passkey_gateway_secret()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| "invalid passkey secret".to_string())?;
    mac.update(payload_b64.as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| "invalid passkey token signature".to_string())
}

fn passkey_gateway_secret() -> Result<String, String> {
    match env::var("MIR2_PASSKEY_AUTH_SECRET") {
        Ok(secret) if !secret.is_empty() => Ok(secret),
        _ if passkey_secret_required_from_env() => {
            Err("MIR2_PASSKEY_AUTH_SECRET is required for production passkey login".to_string())
        }
        _ => {
            eprintln!(
                "MIR2_PASSKEY_AUTH_SECRET is not set; using local development passkey secret"
            );
            Ok("mir2-web3-local-passkey-auth-secret".to_string())
        }
    }
}

fn passkey_secret_required_from_env() -> bool {
    ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"]
        .into_iter()
        .filter_map(|name| env::var(name).ok())
        .any(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "production" | "prod" | "staging"
            )
        })
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{passkey_gateway_secret, unix_now_ms, verify_passkey_gateway_token, HmacSha256};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    use hmac::{KeyInit, Mac};
    use serde_json::json;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn with_passkey_env<T>(vars: &[(&str, Option<&str>)], action: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock should not be poisoned");
        let names = [
            "MIR2_PASSKEY_AUTH_SECRET",
            "MIR2_RUNTIME_ENV",
            "MIR2_DEPLOYMENT_ENV",
            "MIR2_ENV",
        ];
        let previous = names.map(|name| (name, std::env::var(name).ok()));
        for name in names {
            std::env::remove_var(name);
        }
        for (name, value) in vars {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }

        let result = action();

        for (name, value) in previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        result
    }

    fn signed_passkey_token(account_id: &str, exp_ms: u64) -> String {
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "auth": "sui-passkey-v1",
                "accountId": account_id,
                "expMs": exp_ms,
            }))
            .expect("payload should serialize"),
        );
        let secret = passkey_gateway_secret().expect("test passkey secret should resolve");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("test passkey secret should initialize hmac");
        mac.update(payload.as_bytes());
        let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        format!("{payload}.{signature}")
    }

    #[test]
    fn passkey_gateway_token_requires_matching_hmac_account_and_expiry() {
        with_passkey_env(&[], || {
            let account_id = "sui:0xpasskey";
            let token = signed_passkey_token(account_id, unix_now_ms() + 60_000);

            assert!(verify_passkey_gateway_token(account_id, &token).is_ok());
            assert!(verify_passkey_gateway_token("sui:0xother", &token).is_err());

            let expired = signed_passkey_token(account_id, unix_now_ms().saturating_sub(1));
            assert!(verify_passkey_gateway_token(account_id, &expired).is_err());
        });
    }

    #[test]
    fn passkey_gateway_token_requires_configured_secret_in_production() {
        with_passkey_env(&[("MIR2_RUNTIME_ENV", Some("production"))], || {
            let account_id = "sui:0xpasskey";
            let payload = URL_SAFE_NO_PAD.encode(
                serde_json::to_vec(&json!({
                    "auth": "sui-passkey-v1",
                    "accountId": account_id,
                    "expMs": unix_now_ms() + 60_000,
                }))
                .expect("payload should serialize"),
            );
            let token = format!("{payload}.AA");
            let error = verify_passkey_gateway_token(account_id, &token)
                .expect_err("production passkey auth should require explicit secret");
            assert!(error.contains("MIR2_PASSKEY_AUTH_SECRET is required"));
        });
    }
}

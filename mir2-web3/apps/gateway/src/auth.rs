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
    jti: String,
    exp_ms: u64,
    auth_method: String,
}

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedPasskeyGatewayToken {
    pub token_id: String,
    pub expires_at_ms: u64,
    pub auth_method: &'static str,
}

pub(crate) fn verify_passkey_gateway_token(
    account_id: &str,
    token: &str,
) -> Result<VerifiedPasskeyGatewayToken, String> {
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
    if payload.auth != "sui-passkey-v2"
        || payload.account_id != account_id
        || payload.jti.trim().is_empty()
        || !matches!(payload.auth_method.as_str(), "passkey" | "wallet")
    {
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
        .map_err(|_| "invalid passkey token signature".to_string())?;
    Ok(VerifiedPasskeyGatewayToken {
        token_id: payload.jti,
        expires_at_ms: payload.exp_ms,
        auth_method: if payload.auth_method == "wallet" {
            "sui_wallet"
        } else {
            "sui_passkey"
        },
    })
}

fn passkey_gateway_secret() -> Result<String, String> {
    match env::var("MIR2_PASSKEY_AUTH_SECRET") {
        Ok(secret) if secret.len() >= 32 => Ok(secret),
        _ if passkey_secret_required_from_env() => {
            Err("MIR2_PASSKEY_AUTH_SECRET with at least 32 characters is required for production passkey login".to_string())
        }
        // Fail closed by default: the insecure local secret is only used when a
        // developer explicitly opts in. This prevents a misconfigured
        // deployment (one where the env-based production detection happens to
        // miss) from silently signing tokens with a publicly known key.
        _ if dev_passkey_secret_allowed() => {
            eprintln!(
                "MIR2_PASSKEY_AUTH_SECRET is not set; using local development passkey secret \
                 (MIR2_ALLOW_DEV_PASSKEY_SECRET is enabled)"
            );
            Ok("mir2-web3-local-passkey-auth-secret".to_string())
        }
        _ => Err("MIR2_PASSKEY_AUTH_SECRET is not set; set it, or set \
                  MIR2_ALLOW_DEV_PASSKEY_SECRET=1 to use the insecure local development secret"
            .to_string()),
    }
}

fn dev_passkey_secret_allowed() -> bool {
    env::var("MIR2_ALLOW_DEV_PASSKEY_SECRET")
        .map(|value| matches!(value.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
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

/// Verify the operator token presented by the trusted Relayer on the `/onchain/inject`
/// path (M4, WF-5). Constant-time compare against `MIR2_GATEWAY_OPERATOR_TOKEN`; fail-closed
/// in production like the passkey secret.
pub(crate) fn verify_operator_token(provided: &str) -> Result<(), String> {
    let expected = operator_token_secret()?;
    if constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err("invalid operator token".to_string())
    }
}

fn operator_token_secret() -> Result<String, String> {
    match env::var("MIR2_GATEWAY_OPERATOR_TOKEN") {
        Ok(secret) if !secret.is_empty() => Ok(secret),
        _ if passkey_secret_required_from_env() => Err(
            "MIR2_GATEWAY_OPERATOR_TOKEN is required for production on-chain injection".to_string(),
        ),
        // Fail closed by default; the insecure local token is only used when a developer
        // explicitly opts in, so a misconfigured deployment cannot accept a public token.
        _ if dev_operator_token_allowed() => {
            eprintln!(
                "MIR2_GATEWAY_OPERATOR_TOKEN is not set; using local development operator token \
                 (MIR2_ALLOW_DEV_OPERATOR_TOKEN is enabled)"
            );
            Ok("mir2-web3-local-operator-token".to_string())
        }
        _ => Err("MIR2_GATEWAY_OPERATOR_TOKEN is not set; set it, or set \
                  MIR2_ALLOW_DEV_OPERATOR_TOKEN=1 to use the insecure local development token"
            .to_string()),
    }
}

fn dev_operator_token_allowed() -> bool {
    env::var("MIR2_ALLOW_DEV_OPERATOR_TOKEN")
        .map(|value| matches!(value.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// Length-checked constant-time byte comparison (avoids leaking the token via timing).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        passkey_gateway_secret, unix_now_ms, verify_operator_token, verify_passkey_gateway_token,
        HmacSha256,
    };
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
            "MIR2_ALLOW_DEV_PASSKEY_SECRET",
        ];
        let previous = names.map(|name| (name, std::env::var(name).ok()));
        for name in names {
            std::env::remove_var(name);
        }
        // Tests exercise the local development secret path by default; opt in
        // unless a specific case overrides it below.
        std::env::set_var("MIR2_ALLOW_DEV_PASSKEY_SECRET", "1");
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

    fn signed_passkey_token(account_id: &str, exp_ms: u64, auth_method: &str) -> String {
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "auth": "sui-passkey-v2",
                "accountId": account_id,
                "jti": "test-login-token",
                "expMs": exp_ms,
                "authMethod": auth_method,
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
            let token = signed_passkey_token(account_id, unix_now_ms() + 60_000, "passkey");

            assert!(verify_passkey_gateway_token(account_id, &token).is_ok());
            assert!(verify_passkey_gateway_token("sui:0xother", &token).is_err());

            let expired =
                signed_passkey_token(account_id, unix_now_ms().saturating_sub(1), "passkey");
            assert!(verify_passkey_gateway_token(account_id, &expired).is_err());
        });
    }

    #[test]
    fn passkey_gateway_token_maps_only_supported_auth_methods() {
        with_passkey_env(&[], || {
            let account_id = "sui:0xwallet";
            let wallet = signed_passkey_token(account_id, unix_now_ms() + 60_000, "wallet");
            let verified = verify_passkey_gateway_token(account_id, &wallet)
                .expect("wallet token should be accepted");
            assert_eq!(verified.auth_method, "sui_wallet");

            let unknown = signed_passkey_token(account_id, unix_now_ms() + 60_000, "email");
            assert!(verify_passkey_gateway_token(account_id, &unknown).is_err());
        });
    }

    #[test]
    fn passkey_gateway_token_requires_configured_secret_in_production() {
        with_passkey_env(&[("MIR2_RUNTIME_ENV", Some("production"))], || {
            let account_id = "sui:0xpasskey";
            let payload = URL_SAFE_NO_PAD.encode(
                serde_json::to_vec(&json!({
                    "auth": "sui-passkey-v2",
                    "accountId": account_id,
                    "jti": "test-login-token",
                    "expMs": unix_now_ms() + 60_000,
                    "authMethod": "passkey",
                }))
                .expect("payload should serialize"),
            );
            let token = format!("{payload}.AA");
            let error = verify_passkey_gateway_token(account_id, &token)
                .expect_err("production passkey auth should require explicit secret");
            assert!(error.contains("MIR2_PASSKEY_AUTH_SECRET"));
            assert!(error.contains("at least 32 characters"));
        });
    }

    fn with_operator_env<T>(vars: &[(&str, Option<&str>)], action: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock should not be poisoned");
        let names = [
            "MIR2_GATEWAY_OPERATOR_TOKEN",
            "MIR2_ALLOW_DEV_OPERATOR_TOKEN",
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

    #[test]
    fn operator_token_accepts_match_and_rejects_mismatch() {
        with_operator_env(&[("MIR2_GATEWAY_OPERATOR_TOKEN", Some("s3cr3t"))], || {
            assert!(verify_operator_token("s3cr3t").is_ok());
            assert!(verify_operator_token("wrong-token").is_err());
            assert!(verify_operator_token("s3cr3").is_err()); // length mismatch
        });
    }

    #[test]
    fn operator_token_dev_fallback_requires_opt_in() {
        with_operator_env(&[("MIR2_ALLOW_DEV_OPERATOR_TOKEN", Some("1"))], || {
            assert!(verify_operator_token("mir2-web3-local-operator-token").is_ok());
        });
        with_operator_env(&[], || {
            assert!(verify_operator_token("anything").is_err());
        });
    }

    #[test]
    fn operator_token_required_in_production() {
        with_operator_env(&[("MIR2_RUNTIME_ENV", Some("production"))], || {
            let error = verify_operator_token("x").expect_err("production should require a token");
            assert!(error.contains("MIR2_GATEWAY_OPERATOR_TOKEN is required"));
        });
    }
}

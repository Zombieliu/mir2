use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SUI_LOGIN_PROOF_AUTH: &str = "sui-passkey-v1";
const CHANNEL_GUEST_PROOF_AUTH: &str = "mir2-channel-guest-v1";
const GATEWAY_IDENTITY_AUTH: &str = "mir2-identity-v1";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasskeyGatewayTokenPayload {
    auth: String,
    account_id: String,
    jti: String,
    exp_ms: u64,
    auth_method: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySuiLoginTokenPayload {
    auth: String,
    account_id: String,
    exp_ms: u64,
    #[serde(default)]
    provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SuiLoginProof {
    pub subject: String,
    pub provider: String,
    pub expires_at_ms: u64,
    pub token_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelGuestProof {
    pub auth: String,
    pub subject: String,
    pub provider: String,
    pub exp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GatewayIdentityClaims {
    pub auth: String,
    pub account_id: String,
    pub provider: String,
    #[serde(default)]
    pub subject: Option<String>,
    pub exp_ms: u64,
}

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedPasskeyGatewayToken {
    pub token_id: String,
    pub expires_at_ms: u64,
    pub auth_method: &'static str,
    pub credential_subject: Option<String>,
}

pub(crate) fn verify_passkey_gateway_token(
    account_id: &str,
    token: &str,
) -> Result<VerifiedPasskeyGatewayToken, String> {
    if let Ok(claims) = verify_gateway_identity_token(account_id, token) {
        if claims.account_id == account_id {
            let auth_method = match claims.provider.as_str() {
                "suiWallet" => "sui_wallet",
                "suiPasskey" => "sui_passkey",
                _ => "channel",
            };
            return Ok(VerifiedPasskeyGatewayToken {
                token_id: format!(
                    "gateway:{}",
                    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
                ),
                expires_at_ms: claims.exp_ms,
                auth_method,
                credential_subject: claims.subject,
            });
        }
    }
    let payload: PasskeyGatewayTokenPayload = decode_and_verify_hmac_token(token, "passkey token")?;
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
    Ok(VerifiedPasskeyGatewayToken {
        token_id: payload.jti,
        expires_at_ms: payload.exp_ms,
        auth_method: if payload.auth_method == "wallet" {
            "sui_wallet"
        } else {
            "sui_passkey"
        },
        credential_subject: payload.account_id.strip_prefix("sui:").map(str::to_string),
    })
}

pub(crate) fn verify_sui_login_proof(token: &str) -> Result<SuiLoginProof, String> {
    if let Ok(payload) =
        decode_and_verify_hmac_token::<PasskeyGatewayTokenPayload>(token, "Sui login proof")
    {
        if payload.auth != "sui-passkey-v2" || !payload.account_id.starts_with("sui:") {
            return Err("invalid Sui login proof".to_string());
        }
        if payload.exp_ms < unix_now_ms() {
            return Err("expired Sui login proof".to_string());
        }
        let provider = match payload.auth_method.as_str() {
            "passkey" => "suiPasskey",
            "wallet" => "suiWallet",
            _ => return Err("invalid Sui login proof provider".to_string()),
        };
        return Ok(SuiLoginProof {
            subject: payload.account_id,
            provider: provider.to_string(),
            expires_at_ms: payload.exp_ms,
            token_id: Some(payload.jti),
        });
    }
    if passkey_secret_required_from_env() {
        return Err("legacy Sui login proofs are not accepted in production".to_string());
    }
    let payload: LegacySuiLoginTokenPayload =
        decode_and_verify_hmac_token(token, "legacy Sui login proof")?;
    if payload.auth != SUI_LOGIN_PROOF_AUTH || !payload.account_id.starts_with("sui:") {
        return Err("invalid Sui login proof".to_string());
    }
    if payload.exp_ms < unix_now_ms() {
        return Err("expired Sui login proof".to_string());
    }
    let provider = payload.provider.unwrap_or_else(|| "suiPasskey".to_string());
    if !matches!(provider.as_str(), "suiPasskey" | "suiWallet") {
        return Err("invalid Sui login proof provider".to_string());
    }
    Ok(SuiLoginProof {
        subject: payload.account_id,
        provider,
        expires_at_ms: payload.exp_ms,
        token_id: None,
    })
}

pub(crate) fn verify_channel_guest_proof(token: &str) -> Result<ChannelGuestProof, String> {
    let proof: ChannelGuestProof = decode_and_verify_hmac_token(token, "channel guest proof")?;
    if proof.auth != CHANNEL_GUEST_PROOF_AUTH
        || !proof.subject.starts_with("guest:")
        || !matches!(
            proof.provider.as_str(),
            "directGuest" | "itch" | "crazyGamesGuest"
        )
    {
        return Err("invalid channel guest proof".to_string());
    }
    if proof.exp_ms < unix_now_ms() {
        return Err("expired channel guest proof".to_string());
    }
    Ok(proof)
}

#[cfg(test)]
pub(crate) fn issue_gateway_identity_token(
    account_id: &str,
    provider: &str,
    expires_at_ms: u64,
) -> Result<String, String> {
    issue_gateway_identity_token_for_subject(account_id, provider, None, expires_at_ms)
}

pub(crate) fn issue_gateway_identity_token_for_subject(
    account_id: &str,
    provider: &str,
    subject: Option<&str>,
    expires_at_ms: u64,
) -> Result<String, String> {
    if !account_id.starts_with("obl_") {
        return Err("gateway identity token requires an Obelisk player id".to_string());
    }
    if provider.trim().is_empty() {
        return Err("gateway identity token provider is required".to_string());
    }
    if expires_at_ms <= unix_now_ms() {
        return Err("gateway identity token expiry must be in the future".to_string());
    }
    sign_hmac_token(&GatewayIdentityClaims {
        auth: GATEWAY_IDENTITY_AUTH.to_string(),
        account_id: account_id.to_string(),
        provider: provider.to_string(),
        subject: subject.map(str::to_string),
        exp_ms: expires_at_ms,
    })
}

pub(crate) fn verify_gateway_identity_token(
    account_id: &str,
    token: &str,
) -> Result<GatewayIdentityClaims, String> {
    let claims: GatewayIdentityClaims =
        decode_and_verify_hmac_token(token, "gateway identity token")?;
    if claims.auth != GATEWAY_IDENTITY_AUTH || claims.account_id != account_id {
        return Err("invalid gateway identity token".to_string());
    }
    if claims.exp_ms < unix_now_ms() {
        return Err("expired gateway identity token".to_string());
    }
    Ok(claims)
}

fn decode_and_verify_hmac_token<T>(token: &str, label: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let (payload_b64, signature_b64) = token
        .split_once('.')
        .ok_or_else(|| format!("invalid {label}"))?;
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| format!("invalid {label} payload"))?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature_b64)
        .map_err(|_| format!("invalid {label} signature"))?;
    let secret = passkey_gateway_secret()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| "invalid passkey secret".to_string())?;
    mac.update(payload_b64.as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| format!("invalid {label} signature"))?;
    serde_json::from_slice(&payload_bytes).map_err(|_| format!("invalid {label} payload"))
}

fn sign_hmac_token(payload: &impl Serialize) -> Result<String, String> {
    let payload_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(payload)
            .map_err(|error| format!("gateway identity token encode failed: {error}"))?,
    );
    let secret = passkey_gateway_secret()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| "invalid passkey secret".to_string())?;
    mac.update(payload_b64.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Ok(format!("{payload_b64}.{signature}"))
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
        issue_gateway_identity_token, passkey_gateway_secret, unix_now_ms,
        verify_channel_guest_proof, verify_gateway_identity_token, verify_operator_token,
        verify_passkey_gateway_token, verify_sui_login_proof, HmacSha256,
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

    fn signed_passkey_token(account_id: &str, exp_ms: u64) -> String {
        signed_sui_token(account_id, exp_ms, None)
    }

    fn signed_sui_token(account_id: &str, exp_ms: u64, provider: Option<&str>) -> String {
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "auth": "sui-passkey-v2",
                "accountId": account_id,
                "jti": "test-login-token",
                "expMs": exp_ms,
                "authMethod": if provider == Some("suiWallet") { "wallet" } else { "passkey" },
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

    fn signed_guest_token(subject: &str, provider: &str, exp_ms: u64) -> String {
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "auth": "mir2-channel-guest-v1",
                "subject": subject,
                "provider": provider,
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
    fn channel_guest_proof_is_provider_bound_and_expires() {
        with_passkey_env(&[], || {
            let token = signed_guest_token(
                "guest:91c2143e-7356-4d64-b8b8-3d85f11b7f85",
                "crazyGamesGuest",
                unix_now_ms().saturating_add(60_000),
            );
            let proof = verify_channel_guest_proof(&token).expect("guest proof should verify");
            assert_eq!(proof.provider, "crazyGamesGuest");
            assert_eq!(proof.subject, "guest:91c2143e-7356-4d64-b8b8-3d85f11b7f85");

            let expired = signed_guest_token(
                "guest:91c2143e-7356-4d64-b8b8-3d85f11b7f85",
                "crazyGamesGuest",
                unix_now_ms().saturating_sub(1),
            );
            assert!(verify_channel_guest_proof(&expired).is_err());
        });
    }

    #[test]
    fn sui_proof_preserves_passkey_or_wallet_provider() {
        with_passkey_env(&[], || {
            for provider in ["suiPasskey", "suiWallet"] {
                let token = signed_sui_token(
                    "sui:0xprovider",
                    unix_now_ms().saturating_add(60_000),
                    Some(provider),
                );
                let proof = verify_sui_login_proof(&token).expect("Sui proof should verify");
                assert_eq!(proof.subject, "sui:0xprovider");
                assert_eq!(proof.provider, provider);
            }
        });
    }

    #[test]
    fn canonical_gateway_identity_token_is_bound_to_obelisk_player() {
        with_passkey_env(&[], || {
            let account_id = "obl_00112233445566778899aabbccddeeff";
            let token = issue_gateway_identity_token(
                account_id,
                "suiPasskey",
                unix_now_ms().saturating_add(60_000),
            )
            .expect("identity token should issue");
            let claims = verify_gateway_identity_token(account_id, &token)
                .expect("identity token should verify");
            assert_eq!(claims.provider, "suiPasskey");
            assert!(
                verify_gateway_identity_token("obl_ffeeddccbbaa99887766554433221100", &token)
                    .is_err()
            );
            assert!(verify_passkey_gateway_token(account_id, &token).is_ok());
        });
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

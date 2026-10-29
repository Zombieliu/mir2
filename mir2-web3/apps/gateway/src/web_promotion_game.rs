//! A test-realm gameplay capability, never a commercial identity bearer.
//! The website's parent session and wallet credential remain authoritative.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, KeyInit, Mac};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub(crate) const REALM_ID: &str = "numeron-promotion-testnet";
const TOKEN_AUTH: &str = "mir2-promotion-game-v1";
const TOKEN_SCOPE: &str = "gameplay";
const TOKEN_TTL_MS: u64 = 60_000;
const TOKEN_PREFIX: &str = "pg1.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GameSessionRequest {}

struct Profile {
    secret: String,
}

/// A configured but invalid profile still closes legacy login/debug paths.
pub(crate) fn profile_requested() -> bool {
    requested_value(env::var("MIR2_PROMOTION_GAMEPLAY_ENABLED").ok().as_deref())
}

fn requested_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "off"
        )
    })
}

impl Profile {
    fn load() -> Result<Self, String> {
        if !profile_requested()
            || env::var("MIR2_PROMOTION_ENVIRONMENT").as_deref() != Ok("test")
            || env::var("MIR2_PROMOTION_GAMEPLAY_REALM_ID").as_deref() != Ok(REALM_ID)
        {
            return Err("promotionConfigurationInvalid".into());
        }
        let secret = env::var("MIR2_PROMOTION_GAMEPLAY_TOKEN_SECRET")
            .map_err(|_| "promotionConfigurationInvalid")?;
        if secret.len() < 32
            || secret.trim() != secret
            || ["MIR2_PASSKEY_AUTH_SECRET", "MIR2_IDENTITY_SESSION_SECRET"]
                .into_iter()
                .any(|key| env::var(key).is_ok_and(|other| other == secret))
        {
            return Err("promotionConfigurationInvalid".into());
        }
        Ok(Self { secret })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Claims {
    auth: String,
    scope: String,
    realm_id: String,
    account_id: String,
    parent_session_id: String,
    parent_expires_at_ms: u64,
    credential_id: String,
    jti: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Clone)]
pub(super) struct GameBinding {
    claims: Claims,
    credential_subject: String,
}

impl GameBinding {
    pub(super) fn credential_subject(&self) -> &str {
        &self.credential_subject
    }
    pub(super) fn identity(&self) -> VerifiedIdentitySession {
        VerifiedIdentitySession {
            account_id: self.claims.account_id.clone(),
            session_id: self.claims.parent_session_id.clone(),
            expires_at_ms: self.claims.parent_expires_at_ms,
        }
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_:-.".contains(&byte))
}

fn sign(profile: &Profile, claims: &Claims) -> Result<String, String> {
    let payload =
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).map_err(|_| "promotionUnavailable")?);
    let mut mac = Hmac::<Sha256>::new_from_slice(profile.secret.as_bytes())
        .map_err(|_| "promotionConfigurationInvalid")?;
    mac.update(TOKEN_AUTH.as_bytes());
    mac.update(b"\0");
    mac.update(payload.as_bytes());
    Ok(format!(
        "{TOKEN_PREFIX}{payload}.{}",
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    ))
}

fn decode(profile: &Profile, account_id: &str, token: &str, now_ms: u64) -> Result<Claims, String> {
    let rejected = || "promotionAuthenticationRequired".to_string();
    if token.len() > 8_192 {
        return Err(rejected());
    }
    let encoded = token.strip_prefix(TOKEN_PREFIX).ok_or_else(rejected)?;
    let (payload, signature) = encoded.split_once('.').ok_or_else(rejected)?;
    let signature = URL_SAFE_NO_PAD.decode(signature).map_err(|_| rejected())?;
    let mut mac =
        Hmac::<Sha256>::new_from_slice(profile.secret.as_bytes()).map_err(|_| rejected())?;
    mac.update(TOKEN_AUTH.as_bytes());
    mac.update(b"\0");
    mac.update(payload.as_bytes());
    mac.verify_slice(&signature).map_err(|_| rejected())?;
    let claims: Claims =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).map_err(|_| rejected())?)
            .map_err(|_| rejected())?;
    if claims.auth != TOKEN_AUTH
        || claims.scope != TOKEN_SCOPE
        || claims.realm_id != REALM_ID
        || claims.account_id != account_id
        || !valid_identifier(&claims.account_id)
        || !valid_identifier(&claims.parent_session_id)
        || !valid_identifier(&claims.credential_id)
        || !valid_identifier(&claims.jti)
        || claims.issued_at_ms > now_ms.saturating_add(5_000)
        || claims.expires_at_ms <= now_ms
        || claims.expires_at_ms <= claims.issued_at_ms
        || claims.expires_at_ms.saturating_sub(claims.issued_at_ms) > TOKEN_TTL_MS
        || claims.expires_at_ms > claims.parent_expires_at_ms
    {
        return Err(rejected());
    }
    Ok(claims)
}

fn require_durable_backend(
    config: &GatewayConfig,
    identity: &IdentityService,
    cache: &dyn crate::cache::GatewaySessionCache,
) -> Result<(), String> {
    if config.account_store_database_url.is_none()
        || config.account_store_database_mode
            != mir2_simulation::AccountStoreDatabaseMode::SourceOfTruth
        || identity.backend_label() != "postgres"
    {
        return Err("promotionConfigurationInvalid".into());
    }
    let status = cache.status();
    if status.backend != "redis" || !status.healthy {
        return Err("promotionUnavailable".into());
    }
    Ok(())
}

fn current_wallet(
    identity: &IdentityService,
    cache: &dyn crate::cache::GatewaySessionCache,
    parent: &VerifiedIdentitySession,
    now_ms: u64,
) -> Result<crate::identity::VerifiedWalletCredential, String> {
    if cache
        .identity_session_is_revoked(&parent.session_id)
        .map_err(|_| "promotionUnavailable")?
    {
        return Err("promotionAuthenticationRequired".into());
    }
    if parent.expires_at_ms <= now_ms {
        return Err("promotionAuthenticationRequired".into());
    }
    let credential = identity
        .verified_wallet_credential(parent)
        .map_err(|_| "promotionAuthenticationRequired")?;
    if identity
        .resolve_sui_account("sui_wallet", &credential.credential_subject)
        .map_err(|_| "promotionUnavailable")?
        .as_deref()
        != Some(parent.account_id.as_str())
    {
        return Err("promotionAuthenticationRequired".into());
    }
    Ok(credential)
}

pub(super) fn issue(state: &WebState, parent: &VerifiedIdentitySession) -> Result<Value, String> {
    let profile = Profile::load()?;
    require_durable_backend(&state.config, &state.identity, state.session_cache.as_ref())?;
    let now_ms = gateway_now_ms();
    let credential = current_wallet(
        &state.identity,
        state.session_cache.as_ref(),
        parent,
        now_ms,
    )?;
    ensure_account(&state.config, &parent.account_id)?;
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let claims = Claims {
        auth: TOKEN_AUTH.into(),
        scope: TOKEN_SCOPE.into(),
        realm_id: REALM_ID.into(),
        account_id: parent.account_id.clone(),
        parent_session_id: parent.session_id.clone(),
        parent_expires_at_ms: parent.expires_at_ms,
        credential_id: credential.credential_id,
        jti: URL_SAFE_NO_PAD.encode(nonce),
        issued_at_ms: now_ms,
        expires_at_ms: now_ms
            .saturating_add(TOKEN_TTL_MS)
            .min(parent.expires_at_ms),
    };
    let token = sign(&profile, &claims)?;
    // This is a distinct capability. No issue_session call or commercial bearer is returned.
    Ok(
        json!({"accountId":claims.account_id,"token":token,"expiresAt":claims.expires_at_ms,
        "realmId":REALM_ID,"provider":"suiWallet"}),
    )
}

fn ensure_account(config: &GatewayConfig, account_id: &str) -> Result<(), String> {
    match mir2_simulation::SimulationSession::passkey_login_recovery_preflight(config, account_id)
        .map_err(|_| "promotionUnavailable")?
    {
        mir2_simulation::PasskeyRecoveryPreflight::ExistingEligible => Ok(()),
        _ => Err("promotionAuthenticationRequired".into()),
    }
}

pub(super) fn validate_binding(
    config: &GatewayConfig,
    identity: &IdentityService,
    cache: &dyn crate::cache::GatewaySessionCache,
    binding: &GameBinding,
) -> Result<(), String> {
    Profile::load()?;
    validate_wallet_binding(identity, cache, binding, gateway_now_ms())?;
    ensure_account(config, &binding.claims.account_id)
}

fn validate_wallet_binding(
    identity: &IdentityService,
    cache: &dyn crate::cache::GatewaySessionCache,
    binding: &GameBinding,
    now_ms: u64,
) -> Result<(), String> {
    let credential = current_wallet(identity, cache, &binding.identity(), now_ms)?;
    if credential.credential_id != binding.claims.credential_id
        || credential.credential_subject != binding.credential_subject
    {
        return Err("promotionAuthenticationRequired".into());
    }
    Ok(())
}

pub(super) fn bind_login(
    config: &GatewayConfig,
    identity: &IdentityService,
    cache: &dyn crate::cache::GatewaySessionCache,
    account_id: &str,
    token: &str,
) -> Result<GameBinding, String> {
    require_durable_backend(config, identity, cache)?;
    let claims = decode(&Profile::load()?, account_id, token, gateway_now_ms())?;
    let parent = VerifiedIdentitySession {
        account_id: claims.account_id.clone(),
        session_id: claims.parent_session_id.clone(),
        expires_at_ms: claims.parent_expires_at_ms,
    };
    let credential = current_wallet(identity, cache, &parent, gateway_now_ms())?;
    if credential.credential_id != claims.credential_id {
        return Err("promotionAuthenticationRequired".into());
    }
    let binding = GameBinding {
        claims,
        credential_subject: credential.credential_subject,
    };
    validate_binding(config, identity, cache, &binding)?;
    Ok(binding)
}

/// Used by both normal web executors; HTTP commercial-token verification never delegates here.
pub(crate) fn verify_login_token(
    account_id: &str,
    token: &str,
) -> Result<crate::auth::VerifiedPasskeyGatewayToken, String> {
    let claims = decode(&Profile::load()?, account_id, token, gateway_now_ms())?;
    Ok(crate::auth::VerifiedPasskeyGatewayToken {
        token_id: format!("promotion-game:{}:{}", claims.realm_id, claims.jti),
        expires_at_ms: claims.expires_at_ms,
        auth_method: "sui_wallet",
        credential_subject: None,
    })
}

pub(super) fn browser_command_allowed(command: &BrowserCommand) -> bool {
    !matches!(
        command,
        BrowserCommand::Login { .. }
            | BrowserCommand::NewAccount { .. }
            | BrowserCommand::ChangePassword { .. }
            | BrowserCommand::ResumeSession(_)
            | BrowserCommand::QaControl { .. }
            | BrowserCommand::MoveTo { .. }
            | BrowserCommand::Stage5Command { .. }
    )
}

#[cfg(test)]
#[path = "web_promotion_game_tests.rs"]
mod tests;

//! Production-safe native gateway session configuration.

use std::net::IpAddr;

use tokio_tungstenite::tungstenite::http::Uri;

pub const ACCOUNT_ENV: &str = "MIR2_NATIVE_ACCOUNT";
pub const PASSWORD_ENV: &str = "MIR2_NATIVE_PASSWORD";
pub const CHARACTER_INDEX_ENV: &str = "MIR2_NATIVE_CHARACTER_INDEX";
pub const GATEWAY_URL_ENV: &str = "MIR2_GATEWAY_WS_URL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSessionConfig {
    pub account_id: String,
    pub password: String,
    pub character_index: i32,
    pub gateway_url: String,
}

impl NativeSessionConfig {
    pub fn from_env(default_gateway_url: &str) -> Result<Self, String> {
        Self::from_values(
            std::env::var(ACCOUNT_ENV).ok(),
            std::env::var(PASSWORD_ENV).ok(),
            std::env::var(CHARACTER_INDEX_ENV).ok(),
            std::env::var(GATEWAY_URL_ENV).ok(),
            default_gateway_url,
        )
    }

    fn from_values(
        account_id: Option<String>,
        password: Option<String>,
        character_index: Option<String>,
        gateway_url: Option<String>,
        default_gateway_url: &str,
    ) -> Result<Self, String> {
        let account_id = required_non_empty(account_id, ACCOUNT_ENV)?;
        let password = required_non_empty(password, PASSWORD_ENV)?;
        let character_index = character_index
            .as_deref()
            .unwrap_or("0")
            .parse::<i32>()
            .map_err(|_| format!("{CHARACTER_INDEX_ENV} must be a non-negative integer"))?;
        if character_index < 0 {
            return Err(format!(
                "{CHARACTER_INDEX_ENV} must be a non-negative integer"
            ));
        }

        let gateway_url = gateway_url
            .as_deref()
            .unwrap_or(default_gateway_url)
            .trim()
            .to_owned();
        validate_gateway_url(&gateway_url)?;

        Ok(Self {
            account_id,
            password,
            character_index,
            gateway_url,
        })
    }
}

fn validate_gateway_url(gateway_url: &str) -> Result<(), String> {
    let uri = gateway_url
        .parse::<Uri>()
        .map_err(|_| format!("{GATEWAY_URL_ENV} must be a valid WebSocket URL"))?;
    let authority = uri
        .authority()
        .ok_or_else(|| format!("{GATEWAY_URL_ENV} must include a host"))?;
    if authority.as_str().contains('@') {
        return Err(format!("{GATEWAY_URL_ENV} must not contain credentials"));
    }

    let host = uri
        .host()
        .ok_or_else(|| format!("{GATEWAY_URL_ENV} must include a host"))?
        .trim_matches(['[', ']']);
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());

    match uri.scheme_str() {
        Some("wss") => Ok(()),
        Some("ws") if is_loopback => Ok(()),
        Some("ws") => Err(format!(
            "{GATEWAY_URL_ENV} must use wss:// (ws:// is allowed only for loopback development)"
        )),
        _ => Err(format!("{GATEWAY_URL_ENV} must use ws:// or wss://")),
    }
}

fn required_non_empty(value: Option<String>, name: &str) -> Result<String, String> {
    let value = value.ok_or_else(|| format!("{name} is required; credentials have no default"))?;
    if value.trim().is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credentials_are_rejected() {
        let error =
            NativeSessionConfig::from_values(None, None, None, None, "ws://127.0.0.1:7110/ws")
                .expect_err("implicit demo credentials must never be accepted");

        assert!(error.contains(ACCOUNT_ENV));
    }

    #[test]
    fn explicit_credentials_and_defaults_are_accepted() {
        let config = NativeSessionConfig::from_values(
            Some("player-one".to_owned()),
            Some("secret".to_owned()),
            None,
            None,
            "ws://127.0.0.1:7110/ws",
        )
        .expect("explicit credentials should be valid");

        assert_eq!(config.account_id, "player-one");
        assert_eq!(config.password, "secret");
        assert_eq!(config.character_index, 0);
        assert_eq!(config.gateway_url, "ws://127.0.0.1:7110/ws");
    }

    #[test]
    fn invalid_character_index_and_gateway_scheme_are_rejected() {
        let invalid_index = NativeSessionConfig::from_values(
            Some("player".to_owned()),
            Some("secret".to_owned()),
            Some("-1".to_owned()),
            None,
            "ws://127.0.0.1:7110/ws",
        );
        assert!(invalid_index.is_err());

        let invalid_gateway = NativeSessionConfig::from_values(
            Some("player".to_owned()),
            Some("secret".to_owned()),
            None,
            Some("https://example.invalid/ws".to_owned()),
            "ws://127.0.0.1:7110/ws",
        );
        assert!(invalid_gateway.is_err());
    }

    #[test]
    fn plaintext_gateway_is_limited_to_loopback_and_url_credentials_are_rejected() {
        assert!(validate_gateway_url("ws://127.0.0.1:7110/ws").is_ok());
        assert!(validate_gateway_url("ws://[::1]:7110/ws").is_ok());
        assert!(validate_gateway_url("ws://localhost:7110/ws").is_ok());
        assert!(validate_gateway_url("wss://gateway.example.com/ws").is_ok());

        assert!(validate_gateway_url("ws://gateway.example.com/ws").is_err());
        assert!(validate_gateway_url("wss://user:secret@gateway.example.com/ws").is_err());
        assert!(validate_gateway_url("wss:///ws").is_err());
    }
}

//! Production-safe native gateway session configuration.

use std::{fmt, fs, net::IpAddr, path::PathBuf, time::Duration};

use tokio_tungstenite::tungstenite::http::Uri;

pub const ACCOUNT_ENV: &str = "MIR2_NATIVE_ACCOUNT";
pub const PASSWORD_ENV: &str = "MIR2_NATIVE_PASSWORD";
pub const CHARACTER_INDEX_ENV: &str = "MIR2_NATIVE_CHARACTER_INDEX";
pub const GATEWAY_URL_ENV: &str = "MIR2_GATEWAY_WS_URL";
pub const CONFIG_FILE_NAME: &str = "mir2-client.toml";
pub const DEFAULT_WINDOW_WIDTH: u32 = 1024;
pub const DEFAULT_WINDOW_HEIGHT: u32 = 768;
pub const RESUME_DEADLINE_MS_ENV: &str = "MIR2_NATIVE_RESUME_DEADLINE_MS";
pub const RESUME_INITIAL_BACKOFF_MS_ENV: &str = "MIR2_NATIVE_RESUME_INITIAL_BACKOFF_MS";
pub const RESUME_MAX_BACKOFF_MS_ENV: &str = "MIR2_NATIVE_RESUME_MAX_BACKOFF_MS";
pub const RESUME_JITTER_PERCENT_ENV: &str = "MIR2_NATIVE_RESUME_JITTER_PERCENT";
pub const RESUME_COMMAND_BATCH_ENV: &str = "MIR2_NATIVE_RESUME_COMMAND_BATCH";
const NATIVE_RESUME_MAX_ATTEMPTS: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeReconnectConfig {
    pub resume_deadline: Duration,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub jitter_percent: u8,
    pub command_batch_limit: usize,
    pub max_attempts: u8,
}

impl Default for NativeReconnectConfig {
    fn default() -> Self {
        Self {
            resume_deadline: Duration::from_secs(14),
            initial_backoff: Duration::from_millis(250),
            max_backoff: Duration::from_secs(5),
            jitter_percent: 20,
            command_batch_limit: 256,
            max_attempts: NATIVE_RESUME_MAX_ATTEMPTS,
        }
    }
}

impl NativeReconnectConfig {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();
        let resume_deadline = duration_env(
            RESUME_DEADLINE_MS_ENV,
            defaults.resume_deadline,
            1_000,
            14_000,
        )?;
        let initial_backoff = duration_env(
            RESUME_INITIAL_BACKOFF_MS_ENV,
            defaults.initial_backoff,
            25,
            30_000,
        )?;
        let max_backoff = duration_env(
            RESUME_MAX_BACKOFF_MS_ENV,
            defaults.max_backoff,
            initial_backoff.as_millis() as u64,
            60_000,
        )?;
        let jitter_percent = bounded_env_u64(
            RESUME_JITTER_PERCENT_ENV,
            u64::from(defaults.jitter_percent),
            0,
            50,
        )? as u8;
        let command_batch_limit = bounded_env_u64(
            RESUME_COMMAND_BATCH_ENV,
            defaults.command_batch_limit as u64,
            8,
            4096,
        )? as usize;
        Ok(Self {
            resume_deadline,
            initial_backoff,
            max_backoff,
            jitter_percent,
            command_batch_limit,
            max_attempts: defaults.max_attempts,
        })
    }
}

const FORBIDDEN_CONFIG_KEYS: &[&str] = &[
    "account",
    "account_id",
    "accountid",
    "password",
    "passwd",
    "token",
    "passkey",
    "secret",
    "secret_question",
    "secret_answer",
    "email",
    "email_address",
    "authorization",
    "bearer",
    "credential",
    "credentials",
];

#[derive(Clone, PartialEq, Eq)]
pub struct NativeAutoLogin {
    pub account_id: String,
    pub password: String,
    pub character_index: Option<i32>,
}

impl fmt::Debug for NativeAutoLogin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeAutoLogin")
            .field("account_id", &self.account_id)
            .field("password", &"<redacted>")
            .field("character_index", &self.character_index)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSessionConfig {
    pub gateway_url: String,
    /// Explicit development convenience only. Normal players enter credentials
    /// in the visible native login screen, so missing environment credentials
    /// must not prevent the window from opening.
    pub auto_login: Option<NativeAutoLogin>,
    pub window_width: u32,
    pub window_height: u32,
    pub reconnect: NativeReconnectConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ClientFileConfig {
    gateway_ws_url: Option<String>,
    window_width: Option<u32>,
    window_height: Option<u32>,
}

impl NativeSessionConfig {
    pub fn load(default_gateway_url: &str) -> Result<Self, String> {
        let file = read_client_file()?;
        let env_gateway = optional_non_empty(std::env::var(GATEWAY_URL_ENV).ok(), GATEWAY_URL_ENV)?;
        let file_gateway = file
            .as_ref()
            .and_then(|config| config.gateway_ws_url.clone());
        let mut config = Self::from_values(
            std::env::var(ACCOUNT_ENV).ok(),
            std::env::var(PASSWORD_ENV).ok(),
            std::env::var(CHARACTER_INDEX_ENV).ok(),
            env_gateway.or(file_gateway),
            default_gateway_url,
        )?;
        config.reconnect = NativeReconnectConfig::from_env()?;
        if let Some(file) = file {
            if let Some(width) = file.window_width {
                config.window_width = width;
            }
            if let Some(height) = file.window_height {
                config.window_height = height;
            }
        }
        Ok(config)
    }

    fn from_values(
        account_id: Option<String>,
        password: Option<String>,
        character_index: Option<String>,
        gateway_url: Option<String>,
        default_gateway_url: &str,
    ) -> Result<Self, String> {
        let account_id = optional_non_empty(account_id, ACCOUNT_ENV)?;
        let password = optional_non_empty(password, PASSWORD_ENV)?;

        let auto_login = match (account_id, password) {
            (None, None) => {
                if character_index.is_some() {
                    return Err(format!(
                        "{CHARACTER_INDEX_ENV} requires both {ACCOUNT_ENV} and {PASSWORD_ENV}"
                    ));
                }
                None
            }
            (Some(account_id), Some(password)) => {
                let character_index = character_index
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| {
                        value.parse::<i32>().map_err(|_| {
                            format!("{CHARACTER_INDEX_ENV} must be a non-negative integer")
                        })
                    })
                    .transpose()?;
                if character_index.is_some_and(|index| index < 0) {
                    return Err(format!(
                        "{CHARACTER_INDEX_ENV} must be a non-negative integer"
                    ));
                }
                Some(NativeAutoLogin {
                    account_id,
                    password,
                    character_index,
                })
            }
            _ => {
                return Err(format!(
                    "{ACCOUNT_ENV} and {PASSWORD_ENV} must either both be set or both be omitted"
                ));
            }
        };

        let gateway_url = gateway_url
            .as_deref()
            .unwrap_or(default_gateway_url)
            .trim()
            .to_owned();
        validate_gateway_url(&gateway_url)?;

        Ok(Self {
            gateway_url,
            auto_login,
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            reconnect: NativeReconnectConfig::default(),
        })
    }
}

fn read_client_file() -> Result<Option<ClientFileConfig>, String> {
    for path in config_file_candidates() {
        if path.is_file() {
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            return parse_client_toml(&text)
                .map(Some)
                .map_err(|error| format!("{}: {error}", path.display()));
        }
    }
    Ok(None)
}

fn config_file_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(bin_dir) = executable.parent() {
            candidates.push(bin_dir.join(CONFIG_FILE_NAME));
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        let cwd_path = current_dir.join(CONFIG_FILE_NAME);
        if !candidates.iter().any(|path| path == &cwd_path) {
            candidates.push(cwd_path);
        }
    }
    candidates
}

fn parse_client_toml(text: &str) -> Result<ClientFileConfig, String> {
    let value: toml::Value = text
        .parse()
        .map_err(|error| format!("{CONFIG_FILE_NAME} is not valid TOML: {error}"))?;
    let Some(root) = value.as_table() else {
        return Err(format!("{CONFIG_FILE_NAME} must be a TOML table"));
    };
    reject_forbidden_keys(root, "")?;

    let mut config = ClientFileConfig::default();
    for (key, child) in root {
        match key.as_str() {
            "server" => {
                let table = child
                    .as_table()
                    .ok_or_else(|| format!("{CONFIG_FILE_NAME} [server] must be a table"))?;
                reject_forbidden_keys(table, "server.")?;
                for (server_key, server_value) in table {
                    match server_key.as_str() {
                        "gateway_ws_url" => {
                            config.gateway_ws_url =
                                Some(parse_toml_string(server_value, "server.gateway_ws_url")?);
                        }
                        other => {
                            return Err(format!("{CONFIG_FILE_NAME} unknown key server.{other}"));
                        }
                    }
                }
            }
            "display" => {
                let table = child
                    .as_table()
                    .ok_or_else(|| format!("{CONFIG_FILE_NAME} [display] must be a table"))?;
                reject_forbidden_keys(table, "display.")?;
                for (display_key, display_value) in table {
                    match display_key.as_str() {
                        "width" => {
                            config.window_width =
                                Some(parse_toml_u32(display_value, "display.width")?);
                        }
                        "height" => {
                            config.window_height =
                                Some(parse_toml_u32(display_value, "display.height")?);
                        }
                        other => {
                            return Err(format!("{CONFIG_FILE_NAME} unknown key display.{other}"));
                        }
                    }
                }
            }
            other => {
                return Err(format!(
                    "{CONFIG_FILE_NAME} unknown section [{other}]; only [server] and [display] are allowed"
                ));
            }
        }
    }

    if config.window_width == Some(0) || config.window_height == Some(0) {
        return Err(format!(
            "{CONFIG_FILE_NAME} display width and height must be positive"
        ));
    }
    Ok(config)
}

fn reject_forbidden_keys(
    table: &toml::map::Map<String, toml::Value>,
    prefix: &str,
) -> Result<(), String> {
    for key in table.keys() {
        let normalized = key.to_ascii_lowercase().replace('-', "_");
        if FORBIDDEN_CONFIG_KEYS
            .iter()
            .any(|forbidden| normalized.contains(*forbidden))
        {
            return Err(format!(
                "{CONFIG_FILE_NAME} must not contain credentials ({prefix}{key})"
            ));
        }
    }
    Ok(())
}

fn parse_toml_string(value: &toml::Value, key: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{CONFIG_FILE_NAME} {key} must be a non-empty string"))
}

fn parse_toml_u32(value: &toml::Value, key: &str) -> Result<u32, String> {
    match value.as_integer() {
        Some(number) if number > 0 && number <= i64::from(u32::MAX) => Ok(number as u32),
        _ => Err(format!(
            "{CONFIG_FILE_NAME} {key} must be a positive integer"
        )),
    }
}

fn validate_gateway_url(gateway_url: &str) -> Result<(), String> {
    let uri = gateway_url
        .parse::<Uri>()
        .map_err(|_| "gateway URL must be a valid WebSocket URL".to_owned())?;
    let authority = uri
        .authority()
        .ok_or_else(|| "gateway URL must include a host".to_owned())?;
    if authority.as_str().contains('@') {
        return Err("gateway URL must not contain credentials".to_owned());
    }

    let host = uri
        .host()
        .ok_or_else(|| "gateway URL must include a host".to_owned())?
        .trim_matches(['[', ']']);
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());

    match uri.scheme_str() {
        Some("wss") => Ok(()),
        Some("ws") if is_loopback => Ok(()),
        Some("ws") => Err(
            "gateway URL must use wss:// (ws:// is allowed only for loopback development)"
                .to_owned(),
        ),
        _ => Err("gateway URL must use ws:// or wss://".to_owned()),
    }
}

fn optional_non_empty(value: Option<String>, name: &str) -> Result<Option<String>, String> {
    value
        .map(|value| {
            let value = value.trim();
            if value.is_empty() {
                Err(format!("{name} must not be empty when set"))
            } else {
                Ok(value.to_owned())
            }
        })
        .transpose()
}

fn bounded_env_u64(name: &str, default: u64, minimum: u64, maximum: u64) -> Result<u64, String> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(default);
    };
    let value = value
        .to_string_lossy()
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("{name} must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name} must be between {minimum} and {maximum}"));
    }
    Ok(value)
}

fn duration_env(
    name: &str,
    default: Duration,
    minimum_ms: u64,
    maximum_ms: u64,
) -> Result<Duration, String> {
    Ok(Duration::from_millis(bounded_env_u64(
        name,
        default.as_millis() as u64,
        minimum_ms,
        maximum_ms,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credentials_open_the_interactive_login_flow() {
        let config =
            NativeSessionConfig::from_values(None, None, None, None, "ws://127.0.0.1:7110/ws")
                .expect("the visible login screen must not require environment credentials");

        assert_eq!(config.gateway_url, "ws://127.0.0.1:7110/ws");
        assert_eq!(config.auto_login, None);
        assert_eq!(config.window_width, 1024);
        assert_eq!(config.window_height, 768);
    }

    #[test]
    fn explicit_credentials_enable_opt_in_auto_login_without_auto_start() {
        let config = NativeSessionConfig::from_values(
            Some("player-one".to_owned()),
            Some("secret".to_owned()),
            None,
            None,
            "ws://127.0.0.1:7110/ws",
        )
        .expect("explicit credentials should be valid");

        assert_eq!(config.gateway_url, "ws://127.0.0.1:7110/ws");
        assert_eq!(
            config.auto_login,
            Some(NativeAutoLogin {
                account_id: "player-one".to_owned(),
                password: "secret".to_owned(),
                character_index: None,
            })
        );
    }

    #[test]
    fn explicit_character_index_is_only_an_auto_login_option() {
        let config = NativeSessionConfig::from_values(
            Some("player".to_owned()),
            Some("secret".to_owned()),
            Some("2".to_owned()),
            None,
            "ws://127.0.0.1:7110/ws",
        )
        .expect("explicit development auto start should parse");

        assert_eq!(
            config.auto_login.expect("auto login").character_index,
            Some(2)
        );
        assert!(NativeSessionConfig::from_values(
            None,
            None,
            Some("0".to_owned()),
            None,
            "ws://127.0.0.1:7110/ws",
        )
        .expect_err("a character index without credentials is ambiguous")
        .contains(CHARACTER_INDEX_ENV));
    }

    #[test]
    fn partial_or_empty_credentials_are_rejected() {
        for (account, password) in [
            (Some("player".to_owned()), None),
            (None, Some("secret".to_owned())),
            (Some(" ".to_owned()), Some("secret".to_owned())),
            (Some("player".to_owned()), Some(" ".to_owned())),
        ] {
            assert!(NativeSessionConfig::from_values(
                account,
                password,
                None,
                None,
                "ws://127.0.0.1:7110/ws",
            )
            .is_err());
        }
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
    fn debug_output_redacts_the_auto_login_password() {
        let config = NativeSessionConfig::from_values(
            Some("player".to_owned()),
            Some("super-secret".to_owned()),
            Some("0".to_owned()),
            None,
            "ws://127.0.0.1:7110/ws",
        )
        .expect("config");

        let debug = format!("{config:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("super-secret"));
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

    #[test]
    fn client_toml_accepts_wss_and_display_size() {
        let parsed = parse_client_toml(
            r#"
[server]
gateway_ws_url = "wss://candidate-gateway.example/ws"

[display]
width = 1024
height = 768
"#,
        )
        .expect("valid candidate config");
        assert_eq!(
            parsed.gateway_ws_url.as_deref(),
            Some("wss://candidate-gateway.example/ws")
        );
        assert_eq!(parsed.window_width, Some(1024));
        assert_eq!(parsed.window_height, Some(768));
        validate_gateway_url(parsed.gateway_ws_url.as_deref().unwrap()).unwrap();
    }

    #[test]
    fn client_toml_rejects_non_loopback_ws() {
        let parsed = parse_client_toml(
            r#"
[server]
gateway_ws_url = "ws://gateway.example.com/ws"
"#,
        )
        .expect("toml itself is structurally valid");
        assert!(validate_gateway_url(parsed.gateway_ws_url.as_deref().unwrap()).is_err());
    }

    #[test]
    fn client_toml_rejects_credentials_and_unknown_sections() {
        let password = parse_client_toml(
            r#"
[server]
gateway_ws_url = "wss://candidate-gateway.example/ws"
password = "nope"
"#,
        )
        .expect_err("password must be rejected");
        assert!(password.contains("credentials"));

        let unknown = parse_client_toml(
            r#"
[account]
id = "player"
"#,
        )
        .expect_err("account section must be rejected");
        assert!(unknown.contains("credentials") || unknown.contains("unknown"));
    }
}

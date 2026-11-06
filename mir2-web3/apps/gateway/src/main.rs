use std::env;
use std::io;
use std::path::PathBuf;

use mir2_gateway::tcp::chat_broadcast::ChatBroadcastHub;
use mir2_gateway::tcp::run_tcp_gateway;
use mir2_gateway::web::run_web_gateway;
use mir2_gateway::GatewayConfig;

const DEFAULT_TCP_ADDR: &str = "127.0.0.1:7000";
const DEFAULT_WEB_ADDR: &str = "127.0.0.1:7010";
const DEFAULT_ACCOUNT_STORE_PATH: &str = ".mir2-data/accounts.json";

fn main() -> std::io::Result<()> {
    let worker_threads = tokio_worker_threads_from_env();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .build()?
        .block_on(async_main())
}

async fn async_main() -> std::io::Result<()> {
    mir2_gateway::gate15::initialize_from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    let tcp_addr =
        env::var("MIR2_GATEWAY_TCP_ADDR").unwrap_or_else(|_| DEFAULT_TCP_ADDR.to_string());
    let web_addr =
        env::var("MIR2_GATEWAY_WEB_ADDR").unwrap_or_else(|_| DEFAULT_WEB_ADDR.to_string());
    let account_store_path =
        env::var("MIR2_ACCOUNT_STORE_PATH").unwrap_or_else(|_| DEFAULT_ACCOUNT_STORE_PATH.into());
    let config = GatewayConfig::default().with_crystal_world_runtime();
    let config = match env::var("MIR2_CONTENT_PROFILE")
        .unwrap_or_else(|_| "platinum_176".to_string())
        .as_str()
    {
        "platinum_176" => config.with_platinum_176_profile(),
        "crystal_full" => config,
        profile => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unsupported MIR2_CONTENT_PROFILE {profile}; expected platinum_176 or crystal_full"
                ),
            ));
        }
    };
    let config = config
        .with_account_store_environment(PathBuf::from(account_store_path))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let config = match env::var("MIR2_SAVE_RECOVERY_DIR") {
        Ok(path) if !path.trim().is_empty() => {
            config.with_save_recovery_dir(PathBuf::from(path.trim()))
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MIR2_SAVE_RECOVERY_DIR must not be empty when configured",
            ));
        }
        Err(env::VarError::NotPresent) => config,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MIR2_SAVE_RECOVERY_DIR must be valid Unicode",
            ));
        }
    };
    let encoded_recovery_key = match env::var("MIR2_SAVE_RECOVERY_MAC_KEY") {
        Ok(encoded) => Some(encoded),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MIR2_SAVE_RECOVERY_MAC_KEY must be valid Unicode hexadecimal",
            ));
        }
    };
    let config = configure_save_recovery_mac_key(config, encoded_recovery_key.as_deref())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let chat_hub = ChatBroadcastHub::from_env()?;
    let _chat_broadcast_task = chat_hub.spawn();

    tokio::try_join!(
        run_tcp_gateway(&tcp_addr, config.clone(), chat_hub.clone()),
        run_web_gateway(&web_addr, config, chat_hub),
    )?;

    Ok(())
}

fn configure_save_recovery_mac_key(
    config: GatewayConfig,
    encoded: Option<&str>,
) -> Result<GatewayConfig, String> {
    if !config.recovery_journal_enabled() {
        return Ok(config);
    }
    let encoded = encoded.ok_or_else(|| {
        "MIR2_SAVE_RECOVERY_MAC_KEY is required when save recovery is enabled".to_string()
    })?;
    let key = decode_recovery_mac_key(encoded.trim())?;
    config.with_save_recovery_mac_key(key)
}

fn decode_recovery_mac_key(encoded: &str) -> Result<[u8; 32], String> {
    if encoded.len() != 64 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(
            "MIR2_SAVE_RECOVERY_MAC_KEY must contain exactly 64 hexadecimal characters".to_string(),
        );
    }
    let mut key = [0u8; 32];
    for (index, output) in key.iter_mut().enumerate() {
        let offset = index * 2;
        *output = u8::from_str_radix(&encoded[offset..offset + 2], 16)
            .map_err(|_| "MIR2_SAVE_RECOVERY_MAC_KEY contains invalid hexadecimal".to_string())?;
    }
    Ok(key)
}

fn tokio_worker_threads_from_env() -> usize {
    env::var("MIR2_GATEWAY_TOKIO_WORKER_THREADS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(4)
        })
        .clamp(1, 64)
}

#[cfg(test)]
mod tests {
    use super::{configure_save_recovery_mac_key, decode_recovery_mac_key};
    use mir2_gateway::GatewayConfig;

    const STRONG_KEY: &str = "102132435465768798a9bacbdcedfe0f112233445566778899aabbccddeef102";

    #[test]
    fn file_postgres_and_explicit_directory_require_the_same_external_key_contract() {
        let file = GatewayConfig::default().with_account_store_path("accounts.json");
        let mut postgres = GatewayConfig::default();
        postgres.account_store_database_url =
            Some("postgres://not-opened.invalid/mir2".to_string());
        let explicit = GatewayConfig::default().with_save_recovery_dir("recovery-owned");

        for config in [file, postgres, explicit] {
            let missing = configure_save_recovery_mac_key(config.clone(), None).unwrap_err();
            assert!(missing.contains("is required"));
            let configured = configure_save_recovery_mac_key(config, Some(STRONG_KEY)).unwrap();
            assert_eq!(
                configured.save_recovery_mac_key(),
                Some(&decode_recovery_mac_key(STRONG_KEY).unwrap())
            );
        }
    }

    #[test]
    fn empty_malformed_and_weak_recovery_keys_are_rejected() {
        let config = GatewayConfig::default().with_account_store_path("accounts.json");
        for invalid in ["", "abcd", "zzzz"] {
            let error = configure_save_recovery_mac_key(config.clone(), Some(invalid)).unwrap_err();
            assert!(error.contains("64 hexadecimal"));
        }
        let weak = "00".repeat(32);
        let error = configure_save_recovery_mac_key(config, Some(&weak)).unwrap_err();
        assert!(error.contains("minimum diversity"));
    }

    #[test]
    fn disabled_recovery_does_not_invent_or_require_a_key() {
        let config = configure_save_recovery_mac_key(GatewayConfig::default(), None).unwrap();
        assert!(config.save_recovery_mac_key().is_none());
    }

    #[test]
    fn identical_external_key_is_stable_across_independent_config_builds() {
        let first = configure_save_recovery_mac_key(
            GatewayConfig::default().with_account_store_path("first.json"),
            Some(STRONG_KEY),
        )
        .unwrap();
        let second = configure_save_recovery_mac_key(
            GatewayConfig::default().with_account_store_path("second.json"),
            Some(STRONG_KEY),
        )
        .unwrap();
        assert_eq!(
            first.save_recovery_mac_key(),
            second.save_recovery_mac_key()
        );
    }
}

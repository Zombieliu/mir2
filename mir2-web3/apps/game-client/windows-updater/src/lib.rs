//! Signed per-file native updating; no commands or URLs are taken from the game server.
pub mod fs_safe;
pub mod model;
#[cfg(test)]
mod model_regression;
pub mod platform;
pub mod transaction;
pub mod ui;
pub mod update;

pub const BOOTSTRAP_VERSION: u32 = 1;
pub const ENGINE_VERSION: u32 = 1;
pub const FEED_URL: &str = "https://165.154.65.136.sslip.io/client-updates/latest.json";
/// SHA256 of the RSA public-key DER (not a mutable certificate thumbprint).
pub const SIGNING_KEY: &str = "6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E";

pub fn locale() -> String {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        for name in ["locale.json", "locale-seed.json"] {
            let path = std::path::PathBuf::from(&base).join("mir2-web3").join(name);
            if let Ok(bytes) = fs_safe::read_bounded(&path, 256) {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if value["schema"] == 1 {
                        if let Some(locale) = value["locale"].as_str() {
                            if ["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"]
                                .contains(&locale)
                            {
                                return locale.to_owned();
                            }
                        }
                    }
                }
            }
        }
    }
    platform::os_locale()
}

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
pub const CDN_FEED_URL: &str = "https://assets.mir2.obelisk.build/client-updates/latest.json";
/// SHA256 of the RSA public-key DER (not a mutable certificate thumbprint).
pub const SIGNING_KEY: &str = "6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E";

pub fn locale() -> String {
    locale_from_preferences(
        [
            std::env::var_os("APPDATA"),
            std::env::var_os("LOCALAPPDATA"),
        ]
        .into_iter()
        .flatten()
        .map(std::path::PathBuf::from),
    )
    .unwrap_or_else(platform::os_locale)
}

fn locale_from_preferences(bases: impl IntoIterator<Item = std::path::PathBuf>) -> Option<String> {
    // Match the installer's and game's roaming preference first. Local is
    // retained only for older launcher preferences, never ahead of the game.
    for base in bases {
        for name in ["locale.json", "locale-seed.json"] {
            let path = std::path::PathBuf::from(&base).join("mir2-web3").join(name);
            if let Ok(bytes) = fs_safe::read_bounded(&path, 256) {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if value["schema"] == 1 {
                        if let Some(locale) = value["locale"].as_str() {
                            if ["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"]
                                .contains(&locale)
                            {
                                return Some(locale.to_owned());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod locale_tests {
    use super::*;

    #[test]
    fn game_roaming_locale_overrules_legacy_local_and_corrupt_preferences_fall_back() {
        let root = std::env::temp_dir().join(format!(
            "mir2-updater-locale-{}-{}",
            std::process::id(),
            update::now()
        ));
        assert!(!root.exists());
        let roaming = root.join("roaming");
        let local = root.join("local");
        for base in [&roaming, &local] {
            std::fs::create_dir_all(base.join("mir2-web3")).unwrap();
        }
        let preference = roaming.join("mir2-web3/locale.json");
        std::fs::write(&preference, br#"{"schema":1,"locale":"zh-TW"}"#).unwrap();
        std::fs::write(
            local.join("mir2-web3/locale.json"),
            br#"{"schema":1,"locale":"pt-BR"}"#,
        )
        .unwrap();
        assert_eq!(
            locale_from_preferences([roaming.clone(), local.clone()]).as_deref(),
            Some("zh-TW")
        );
        std::fs::write(&preference, b"broken").unwrap();
        assert_eq!(
            locale_from_preferences([roaming.clone(), local.clone()]).as_deref(),
            Some("pt-BR")
        );
        std::fs::write(
            roaming.join("mir2-web3/locale-seed.json"),
            br#"{"schema":1,"locale":"ar"}"#,
        )
        .unwrap();
        assert_eq!(
            locale_from_preferences([roaming, local]).as_deref(),
            Some("ar")
        );
        // Retain this small owned fixture; no recursive cleanup of user paths.
    }
}

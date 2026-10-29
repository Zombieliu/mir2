//! Signed per-file native updating; no commands or URLs are taken from the game server.
#[cfg(test)]
mod five_class_manifest_tests;
pub mod fs_safe;
mod local_io;
pub mod model;
#[cfg(test)]
mod model_regression;
pub mod platform;
pub mod transaction;
pub mod ui;
pub mod update;

pub const BOOTSTRAP_VERSION: u32 = 1;
pub const ENGINE_VERSION: u32 = 1;
// The original directory remains a signed, old-engine-readable upgrade bridge.
// This engine reads its separate channel after normal verified activation.
pub const FEED_URL: &str = "https://165.154.65.136.sslip.io/client-updates/five-class/latest.json";
pub const CDN_FEED_URL: &str =
    "https://assets.mir2.obelisk.build/client-updates/five-class/latest.json";
/// SHA256 of the RSA public-key DER (not a mutable certificate thumbprint).
pub const SIGNING_KEY: &str = "6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E";

// Existing UI indices stay stable; Simplified Chinese is appended as index 9.
pub(crate) const SUPPORTED_LOCALES: [&str; 10] = [
    "en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar", "zh-CN",
];

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalePreference {
    schema: u32,
    locale: String,
}

fn parse_locale_preference(bytes: &[u8]) -> Option<String> {
    if bytes.len() > 256 {
        return None;
    }
    let preference: LocalePreference = serde_json::from_slice(bytes).ok()?;
    (preference.schema == 1 && SUPPORTED_LOCALES.contains(&preference.locale.as_str()))
        .then_some(preference.locale)
}

/// OS locale names may include region/script subtags. Persisted preferences use
/// the strict canonical whitelist above and never enter this normalization.
pub(crate) fn canonical_os_locale(code: &str) -> &'static str {
    if code.len() > 85 || code.is_empty() {
        return "en";
    }
    let code = code.replace('_', "-").to_ascii_lowercase();
    if code.split('-').any(|part| {
        part.is_empty() || part.len() > 8 || !part.bytes().all(|b| b.is_ascii_alphanumeric())
    }) {
        return "en";
    }
    if matches!(code.as_str(), "zh-tw" | "zh-hk" | "zh-mo" | "zh-hant")
        || code.starts_with("zh-hant-")
    {
        return "zh-TW";
    }
    if matches!(
        code.as_str(),
        "zh" | "zh-cn" | "zh-sg" | "zh-my" | "zh-hans"
    ) || code.starts_with("zh-hans-")
    {
        return "zh-CN";
    }
    match code.split('-').next().unwrap_or("en") {
        "pt" => "pt-BR",
        "ru" => "ru",
        "hi" => "hi",
        "id" => "id",
        "vi" => "vi",
        "th" => "th",
        "ar" => "ar",
        _ => "en",
    }
}

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
    .unwrap_or_else(|| canonical_os_locale(&platform::os_locale()).to_owned())
}

fn locale_from_preferences(bases: impl IntoIterator<Item = std::path::PathBuf>) -> Option<String> {
    // Match the installer's and game's roaming preference first. Local is
    // retained only for older launcher preferences, never ahead of the game.
    for base in bases {
        for name in ["locale.json", "locale-seed.json"] {
            let path = std::path::PathBuf::from(&base).join("mir2-web3").join(name);
            if let Ok(bytes) = fs_safe::read_bounded(&path, 256) {
                if let Some(locale) = parse_locale_preference(&bytes) {
                    return Some(locale);
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
    fn canonical_game_preferences_accept_ten_locales_and_reject_untrusted_variants() {
        for locale in SUPPORTED_LOCALES {
            let bytes =
                serde_json::to_vec(&serde_json::json!({"schema":1,"locale":locale})).unwrap();
            assert_eq!(parse_locale_preference(&bytes).as_deref(), Some(locale));
        }
        for bytes in [
            &br#"{"schema":2,"locale":"zh-CN"}"#[..],
            &br#"{"schema":1,"locale":"zh-XX"}"#[..],
            &br#"{"schema":1,"locale":"zh_CN"}"#[..],
            &br#"{"schema":1,"locale":"zh-Hans"}"#[..],
            &br#"{"schema":1,"locale":"ZH-CN"}"#[..],
            &br#"{"schema":1,"locale":"en-US"}"#[..],
            &br#"{"schema":1,"locale":"zh-CN","command":"ignored"}"#[..],
            &br#"{"schema":1,"locale":"zh-TW","locale":"zh-CN"}"#[..],
            &br#"{"schema":1,"locale":9}"#[..],
            &br#"{"schema":1.0,"locale":"zh-CN"}"#[..],
            &b"broken"[..],
        ] {
            assert_eq!(parse_locale_preference(bytes), None);
        }
        assert_eq!(parse_locale_preference(&[b' '; 257]), None);
    }

    #[test]
    fn operating_system_chinese_scripts_keep_simplified_and_traditional_separate() {
        for code in [
            "zh",
            "zh-CN",
            "zh_SG",
            "zh-MY",
            "zh-Hans",
            "zh-Hans-CN",
            "zh-Hans-TW",
        ] {
            assert_eq!(canonical_os_locale(code), "zh-CN", "{code}");
        }
        for code in [
            "zh-TW",
            "zh_HK",
            "zh-MO",
            "zh-Hant",
            "zh-Hant-HK",
            "zh-Hant-CN",
        ] {
            assert_eq!(canonical_os_locale(code), "zh-TW", "{code}");
        }
        for code in [
            "zh-XX",
            "zh-Hansfake",
            "zh-Hantfake",
            "zh--CN",
            "zh-CN/injected",
            "zh-CN\0",
            "xx-ZZ",
        ] {
            assert_eq!(canonical_os_locale(code), "en", "{code:?}");
        }
        assert_eq!(canonical_os_locale("pt_BR"), "pt-BR");
        assert_eq!(canonical_os_locale("ru-RU"), "ru");
        assert_eq!(canonical_os_locale(&"x".repeat(86)), "en");
    }

    #[test]
    fn simplified_game_preference_and_installer_seed_preserve_existing_precedence() {
        let root = tempfile::tempdir().unwrap();
        let roaming = root.path().join("roaming");
        let local = root.path().join("local");
        for base in [&roaming, &local] {
            std::fs::create_dir_all(base.join("mir2-web3")).unwrap();
        }
        let preference = roaming.join("mir2-web3/locale.json");
        let seed = roaming.join("mir2-web3/locale-seed.json");
        std::fs::write(&preference, br#"{"schema":1,"locale":"zh-CN"}"#).unwrap();
        std::fs::write(&seed, br#"{"schema":1,"locale":"zh-TW"}"#).unwrap();
        std::fs::write(
            local.join("mir2-web3/locale.json"),
            br#"{"schema":1,"locale":"ar"}"#,
        )
        .unwrap();
        let read = || locale_from_preferences([roaming.clone(), local.clone()]);
        assert_eq!(read().as_deref(), Some("zh-CN"));
        std::fs::write(&preference, br#"{"schema":1,"locale":"zh-XX"}"#).unwrap();
        assert_eq!(read().as_deref(), Some("zh-TW"));
        std::fs::write(&seed, br#"{"schema":1,"locale":"zh-CN"}"#).unwrap();
        assert_eq!(read().as_deref(), Some("zh-CN"));
        std::fs::write(&seed, br#"{"schema":1,"locale":"zh-CN","extra":true}"#).unwrap();
        assert_eq!(read().as_deref(), Some("ar"));
    }

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

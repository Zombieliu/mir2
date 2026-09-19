//! Crystal Configs/Setup.ini rules used by creature reward draws.
//! Missing DropRate uses the original Settings.cs default 1F; invalid, zero,
//! negative or non-finite values fail import rather than inventing probabilities.
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrystalCreatureSettings {
    pub source: String,
    pub source_sha256: String,
    #[serde(deserialize_with = "positive_drop_rate")]
    pub drop_rate: f32,
}

fn positive_drop_rate<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if !value.is_finite() || value <= 0.0 {
        return Err(serde::de::Error::custom(
            "DropRate must be finite positive f32",
        ));
    }
    Ok(value)
}

pub fn crystal_creature_settings() -> &'static CrystalCreatureSettings {
    static SETTINGS: OnceLock<CrystalCreatureSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../data/generated/crystal_creature_settings.json"
        ))
        .expect("exported Crystal creature settings must be valid")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creature_drop_rate_matches_current_source() {
        let settings = crystal_creature_settings();
        assert_eq!(settings.drop_rate, 1.0);
        assert_eq!(
            settings.source,
            "Crystal/Build/Server/Debug/Configs/Setup.ini"
        );
        assert_eq!(settings.source_sha256.len(), 64);
        assert!(settings
            .source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn creature_drop_rate_rejects_invalid_f32_values() {
        for rate in ["0", "-1", "1e39", "1e-99", "\"NaN\""] {
            let json = format!(r#"{{"source":"test","sourceSha256":"test","dropRate":{rate}}}"#);
            assert!(
                serde_json::from_str::<CrystalCreatureSettings>(&json).is_err(),
                "{rate}"
            );
        }
    }
}

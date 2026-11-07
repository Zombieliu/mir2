//! Presentation only: canonical Quest chrome plus field-specific legacy Chinese adapters.
use bevy::prelude::Resource;
use mir2_game_data::LanguageCode;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

include!("../../../../packages/game-data/data/generated/quest_presentation_text.rs");

/// Optional display-only opt-in. This never expands LanguageCode or wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDisplayLanguage(pub crate::native_i18n::Locale);

impl Serialize for QuestDisplayLanguage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.code())
    }
}
impl<'de> Deserialize<'de> for QuestDisplayLanguage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        crate::native_i18n::Locale::from_code(&value)
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom("invalid Quest display language"))
    }
}
impl QuestDisplayLanguage {
    pub const fn font_asset(self) -> &'static str {
        use crate::native_i18n::Locale;
        match self.0 {
            Locale::SimplifiedChinese => "original-ui/fonts/NotoSansCJKsc-Regular.otf",
            Locale::TraditionalChinese => "original-ui/fonts/NotoSansTC.ttf",
            Locale::Hindi => "original-ui/fonts/NotoSansDevanagari-Regular.ttf",
            Locale::Thai => "original-ui/fonts/NotoSansThai-Regular.ttf",
            Locale::Arabic => "original-ui/fonts/NotoSansArabic-Regular.ttf",
            _ => "original-ui/fonts/NotoSans-Regular.ttf",
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPresentationLocale(pub LanguageCode);

impl Default for QuestPresentationLocale {
    fn default() -> Self {
        Self(LanguageCode::ChineseSimplified)
    }
}

impl Serialize for QuestPresentationLocale {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.code())
    }
}

impl<'de> Deserialize<'de> for QuestPresentationLocale {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        let language = match code.as_str() {
            "en" => LanguageCode::English,
            "zh-CN" => LanguageCode::ChineseSimplified,
            "es" => LanguageCode::Spanish,
            "pt-BR" => LanguageCode::Portuguese,
            _ => {
                return Err(serde::de::Error::custom(
                    "invalid Quest presentation language",
                ))
            }
        };
        Ok(Self(language))
    }
}

/// Immutable context passed through the selected render/build subtree.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuestRenderText {
    pub locale: QuestPresentationLocale,
}

impl QuestRenderText {
    /// Canonical Native semantic key, with the former four-language path kept
    /// intact when the portable host did not explicitly opt in.
    pub fn keyed(self, key: &str, fallback: &str) -> String {
        if crate::native_i18n::active() {
            crate::native_i18n::key(key, fallback)
        } else {
            self.body(fallback)
        }
    }
    pub fn named(self, key: &str, fallback: &str, args: &[(&str, &str)]) -> String {
        if crate::native_i18n::active() {
            crate::native_i18n::format_key(key, fallback, args)
        } else {
            crate::player_text::format_named(key, fallback, args)
        }
    }
    pub fn is_chinese(self) -> bool {
        if crate::native_i18n::active() {
            return false;
        }
        self.locale.0 == LanguageCode::ChineseSimplified
    }
    pub fn chrome<'a>(self, key: &str, fallback: &'a str) -> &'a str {
        if crate::native_i18n::active() {
            return crate::native_i18n::borrowed_key(key, fallback);
        }
        let index = match self.locale.0 {
            LanguageCode::English => 0,
            LanguageCode::Spanish => 1,
            LanguageCode::Portuguese => 2,
            LanguageCode::ChineseSimplified => 3,
        };
        QUEST_PRESENTATION_TEXT
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .map(|(_, values)| {
                if values[index].is_empty() {
                    values[0]
                } else {
                    values[index]
                }
            })
            .filter(|value| !value.is_empty())
            .unwrap_or(fallback)
    }
    pub fn title(self, quest_index: i32, value: &str) -> String {
        if crate::native_i18n::active() {
            return crate::player_text::quest_title(quest_index, value);
        }
        if self.is_chinese() {
            crate::player_text::quest_title(quest_index, value)
        } else {
            value.to_owned()
        }
    }
    pub fn description(self, quest_index: i32, value: &str) -> String {
        if crate::native_i18n::active() {
            return crate::player_text::quest_description(quest_index, value);
        }
        if self.is_chinese() {
            crate::player_text::quest_description(quest_index, value)
        } else {
            value.to_owned()
        }
    }
    pub fn objective(self, value: &str) -> String {
        if crate::native_i18n::active() {
            return crate::player_text::quest_objective(value);
        }
        if self.is_chinese() {
            crate::player_text::quest_objective(value)
        } else {
            value.to_owned()
        }
    }
    pub fn reward_name(self, value: &str) -> String {
        if crate::native_i18n::active() {
            return crate::player_text::name(value);
        }
        if self.is_chinese() {
            crate::player_text::name(value)
        } else {
            value.to_owned()
        }
    }
    pub fn body(self, value: &str) -> String {
        if crate::native_i18n::active() {
            return crate::player_text::text(value);
        }
        if self.is_chinese() {
            crate::player_text::text(value)
        } else {
            value.to_owned()
        }
    }
    pub fn format(self, key: &str, fallback: &str, value: impl std::fmt::Display) -> String {
        self.chrome(key, fallback)
            .replace("{0}", &value.to_string())
    }
    /// Unkeyed authored client copy remains outside this slice.
    pub fn legacy_copy(self, value: &str) -> String {
        crate::player_text::text(value)
    }
    pub fn track(self, tracked: bool) -> String {
        let label = self.chrome("ui.questTrack", "Track");
        if tracked {
            format!("✓ {label}")
        } else {
            label.to_owned()
        }
    }
}

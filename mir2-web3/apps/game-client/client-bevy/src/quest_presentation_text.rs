//! Presentation only: canonical Quest chrome plus field-specific legacy Chinese adapters.
use bevy::prelude::Resource;
use mir2_game_data::LanguageCode;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

include!("../../../../packages/game-data/data/generated/quest_presentation_text.rs");

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPresentationLocale(pub LanguageCode);

impl Default for QuestPresentationLocale {
    fn default() -> Self { Self(LanguageCode::ChineseSimplified) }
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
            _ => return Err(serde::de::Error::custom("invalid Quest presentation language")),
        };
        Ok(Self(language))
    }
}

/// Immutable context passed through the selected render/build subtree.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuestRenderText { pub locale: QuestPresentationLocale }

impl QuestRenderText {
    pub fn is_chinese(self) -> bool { self.locale.0 == LanguageCode::ChineseSimplified }
    pub fn chrome<'a>(self, key: &str, fallback: &'a str) -> &'a str {
        let index = match self.locale.0 {
            LanguageCode::English => 0, LanguageCode::Spanish => 1,
            LanguageCode::Portuguese => 2, LanguageCode::ChineseSimplified => 3,
        };
        QUEST_PRESENTATION_TEXT.iter().find(|(candidate, _)| *candidate == key)
            .map(|(_, values)| if values[index].is_empty() { values[0] } else { values[index] })
            .filter(|value| !value.is_empty()).unwrap_or(fallback)
    }
    pub fn title(self, quest_index: i32, value: &str) -> String {
        if self.is_chinese() { crate::player_text::quest_title(quest_index, value) } else { value.to_owned() }
    }
    pub fn description(self, quest_index: i32, value: &str) -> String {
        if self.is_chinese() { crate::player_text::quest_description(quest_index, value) } else { value.to_owned() }
    }
    pub fn objective(self, value: &str) -> String {
        if self.is_chinese() { crate::player_text::quest_objective(value) } else { value.to_owned() }
    }
    pub fn reward_name(self, value: &str) -> String {
        if self.is_chinese() { crate::player_text::name(value) } else { value.to_owned() }
    }
    pub fn body(self, value: &str) -> String {
        if self.is_chinese() { crate::player_text::text(value) } else { value.to_owned() }
    }
    pub fn format(self, key: &str, fallback: &str, value: impl std::fmt::Display) -> String {
        self.chrome(key, fallback).replace("{0}", &value.to_string())
    }
    /// Unkeyed authored client copy remains outside this slice.
    pub fn legacy_copy(self, value: &str) -> String { crate::player_text::text(value) }
    pub fn track(self, tracked: bool) -> String {
        let label = self.chrome("ui.questTrack", "Track");
        if tracked { format!("✓ {label}") } else { label.to_owned() }
    }
}

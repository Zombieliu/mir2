//! Optional, presentation-only quest guidance for the native client.
//!
//! The selected profile is read once when the Bevy resource is initialized.
//! It never changes authoritative quest availability, state, or tracking.

use std::collections::BTreeMap;

use bevy::prelude::Resource;
use serde::Deserialize;

const NEWCOMER_PROFILE_NAME: &str = "newcomer-v1";
const NEWCOMER_PROFILE_JSON: &str =
    include_str!("../../../../config/quest-guidance/newcomer-v1.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestGuidanceCategory {
    Recommended,
    Optional,
    Challenge,
    Deferred,
}

impl QuestGuidanceCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Recommended => "Recommended",
            Self::Optional => "Optional",
            Self::Challenge => "Challenge",
            Self::Deferred => "Deferred",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Recommended => 0,
            Self::Optional => 1,
            Self::Challenge => 2,
            Self::Deferred => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct QuestGuidanceEntry {
    pub id: i32,
    pub category: QuestGuidanceCategory,
    pub order: i32,
    pub hint: String,
}

#[derive(Debug, Deserialize)]
struct QuestGuidanceDocument {
    schema: u32,
    profile: String,
    quests: Vec<QuestGuidanceEntry>,
}

/// Startup-cached native quest presentation profile.
#[derive(Resource, Debug, Clone)]
pub struct QuestGuidance {
    profile: Option<String>,
    entries: BTreeMap<i32, QuestGuidanceEntry>,
}

impl QuestGuidance {
    fn disabled() -> Self {
        Self {
            profile: None,
            entries: BTreeMap::new(),
        }
    }

    pub fn from_environment() -> Self {
        let requested = std::env::var("MIR2_QUEST_GUIDANCE").unwrap_or_default();
        Self::from_profile_name(requested.trim())
    }

    pub fn from_profile_name(profile: &str) -> Self {
        if !profile.eq_ignore_ascii_case(NEWCOMER_PROFILE_NAME) {
            return Self::disabled();
        }

        let document: QuestGuidanceDocument = serde_json::from_str(NEWCOMER_PROFILE_JSON)
            .expect("bundled newcomer quest guidance must be valid JSON");
        assert_eq!(document.schema, 1, "unsupported quest guidance schema");
        assert_eq!(
            document.profile, NEWCOMER_PROFILE_NAME,
            "bundled quest guidance profile mismatch"
        );

        let mut entries = BTreeMap::new();
        for entry in document.quests {
            assert!(
                entries.insert(entry.id, entry).is_none(),
                "bundled quest guidance contains a duplicate quest id"
            );
        }
        Self {
            profile: Some(document.profile),
            entries,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.profile.is_some()
    }

    pub fn entry(&self, quest_id: i32) -> Option<&QuestGuidanceEntry> {
        self.entries.get(&quest_id)
    }

    /// Sort configured quests by category/order and retain server order for ties
    /// and tasks outside the optional profile.
    pub fn sort_key(&self, quest_id: i32, server_position: usize) -> (u8, i32, usize) {
        self.entry(quest_id)
            .map(|entry| (entry.category.rank(), entry.order, server_position))
            .unwrap_or((u8::MAX, i32::MAX, server_position))
    }
}

impl Default for QuestGuidance {
    fn default() -> Self {
        Self::from_environment()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystal_mode_is_the_default_and_unknown_values_are_ignored() {
        assert!(!QuestGuidance::from_profile_name("").is_enabled());
        assert!(!QuestGuidance::from_profile_name("crystal").is_enabled());
    }

    #[test]
    fn newcomer_profile_loads_unique_classified_entries() {
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        assert!(guidance.is_enabled());
        assert_eq!(guidance.entries.len(), 144);
        assert!(guidance
            .entries
            .values()
            .all(|entry| !entry.hint.trim().is_empty()));
    }

    #[test]
    fn configured_order_precedes_unconfigured_server_order() {
        let guidance = QuestGuidance::from_profile_name("newcomer-v1");
        let configured = guidance.entries.values().next().unwrap();
        assert!(guidance.sort_key(configured.id, 9) < guidance.sort_key(i32::MAX, 0));
        assert!(guidance.sort_key(i32::MAX, 2) < guidance.sort_key(i32::MAX - 1, 3));
    }
}

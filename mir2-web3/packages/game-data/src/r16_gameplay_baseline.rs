//! Shared defaults for the R16 cross-platform gameplay presentation.
//!
//! This is embedded public build data, not a character-save migration or an
//! instruction to replace an explicitly selected historical profile.

use std::sync::OnceLock;

use serde::Deserialize;

const CANONICAL_JSON: &str = include_str!("../data/r16-gameplay-baseline.json");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R16GameplayBaseline {
    pub schema: u32,
    pub id: String,
    pub content_profile: String,
    pub quest_guidance: String,
    pub quest_cadence: String,
    pub onboarding: String,
}

impl R16GameplayBaseline {
    fn validate(&self) -> Result<(), &'static str> {
        if self.schema != 1 || self.id != "r16-platinum-fiveclass-newcomer-v2" {
            return Err("unsupported R16 baseline identity");
        }
        if self.content_profile != "platinum_fiveclass"
            || self.quest_guidance != "newcomer-v2"
            || self.quest_cadence != self.quest_guidance
            || self.onboarding != "native"
        {
            return Err("inconsistent R16 gameplay baseline");
        }
        Ok(())
    }
}

fn parse(json: &str) -> Result<R16GameplayBaseline, String> {
    let baseline: R16GameplayBaseline =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    baseline.validate().map_err(str::to_owned)?;
    Ok(baseline)
}

/// The single validated baseline shared by native hosts and build tooling.
pub fn r16_gameplay_baseline() -> &'static R16GameplayBaseline {
    static BASELINE: OnceLock<R16GameplayBaseline> = OnceLock::new();
    BASELINE.get_or_init(|| parse(CANONICAL_JSON).expect("invalid canonical R16 gameplay baseline"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_r16_baseline_is_loaded_once_without_runtime_environment() {
        let baseline = r16_gameplay_baseline();
        assert_eq!(baseline, &parse(CANONICAL_JSON).unwrap());
        assert!(std::ptr::eq(baseline, r16_gameplay_baseline()));
        assert_eq!(baseline.content_profile, "platinum_fiveclass");
        assert_eq!(baseline.quest_guidance, "newcomer-v2");
        assert_eq!(baseline.quest_cadence, baseline.quest_guidance);
        assert_eq!(baseline.onboarding, "native");
    }

    #[test]
    fn invalid_or_incomplete_baseline_never_falls_back_to_another_profile() {
        let canonical: serde_json::Value = serde_json::from_str(CANONICAL_JSON).unwrap();
        for field in [
            "schema",
            "id",
            "contentProfile",
            "questGuidance",
            "questCadence",
            "onboarding",
        ] {
            let mut missing = canonical.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(parse(&missing.to_string()).is_err(), "missing {field}");
        }
        for (field, value) in [
            ("schema", serde_json::json!(2)),
            ("schema", serde_json::json!("1")),
            ("id", serde_json::json!("another-release")),
            ("contentProfile", serde_json::json!("platinum_176")),
            ("questGuidance", serde_json::json!("crystal")),
            ("questCadence", serde_json::json!("newcomer-v1")),
            ("onboarding", serde_json::json!("cards")),
            ("unrecognized", serde_json::json!(true)),
        ] {
            let mut changed = canonical.clone();
            changed[field] = value;
            assert!(parse(&changed.to_string()).is_err(), "changed {field}");
        }
        assert!(parse("{not json}").is_err());
    }

    #[test]
    fn duplicate_baseline_fields_are_rejected() {
        let duplicate = CANONICAL_JSON.replacen("{", r#"{"schema":1,"#, 1);
        assert!(parse(&duplicate).is_err());
    }
}

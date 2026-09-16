//! Original Hero-specific settings. Do not substitute the player base-stat table.
//! Source XP uses 500 Int64 rows with previous-value fallback; JSON stores decimal
//! strings so tooling never rounds them through an IEEE-754 JavaScript number.
use mir2_protocol::{BaseStat, BaseStats, MirClass};
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalHeroSource {
    pub path: String,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalHeroRules {
    pub allow_new: bool,
    pub minimum_level: u8,
    pub maximum_count: u8,
    pub can_create_classes: [bool; 5],
    pub seal_item_name: String,
    pub maximum_seal_count: u16,
    pub max_luck: u8,
    pub fire_ring_spell: String,
    pub heal_ring_spell: String,
    pub blink_spell: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrystalHeroSettings {
    pub sources: Vec<CrystalHeroSource>,
    pub rules: CrystalHeroRules,
    #[serde(deserialize_with = "read_classes")]
    pub classes: Vec<BaseStats>,
    #[serde(with = "decimal_i64_list")]
    pub experience: Vec<i64>,
}
fn read_classes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<BaseStats>, D::Error> {
    let classes = Vec::<BaseStats>::deserialize(d)?;
    let jobs = [
        MirClass::Warrior,
        MirClass::Wizard,
        MirClass::Taoist,
        MirClass::Assassin,
        MirClass::Archer,
    ];
    if classes.len() != 5
        || jobs
            .iter()
            .any(|job| classes.iter().filter(|c| c.job == *job).count() != 1)
        || classes.iter().any(|c| {
            c.stats
                .iter()
                .any(|s| s.formula_type > 3 || !s.gain.is_finite() || !s.gain_rate.is_finite())
        })
    {
        return Err(serde::de::Error::custom(
            "Invalid original Hero class settings",
        ));
    }
    Ok(classes)
}
mod decimal_i64_list {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(values: &[i64], s: S) -> Result<S::Ok, S::Error> {
        values
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<i64>, D::Error> {
        let raw = Vec::<String>::deserialize(d)?;
        if raw.len() != 500 {
            return Err(serde::de::Error::custom(
                "Hero experience requires 500 rows",
            ));
        }
        raw.into_iter()
            .map(|s| {
                s.parse::<i64>().ok().filter(|v| *v >= 0).ok_or_else(|| {
                    serde::de::Error::custom("Invalid nonnegative Hero Int64 experience")
                })
            })
            .collect()
    }
}
pub fn crystal_hero_settings() -> &'static CrystalHeroSettings {
    static SETTINGS: OnceLock<CrystalHeroSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        serde_json::from_str(include_str!("../data/generated/crystal_hero_settings.json"))
            .expect("exported original Hero settings must be valid")
    })
}
impl CrystalHeroSettings {
    pub fn base_stats(&self, class: MirClass) -> &BaseStats {
        self.classes
            .iter()
            .find(|entry| entry.job == class)
            .expect("all five Hero classes imported")
    }
    /// Preserve HeroObject.RefreshMaxExperience's strict level < Count boundary.
    pub fn max_experience(&self, level: u16) -> i64 {
        if level == 0 || usize::from(level) >= self.experience.len() {
            0
        } else {
            self.experience[usize::from(level) - 1]
        }
    }
}
/// Exact Shared/BaseStats.cs BaseStat.Calculate operations use System.Single,
/// including its Gain==0 early return (which bypasses Max).
pub fn calculate_crystal_hero_base_stat(stat: &BaseStat, class: MirClass, level: u16) -> i32 {
    if stat.gain == 0.0 {
        return stat.base;
    }
    let l = f32::from(level);
    let b = stat.base as f32;
    let g = stat.gain;
    let r = stat.gain_rate;
    let value = match stat.formula_type {
        0 => {
            if class == MirClass::Warrior {
                b + (l / g + r + l / 20.0) * l
            } else {
                b + (l / g + r) * l
            }
        }
        1 => match class {
            MirClass::Wizard => b + ((l / g + 2.0) * 2.2 * l) + (l * r),
            MirClass::Taoist => (b + l / g * 2.2 * l) + (l * r),
            _ => b + (l * g) + (l * r),
        },
        2 => b + (l / g) * l,
        _ => b + l / g,
    };
    value.min(if stat.max > 0 {
        stat.max as f32
    } else {
        i32::MAX as f32
    }) as i32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hero_settings_import_five_classes_and_int64_experience_without_rounding() {
        let settings = crystal_hero_settings();
        assert_eq!(settings.sources.len(), 11);
        assert!(settings.sources.iter().all(|s| s.sha256.len() == 64));
        assert_eq!(settings.max_experience(1), 5);
        assert_eq!(settings.experience[499], 45_400_000_000);
        assert_eq!(settings.max_experience(499), 45_300_000_000);
        assert_eq!(settings.max_experience(500), 0);
        let mut json = serde_json::to_value(settings).unwrap();
        json["experience"][0] = "9223372036854775807".into();
        let restored: CrystalHeroSettings = serde_json::from_value(json).unwrap();
        assert_eq!(restored.experience[0], i64::MAX);
    }
    #[test]
    fn hero_base_formulas_use_original_class_stats_and_max_rule() {
        let settings = crystal_hero_settings();
        let hp = settings
            .base_stats(MirClass::Warrior)
            .stats
            .iter()
            .find(|s| s.stat == 12)
            .unwrap();
        assert_eq!(
            calculate_crystal_hero_base_stat(hp, MirClass::Warrior, 1),
            18
        );
        assert_eq!(
            calculate_crystal_hero_base_stat(hp, MirClass::Warrior, 20),
            224
        );
        let mut stat = hp.clone();
        stat.max = 100;
        assert_eq!(
            calculate_crystal_hero_base_stat(&stat, MirClass::Warrior, 20),
            100
        );
        stat.gain = 0.0;
        stat.base = 200;
        assert_eq!(
            calculate_crystal_hero_base_stat(&stat, MirClass::Warrior, 20),
            200
        );
    }
    #[test]
    fn hero_settings_reject_invalid_class_formula_and_experience() {
        let value = serde_json::to_value(crystal_hero_settings()).unwrap();
        let mut invalid = value.clone();
        invalid["classes"][0]["stats"][0]["formula_type"] = 4.into();
        assert!(serde_json::from_value::<CrystalHeroSettings>(invalid).is_err());
        let mut invalid = value.clone();
        invalid["experience"][0] = "9223372036854775808".into();
        assert!(serde_json::from_value::<CrystalHeroSettings>(invalid).is_err());
        let mut invalid = value;
        invalid["classes"].as_array_mut().unwrap().pop();
        assert!(serde_json::from_value::<CrystalHeroSettings>(invalid).is_err());
    }
    #[test]
    fn hero_rules_match_original_creation_item_and_luck_configuration() {
        let settings=crystal_hero_settings();let rules=&settings.rules;
        assert_eq!((rules.minimum_level,rules.maximum_count,rules.maximum_seal_count,rules.max_luck),(22,1,5,10));
        assert!(rules.allow_new);assert_eq!(rules.can_create_classes,[true,true,true,true,false]);
        assert_eq!((&*rules.seal_item_name,&*rules.fire_ring_spell,&*rules.heal_ring_spell,&*rules.blink_spell),("SealedHero","FireBall","Healing","Blink"));
        let mut invalid=serde_json::to_value(settings).unwrap();invalid["rules"]["max_luck"]=256.into();
        assert!(serde_json::from_value::<CrystalHeroSettings>(invalid).is_err());
    }

}

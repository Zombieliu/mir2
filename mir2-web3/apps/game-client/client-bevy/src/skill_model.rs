use bevy::prelude::Resource;
use serde::{Deserialize, Deserializer, Serialize};

/// Maximum number of learned skills and per-skill authoritative metadata kept
/// from one host payload.  The two collections are kept in lock-step so a
/// truncated skill list can never retain metadata for skills that were
/// discarded.
pub const MAX_LEARNED_SKILLS: usize = 512;

/// Server-authoritative learned-skill list consumed by native input.
///
/// `SkillEntry` intentionally keeps its original ABI: the runtime crate and
/// older hosts construct it directly. Richer fields that newer snapshots may
/// carry live in `bindings`, which is a bounded, additive sidecar and is
/// ignored by older producers.
#[derive(Debug, Clone, Default, Resource, Serialize)]
pub struct SkillModel {
    pub skills: Vec<SkillEntry>,
    #[serde(default)]
    pub bindings: Vec<SkillBinding>,
}

impl<'de> Deserialize<'de> for SkillModel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSkillModel::deserialize(deserializer)?;
        // Keep the compatibility sidecar bounded independently of the
        // learned-skill list.  The final vectors are built in learned-skill
        // order below, so an oversized sidecar can never create entries for
        // skills that were discarded by the authoritative-skill bound.
        let raw_bindings = raw
            .bindings
            .into_iter()
            .take(MAX_LEARNED_SKILLS)
            .collect::<Vec<_>>();
        let mut skills = Vec::with_capacity(raw.skills.len().min(MAX_LEARNED_SKILLS));
        let mut bindings = Vec::with_capacity(raw.skills.len().min(MAX_LEARNED_SKILLS));

        for raw_skill in raw.skills.into_iter().take(MAX_LEARNED_SKILLS) {
            let skill = SkillEntry {
                id: raw_skill.id,
                name: raw_skill.name,
                level: raw_skill.level,
                key: raw_skill.key,
                cooldown_ms: raw_skill.cooldown_ms,
                mp_cost: raw_skill.mp_cost.unwrap_or_default(),
            };
            // A top-level binding is an additive compatibility form.  It is
            // authoritative for fields it carries, while fields absent from
            // it remain sourced from the actual known-skill entry.
            let explicit = raw_bindings
                .iter()
                .find(|binding| binding.skill_id == skill.id);
            bindings.push(SkillBinding {
                skill_id: skill.id,
                spell: explicit
                    .and_then(|binding| binding.spell.clone())
                    .or(raw_skill.spell),
                hotkey: explicit
                    .and_then(|binding| binding.hotkey)
                    .or(raw_skill.hotkey),
                cast_kind: explicit
                    .and_then(|binding| binding.cast_kind.clone())
                    .or(raw_skill.cast_kind),
                can_use: explicit
                    .and_then(|binding| binding.can_use)
                    .or(raw_skill.can_use),
                offensive: explicit
                    .and_then(|binding| binding.offensive)
                    .or(raw_skill.offensive),
                cooldown_remaining_ticks: explicit
                    .map(|binding| binding.cooldown_remaining_ticks)
                    .unwrap_or(raw_skill.cooldown_remaining_ticks),
                mp_cost: explicit
                    .and_then(|binding| binding.mp_cost)
                    .or(raw_skill.mp_cost),
                delay_ms: explicit
                    .and_then(|binding| binding.delay_ms)
                    .or(Some(raw_skill.cooldown_ms)),
                cast_time_ms: explicit
                    .and_then(|binding| binding.cast_time_ms)
                    .or(raw_skill.cast_time_ms),
            });
            skills.push(skill);
        }

        debug_assert_eq!(skills.len(), bindings.len());
        Ok(Self { skills, bindings })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillBinding {
    pub skill_id: u32,
    #[serde(default)]
    pub spell: Option<String>,
    #[serde(default)]
    /// Preserve explicit invalid values so they fail closed instead of being
    /// mistaken for an omitted shortcut during learned-order filling.
    pub hotkey: Option<i32>,
    #[serde(default)]
    pub cast_kind: Option<String>,
    /// Authoritative runtime enabled state emitted as `canUse`. This is
    /// independent from the immutable `castKind` skill definition.
    #[serde(default)]
    pub can_use: Option<bool>,
    #[serde(default)]
    pub offensive: Option<bool>,
    #[serde(default)]
    pub cooldown_remaining_ticks: u32,
    #[serde(default)]
    pub mp_cost: Option<u32>,
    #[serde(default)]
    pub delay_ms: Option<u32>,
    #[serde(default)]
    pub cast_time_ms: Option<i64>,
}

impl Default for SkillBinding {
    fn default() -> Self {
        Self {
            skill_id: 0,
            spell: None,
            hotkey: None,
            // A default value must not assign active-cast semantics. Every
            // producer that knows a cast kind must state it explicitly.
            cast_kind: None,
            can_use: None,
            offensive: None,
            cooldown_remaining_ticks: 0,
            mp_cost: None,
            delay_ms: None,
            cast_time_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCastSelection {
    pub skill_id: u32,
    pub skill_key: Option<String>,
    pub spell: Option<String>,
    pub hotkey: Option<i32>,
    pub cast_kind: Option<String>,
    pub can_use: Option<bool>,
    pub offensive: Option<bool>,
    pub cooldown_remaining_ticks: u32,
    pub mp_cost: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawSkillModel {
    #[serde(default)]
    skills: Vec<RawSkillEntry>,
    #[serde(default)]
    bindings: Vec<SkillBinding>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSkillEntry {
    #[serde(default)]
    id: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    level: u8,
    #[serde(default)]
    key: Option<String>,
    #[serde(default, alias = "delayMs", alias = "delay_ms", alias = "cooldownMs")]
    cooldown_ms: u32,
    #[serde(default, alias = "mp_cost")]
    mp_cost: Option<u32>,
    #[serde(default)]
    spell: Option<String>,
    #[serde(default)]
    hotkey: Option<i32>,
    #[serde(default, alias = "cast_kind")]
    cast_kind: Option<String>,
    #[serde(default, alias = "can_use")]
    can_use: Option<bool>,
    #[serde(default)]
    offensive: Option<bool>,
    #[serde(default, alias = "cooldown_remaining_ticks")]
    cooldown_remaining_ticks: u32,
    #[serde(default, alias = "cast_time_ms")]
    cast_time_ms: Option<i64>,
}

impl SkillModel {
    /// Resolve the deterministic F1-F8 mapping.
    ///
    /// Valid explicit hotkeys win first. If two learned entries claim the
    /// same slot, the first entry in authoritative learned order wins and the
    /// later conflicting entry stays unassigned; it is not silently moved to
    /// another slot. Entries without a valid hotkey then fill the remaining
    /// slots in learned order, so an explicitly bound skill can never be
    /// selected a second time by fallback.
    pub fn skill_for_shortcut(&self, slot: u8) -> Option<&SkillEntry> {
        if !(1..=8).contains(&slot) {
            return None;
        }
        let slots = self.shortcut_skill_indices();
        slots[usize::from(slot - 1)].and_then(|index| self.skills.get(index))
    }

    fn shortcut_skill_indices(&self) -> [Option<usize>; 8] {
        let mut slots = [None; 8];
        let mut explicitly_assigned = vec![false; self.skills.len()];

        // First pass: claim each valid explicit slot in learned order. This
        // makes duplicate-hotkey precedence stable and independent of map
        // iteration or later fallback behavior.
        for (index, skill) in self.skills.iter().enumerate() {
            let binding = self.binding_for(skill.id);
            let Some(slot) = valid_shortcut_index(binding.hotkey) else {
                continue;
            };
            if slots[slot].is_none() {
                slots[slot] = Some(index);
                explicitly_assigned[index] = true;
            }
        }

        // Second pass: only entries without an explicit hotkey may fill holes.
        // Duplicate and invalid explicit values are not unbound skills.
        for (index, skill) in self.skills.iter().enumerate() {
            if explicitly_assigned[index] || self.binding_for(skill.id).hotkey.is_some() {
                continue;
            }
            let Some(empty_slot) = slots.iter().position(|slot| slot.is_none()) else {
                break;
            };
            slots[empty_slot] = Some(index);
        }

        slots
    }

    /// Return the selected learned skill plus the optional authoritative
    /// metadata that newer snapshots provide.
    pub fn selection_for_shortcut(&self, slot: u8) -> Option<SkillCastSelection> {
        let skill = self.skill_for_shortcut(slot)?;
        let binding = self.binding_for(skill.id);
        let cast_kind = normalize_cast_kind(binding.cast_kind.clone())?;
        Some(SkillCastSelection {
            skill_id: skill.id,
            skill_key: skill.key.clone(),
            // Never turn a display name into a protocol spell.  The server's
            // `spell` field is the only accepted spell identifier.
            spell: non_empty(binding.spell),
            hotkey: binding.hotkey,
            cast_kind: Some(cast_kind),
            can_use: binding.can_use,
            offensive: binding.offensive,
            cooldown_remaining_ticks: binding.cooldown_remaining_ticks,
            mp_cost: binding
                .mp_cost
                .or_else(|| (skill.mp_cost > 0).then_some(skill.mp_cost)),
        })
    }

    pub fn binding_for(&self, skill_id: u32) -> SkillBinding {
        self.bindings
            .iter()
            .find(|binding| binding.skill_id == skill_id)
            .cloned()
            .unwrap_or_else(|| SkillBinding {
                skill_id,
                ..Default::default()
            })
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn valid_shortcut_index(hotkey: Option<i32>) -> Option<usize> {
    hotkey.and_then(|hotkey| {
        if (1..=8).contains(&hotkey) {
            Some((hotkey - 1) as usize)
        } else {
            None
        }
    })
}

fn normalize_cast_kind(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_ascii_lowercase();
    match value.as_str() {
        "passive" | "toggle" | "self" | "target" | "ground" | "direction" => Some(value),
        // A missing or future server cast kind is not safe to interpret as a
        // target cast.  Returning None makes shortcut selection fail closed.
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: u32,
    pub name: String,
    pub level: u8,
    pub key: Option<String>,
    pub cooldown_ms: u32,
    pub mp_cost: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_server_skill_json_is_backward_compatible_and_selectable() {
        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [
                {
                    "id": 7,
                    "name": "FireBall",
                    "level": 2,
                    "key": "fireball",
                    "delayMs": 1200,
                    "spell": "FireBall",
                    "castKind": "target",
                    "offensive": true,
                    "hotkey": 2,
                    "cooldownRemainingTicks": 0,
                    "mpCost": 5
                }
            ]
        }))
        .expect("rich skill model");

        let selected = model
            .selection_for_shortcut(2)
            .expect("explicit hotkey selection");
        assert_eq!(selected.skill_id, 7);
        assert_eq!(selected.spell.as_deref(), Some("FireBall"));
        assert_eq!(selected.hotkey, Some(2));
        assert_eq!(selected.mp_cost, Some(5));
        assert!(selected.offensive.unwrap());
    }

    #[test]
    fn missing_hotkeys_fall_back_to_learned_order_without_inventing_a_skill() {
        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [{
                "id": 1,
                "name": "Healing",
                "key": "healing",
                "castKind": "TARGET"
            }]
        }))
        .expect("legacy skill model");
        let selected = model.selection_for_shortcut(1).expect("learned skill");
        assert_eq!(selected.spell, None);
        assert_eq!(selected.cast_kind.as_deref(), Some("target"));
        assert!(model.selection_for_shortcut(2).is_none());
        assert!(model.selection_for_shortcut(9).is_none());
    }

    #[test]
    fn name_and_key_only_skill_has_no_authoritative_spell_for_f1() {
        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [{
                "id": 42,
                "name": "Localized display name",
                "key": "display-key",
                "castKind": "target",
                "hotkey": 1
            }]
        }))
        .expect("display-only skill model");

        let selected = model.selection_for_shortcut(1).expect("F1 selection");
        assert_eq!(selected.spell, None);
        assert_eq!(selected.cast_kind.as_deref(), Some("target"));
    }

    #[test]
    fn cooldown_and_mp_metadata_survive_deserialization() {
        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [{
                "id": 3,
                "name": "Lightning",
                "spell": "Lightning",
                "hotkey": 1,
                "castKind": "target",
                "cooldownRemainingTicks": 4,
                "mpCost": 12
            }]
        }))
        .expect("skill availability model");
        let selected = model.selection_for_shortcut(1).unwrap();
        assert_eq!(selected.cooldown_remaining_ticks, 4);
        assert_eq!(selected.mp_cost, Some(12));
    }

    #[test]
    fn explicit_hotkeys_are_resolved_before_unbound_learned_order() {
        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [
                {"id": 10, "spell": "ThirdSlot", "castKind": "target", "hotkey": 3},
                {"id": 11, "spell": "FirstUnbound", "castKind": "target"},
                {"id": 12, "spell": "FirstSlot", "castKind": "target", "hotkey": 1},
                {"id": 13, "spell": "SecondUnbound", "castKind": "target"}
            ]
        }))
        .expect("ordered shortcut fixture");

        assert_eq!(model.skill_for_shortcut(1).map(|skill| skill.id), Some(12));
        assert_eq!(model.skill_for_shortcut(2).map(|skill| skill.id), Some(11));
        assert_eq!(model.skill_for_shortcut(3).map(|skill| skill.id), Some(10));
        assert_eq!(model.skill_for_shortcut(4).map(|skill| skill.id), Some(13));
    }

    #[test]
    fn duplicate_hotkey_uses_first_learned_entry_and_does_not_duplicate_loser() {
        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [
                {"id": 20, "spell": "FirstClaim", "castKind": "target", "hotkey": 2},
                {"id": 21, "spell": "DuplicateClaim", "castKind": "target", "hotkey": 2},
                {"id": 22, "spell": "Unbound", "castKind": "target"}
            ]
        }))
        .expect("duplicate shortcut fixture");

        assert_eq!(model.skill_for_shortcut(1).map(|skill| skill.id), Some(22));
        assert_eq!(model.skill_for_shortcut(2).map(|skill| skill.id), Some(20));
        for slot in 3_u8..=8_u8 {
            assert_ne!(
                model.skill_for_shortcut(slot).map(|skill| skill.id),
                Some(21)
            );
        }
    }

    #[test]
    fn invalid_explicit_hotkeys_fail_closed_instead_of_filling_shortcuts() {
        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [
                {"id": 1, "spell": "InvalidZero", "castKind": "target", "hotkey": 0},
                {"id": 2, "spell": "InvalidNine", "castKind": "target", "hotkey": 9},
                {"id": 3, "spell": "MissingHotkey", "castKind": "target"}
            ]
        }))
        .expect("invalid shortcut fixture");

        assert_eq!(model.skill_for_shortcut(1).map(|skill| skill.id), Some(3));
        assert!(model.skill_for_shortcut(2).is_none());
        assert!(model.skill_for_shortcut(3).is_none());
        assert_eq!(model.binding_for(1).hotkey, Some(0));
        assert_eq!(model.binding_for(2).hotkey, Some(9));
    }

    #[test]
    fn toggle_definition_and_authoritative_enabled_state_remain_independent() {
        let disabled: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [{
                "id": 8,
                "spell": "FlamingSword",
                "castKind": "toggle",
                "canUse": false,
                "hotkey": 1
            }]
        }))
        .expect("disabled toggle model");
        let selection = disabled
            .selection_for_shortcut(1)
            .expect("toggle selection");
        assert_eq!(selection.cast_kind.as_deref(), Some("toggle"));
        assert_eq!(selection.can_use, Some(false));

        let enabled: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": [{
                "id": 8,
                "spell": "FlamingSword",
                "castKind": "toggle",
                "canUse": true,
                "hotkey": 1
            }]
        }))
        .expect("enabled toggle model");
        let selection = enabled.selection_for_shortcut(1).expect("toggle selection");
        assert_eq!(selection.cast_kind.as_deref(), Some("toggle"));
        assert_eq!(selection.can_use, Some(true));
    }

    #[test]
    fn unknown_or_missing_cast_kind_fails_closed() {
        for cast_kind in [None, Some("FutureKind"), Some(" passive ")] {
            let mut skill = serde_json::json!({
                "id": 1,
                "name": "FireBall",
                "spell": "FireBall"
            });
            if let Some(cast_kind) = cast_kind {
                skill["castKind"] = serde_json::json!(cast_kind);
            }
            let model: SkillModel = serde_json::from_value(serde_json::json!({
                "skills": [skill]
            }))
            .expect("cast kind payload");
            if cast_kind == Some(" passive ") {
                assert_eq!(
                    model
                        .selection_for_shortcut(1)
                        .and_then(|selection| selection.cast_kind),
                    Some("passive".to_owned())
                );
            } else {
                assert!(model.selection_for_shortcut(1).is_none());
            }
        }
    }

    #[test]
    fn learned_skills_and_bindings_share_one_deterministic_bound() {
        let skills = (0..(MAX_LEARNED_SKILLS + 17))
            .map(|id| {
                serde_json::json!({
                    "id": id,
                    "name": format!("Skill{id}"),
                    "key": format!("skill-{id}"),
                    "spell": format!("Spell{id}"),
                    "castKind": "target",
                    "hotkey": if id < 8 { id + 1 } else { 0 }
                })
            })
            .collect::<Vec<_>>();
        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": skills
        }))
        .expect("bounded skill payload");

        assert_eq!(model.skills.len(), MAX_LEARNED_SKILLS);
        assert_eq!(model.bindings.len(), MAX_LEARNED_SKILLS);
        assert_eq!(model.skills.first().map(|skill| skill.id), Some(0));
        assert_eq!(model.skills.last().map(|skill| skill.id), Some(511));
        assert_eq!(
            model.bindings.first().map(|binding| binding.skill_id),
            Some(0)
        );
        assert_eq!(
            model.bindings.last().map(|binding| binding.skill_id),
            Some(511)
        );
        assert_eq!(
            model
                .selection_for_shortcut(1)
                .and_then(|selection| selection.spell),
            Some("Spell0".to_owned())
        );
        assert!(model
            .skills
            .iter()
            .all(|skill| skill.id < MAX_LEARNED_SKILLS as u32));
    }

    #[test]
    fn oversized_skill_and_sidecar_payloads_keep_the_same_prefix_and_order() {
        let skills = (0..(MAX_LEARNED_SKILLS + 17))
            .map(|id| {
                serde_json::json!({
                    "id": id,
                    "name": format!("Skill{id}"),
                    "key": format!("skill-{id}"),
                    "spell": format!("Spell{id}"),
                    "castKind": "target"
                })
            })
            .collect::<Vec<_>>();
        let bindings = (0..(MAX_LEARNED_SKILLS + 17))
            .map(|id| {
                serde_json::json!({
                    "skillId": id,
                    "spell": format!("SidecarSpell{id}"),
                    "hotkey": if id < 8 { id + 1 } else { 0 },
                    "castKind": "TARGET"
                })
            })
            .collect::<Vec<_>>();

        let model: SkillModel = serde_json::from_value(serde_json::json!({
            "skills": skills,
            "bindings": bindings
        }))
        .expect("bounded skill and sidecar payload");

        assert_eq!(model.skills.len(), MAX_LEARNED_SKILLS);
        assert_eq!(model.bindings.len(), MAX_LEARNED_SKILLS);
        assert_eq!(model.skills.first().map(|skill| skill.id), Some(0));
        assert_eq!(model.skills.last().map(|skill| skill.id), Some(511));
        assert_eq!(
            model.bindings.first().map(|binding| binding.skill_id),
            Some(0)
        );
        assert_eq!(
            model.bindings.last().map(|binding| binding.skill_id),
            Some(511)
        );
        assert_eq!(
            model
                .selection_for_shortcut(1)
                .and_then(|selection| selection.spell),
            Some("SidecarSpell0".to_owned())
        );
        assert!(model
            .bindings
            .iter()
            .all(|binding| binding.skill_id < MAX_LEARNED_SKILLS as u32));
    }
}

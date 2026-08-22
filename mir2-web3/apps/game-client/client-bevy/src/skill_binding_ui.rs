//! Renderer-neutral Crystal skill-key assignment state.
//!
//! This module deliberately contains no Bevy UI entities and no input or
//! gateway code.  It is the small state machine shared by those adapters:
//! selection is checked against the authoritative learned-skill list,
//! assignment is limited to F1-F8, and merging a local assignment into a
//! [`SkillModel`] changes only `hotkey`.

use bevy::prelude::Resource;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::skill_model::{SkillBinding, SkillModel};

/// Crystal exposes exactly eight assignable function-key slots.
pub const MIN_SKILL_HOTKEY: u8 = 1;
pub const MAX_SKILL_HOTKEY: u8 = 8;
/// The persisted state can never contain more entries than there are slots.
pub const MAX_SKILL_HOTKEY_BINDINGS: usize = 8;

/// One learned skill assigned to one Crystal function-key slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillHotkeyBinding {
    pub skill_id: u32,
    pub hotkey: u8,
}

impl SkillHotkeyBinding {
    pub const fn new(skill_id: u32, hotkey: u8) -> Option<Self> {
        if skill_id == 0 || !valid_hotkey(hotkey) {
            return None;
        }
        Some(Self { skill_id, hotkey })
    }
}

/// Renderer-neutral state for Crystal's AssignKey interaction.
///
/// Only explicit assignments are persisted.  The selected skill and the
/// AssignKey toggle are UI state; the spell name, cast kind, cooldown and all
/// other authoritative skill data remain in [`SkillModel`].
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct SkillBindingUi {
    pub selected_skill_id: Option<u32>,
    pub assign_key: bool,
    pub bindings: Vec<SkillHotkeyBinding>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSkillBindingUi<'a> {
    bindings: &'a [SkillHotkeyBinding],
}

/// Name used by renderer code that treats this as a Bevy resource.
pub type SkillBindingUiState = SkillBindingUi;

impl Default for SkillBindingUi {
    fn default() -> Self {
        Self {
            selected_skill_id: None,
            assign_key: false,
            bindings: Vec::new(),
        }
    }
}

impl Serialize for SkillBindingUi {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bindings = self.sanitized_bindings();
        PersistedSkillBindingUi {
            bindings: &bindings,
        }
        .serialize(serializer)
    }
}

/// Deserialization is intentionally sanitizing rather than trusting disk
/// state. Invalid slots, zero IDs, duplicate skills and duplicate slots are
/// discarded deterministically in file order, and the collection is bounded.
impl<'de> Deserialize<'de> for SkillBindingUi {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawSkillBindingUi {
            #[serde(default)]
            bindings: Vec<RawSkillHotkeyBinding>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawSkillHotkeyBinding {
            #[serde(default)]
            skill_id: u32,
            #[serde(default)]
            hotkey: i32,
        }

        let raw = RawSkillBindingUi::deserialize(deserializer)?;
        let mut state = Self {
            selected_skill_id: None,
            assign_key: false,
            bindings: Vec::with_capacity(MAX_SKILL_HOTKEY_BINDINGS),
        };

        for raw_binding in raw.bindings {
            if state.bindings.len() >= MAX_SKILL_HOTKEY_BINDINGS {
                break;
            }
            let Ok(hotkey) = u8::try_from(raw_binding.hotkey) else {
                continue;
            };
            let Some(binding) = SkillHotkeyBinding::new(raw_binding.skill_id, hotkey) else {
                continue;
            };
            if state.bindings.iter().any(|existing| {
                existing.skill_id == binding.skill_id || existing.hotkey == binding.hotkey
            }) {
                continue;
            }
            state.bindings.push(binding);
        }
        Ok(state)
    }
}

impl SkillBindingUi {
    /// Toggle Crystal's AssignKey mode and return its new value.
    pub fn toggle_assign_key(&mut self) -> bool {
        self.assign_key = !self.assign_key;
        self.assign_key
    }

    pub fn set_assign_key(&mut self, enabled: bool) {
        self.assign_key = enabled;
    }

    pub const fn is_assign_key_enabled(&self) -> bool {
        self.assign_key
    }

    pub const fn selected_skill_id(&self) -> Option<u32> {
        self.selected_skill_id
    }

    pub fn binding_for_skill(&self, skill_id: u32) -> Option<SkillHotkeyBinding> {
        self.bindings.iter().copied().find(|binding| {
            binding.skill_id == skill_id && binding.skill_id != 0 && valid_hotkey(binding.hotkey)
        })
    }

    pub fn skill_for_hotkey(&self, hotkey: u8) -> Option<u32> {
        if !valid_hotkey(hotkey) {
            return None;
        }
        self.bindings
            .iter()
            .find(|binding| {
                binding.hotkey == hotkey && binding.skill_id != 0 && valid_hotkey(binding.hotkey)
            })
            .map(|binding| binding.skill_id)
    }

    /// Select only a skill present in the authoritative learned-skill list.
    /// Unknown IDs fail closed and do not replace an existing valid selection.
    pub fn select_skill(&mut self, skill_id: u32, model: &SkillModel) -> bool {
        if model.skills.iter().any(|skill| skill.id == skill_id) {
            self.selected_skill_id = Some(skill_id);
            true
        } else {
            false
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_skill_id = None;
    }

    /// Refresh against a new authoritative model.
    ///
    /// Stale assignments and a selection for an unlearned skill are removed.
    /// Existing local assignments win on refresh; otherwise a valid server
    /// hotkey is imported once. This makes reconnect/bootstrap deterministic
    /// without allowing a refresh to resurrect an unknown skill.
    pub fn refresh(&mut self, model: &SkillModel) {
        let learned = |skill_id: u32| model.skills.iter().any(|skill| skill.id == skill_id);

        self.bindings.retain(|binding| learned(binding.skill_id));
        self.normalize_bindings();

        if let Some(skill_id) = self.selected_skill_id {
            if !learned(skill_id) {
                self.selected_skill_id = None;
            }
        }

        // SkillModel already bounds the authoritative learned list. Do not
        // inspect only its first eight rows: a later learned skill may carry
        // the explicit F1-F8 binding that must be imported.
        for skill in &model.skills {
            if self.binding_for_skill(skill.id).is_some() {
                continue;
            }
            let server_hotkey = model.binding_for(skill.id).hotkey;
            let Some(hotkey) = server_hotkey.and_then(valid_hotkey_i32) else {
                continue;
            };
            if self.skill_for_hotkey(hotkey).is_none() {
                self.bindings.push(SkillHotkeyBinding {
                    skill_id: skill.id,
                    hotkey,
                });
            }
            if self.bindings.len() >= MAX_SKILL_HOTKEY_BINDINGS {
                break;
            }
        }
    }

    /// Assign the selected learned skill to F1-F8.
    ///
    /// Assignment is accepted only while AssignKey is enabled. A slot has one
    /// owner and a skill has one slot: assigning either side removes its old
    /// owner, which gives Crystal's conflict-rebind behavior.
    pub fn assign_selected_key(&mut self, hotkey: u8, model: &SkillModel) -> bool {
        if !self.assign_key || !valid_hotkey(hotkey) {
            return false;
        }
        let Some(skill_id) = self.selected_skill_id else {
            return false;
        };
        self.assign_key_to_skill(skill_id, hotkey, model)
    }

    /// Explicit form useful to a renderer that keeps selection in its own
    /// transient state. It still requires AssignKey and an authoritative ID.
    pub fn assign_key_to_skill(&mut self, skill_id: u32, hotkey: u8, model: &SkillModel) -> bool {
        if !self.assign_key
            || !valid_hotkey(hotkey)
            || !model.skills.iter().any(|skill| skill.id == skill_id)
        {
            return false;
        }

        self.bindings
            .retain(|binding| binding.skill_id != skill_id && binding.hotkey != hotkey);
        if self.bindings.len() >= MAX_SKILL_HOTKEY_BINDINGS {
            return false;
        }
        self.bindings.push(SkillHotkeyBinding { skill_id, hotkey });
        true
    }

    pub fn unassign_skill(&mut self, skill_id: u32) -> bool {
        let before = self.bindings.len();
        self.bindings.retain(|binding| binding.skill_id != skill_id);
        self.bindings.len() != before
    }

    /// Return a model with local hotkeys merged by learned `skill_id`.
    ///
    /// No skill is invented, no unknown persisted binding is emitted, and all
    /// fields other than `hotkey` are copied unchanged from the input model.
    pub fn merge_skill_model(&self, model: &SkillModel) -> SkillModel {
        let mut merged = model.clone();
        self.apply_to_skill_model(&mut merged);
        merged
    }

    pub fn merged_skill_model(&self, model: &SkillModel) -> SkillModel {
        self.merge_skill_model(model)
    }

    pub fn apply_to_skill_model(&self, model: &mut SkillModel) {
        let local_bindings = self
            .sanitized_bindings()
            .into_iter()
            .filter(|local| model.skills.iter().any(|skill| skill.id == local.skill_id))
            .collect::<Vec<_>>();
        let local_skill_ids = local_bindings
            .iter()
            .map(|local| local.skill_id)
            .collect::<Vec<_>>();
        let local_hotkeys = local_bindings
            .iter()
            .map(|local| local.hotkey)
            .collect::<Vec<_>>();

        // SkillModel resolves duplicate slots in binding order. Clear only
        // the conflicting hotkey, never spell/cast/cooldown metadata, so a
        // local rebind cannot be shadowed by an older server row.
        for binding in &mut model.bindings {
            if local_skill_ids.contains(&binding.skill_id)
                || binding
                    .hotkey
                    .and_then(valid_hotkey_i32)
                    .is_some_and(|hotkey| local_hotkeys.contains(&hotkey))
            {
                binding.hotkey = None;
            }
        }

        // SkillModel normally keeps bindings in lock-step with skills, but
        // older producers may omit the sidecar. Add only the missing known
        // skill row and leave every existing field untouched.
        for local in &local_bindings {
            if model
                .bindings
                .iter()
                .any(|binding| binding.skill_id == local.skill_id)
            {
                for binding in &mut model.bindings {
                    if binding.skill_id == local.skill_id {
                        binding.hotkey = Some(i32::from(local.hotkey));
                    }
                }
            } else {
                model.bindings.push(SkillBinding {
                    skill_id: local.skill_id,
                    hotkey: Some(i32::from(local.hotkey)),
                    ..SkillBinding::default()
                });
            }
        }
    }

    fn normalize_bindings(&mut self) {
        let old = std::mem::take(&mut self.bindings);
        for binding in old {
            if self.bindings.len() >= MAX_SKILL_HOTKEY_BINDINGS
                || !valid_hotkey(binding.hotkey)
                || binding.skill_id == 0
                || self.binding_for_skill(binding.skill_id).is_some()
                || self.skill_for_hotkey(binding.hotkey).is_some()
            {
                continue;
            }
            self.bindings.push(binding);
        }
    }

    fn sanitized_bindings(&self) -> Vec<SkillHotkeyBinding> {
        let mut bindings = Vec::with_capacity(MAX_SKILL_HOTKEY_BINDINGS);
        for binding in self.bindings.iter().copied() {
            if bindings.len() >= MAX_SKILL_HOTKEY_BINDINGS
                || binding.skill_id == 0
                || !valid_hotkey(binding.hotkey)
                || bindings.iter().any(|existing: &SkillHotkeyBinding| {
                    existing.skill_id == binding.skill_id || existing.hotkey == binding.hotkey
                })
            {
                continue;
            }
            bindings.push(binding);
        }
        bindings
    }
}

const fn valid_hotkey(hotkey: u8) -> bool {
    hotkey >= MIN_SKILL_HOTKEY && hotkey <= MAX_SKILL_HOTKEY
}

fn valid_hotkey_i32(hotkey: i32) -> Option<u8> {
    let hotkey = u8::try_from(hotkey).ok()?;
    valid_hotkey(hotkey).then_some(hotkey)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(id: u32, name: &str) -> crate::skill_model::SkillEntry {
        crate::skill_model::SkillEntry {
            id,
            name: name.to_owned(),
            level: 7,
            key: Some(name.to_ascii_lowercase()),
            cooldown_ms: 1200,
            mp_cost: 15,
        }
    }

    fn model(ids: &[u32]) -> SkillModel {
        SkillModel {
            skills: ids
                .iter()
                .map(|id| skill(*id, &format!("Skill{id}")))
                .collect(),
            bindings: ids
                .iter()
                .map(|id| SkillBinding {
                    skill_id: *id,
                    spell: Some(format!("Spell{id}")),
                    cast_kind: Some("target".to_owned()),
                    can_use: Some(true),
                    offensive: Some(true),
                    cooldown_remaining_ticks: 3,
                    mp_cost: Some(15),
                    delay_ms: Some(1200),
                    cast_time_ms: Some(200),
                    ..SkillBinding::default()
                })
                .collect(),
        }
    }

    #[test]
    fn selection_requires_a_learned_skill() {
        let model = model(&[10]);
        let mut ui = SkillBindingUi::default();
        assert!(!ui.select_skill(99, &model));
        assert_eq!(ui.selected_skill_id(), None);
        assert!(ui.select_skill(10, &model));
        assert_eq!(ui.selected_skill_id(), Some(10));
        assert!(!ui.select_skill(99, &model));
        assert_eq!(ui.selected_skill_id(), Some(10));
    }

    #[test]
    fn assignment_is_gated_by_assign_key_and_slot_range() {
        let model = model(&[10]);
        let mut ui = SkillBindingUi::default();
        assert!(ui.select_skill(10, &model));
        assert!(!ui.assign_selected_key(1, &model));
        assert!(ui.toggle_assign_key());
        assert!(!ui.assign_selected_key(0, &model));
        assert!(!ui.assign_selected_key(9, &model));
        assert!(ui.assign_selected_key(1, &model));
        assert_eq!(ui.binding_for_skill(10).unwrap().hotkey, 1);
    }

    #[test]
    fn assigning_an_occupied_slot_rebinds_both_sides_uniquely() {
        let model = model(&[10, 20]);
        let mut ui = SkillBindingUi::default();
        ui.set_assign_key(true);
        assert!(ui.assign_key_to_skill(10, 1, &model));
        assert!(ui.assign_key_to_skill(20, 2, &model));
        assert!(ui.assign_key_to_skill(10, 2, &model));
        assert_eq!(ui.skill_for_hotkey(1), None);
        assert_eq!(ui.skill_for_hotkey(2), Some(10));
        assert_eq!(ui.binding_for_skill(20), None);
        assert_eq!(ui.bindings.len(), 1);
    }

    #[test]
    fn merge_changes_only_hotkey_for_known_skill() {
        let mut source = model(&[10, 20]);
        source.bindings[0].hotkey = Some(7);
        source.bindings[1].hotkey = Some(2);
        let original_second = source.bindings[1].clone();

        let mut ui = SkillBindingUi::default();
        ui.set_assign_key(true);
        assert!(ui.assign_key_to_skill(10, 2, &source));
        let merged = ui.merge_skill_model(&source);

        assert_eq!(merged.bindings[0].hotkey, Some(2));
        assert_eq!(merged.bindings[0].spell, source.bindings[0].spell);
        assert_eq!(merged.bindings[0].cast_kind, source.bindings[0].cast_kind);
        assert_eq!(merged.bindings[0].cooldown_remaining_ticks, 3);
        assert_eq!(merged.bindings[1].hotkey, None);
        assert_eq!(merged.bindings[1].spell, original_second.spell);
        assert_eq!(merged.bindings[1].cast_kind, original_second.cast_kind);
        assert_eq!(
            merged.bindings[1].cooldown_remaining_ticks,
            original_second.cooldown_remaining_ticks
        );
        assert_eq!(merged.skill_for_shortcut(2).map(|skill| skill.id), Some(10));
        assert_eq!(merged.skill_for_shortcut(7), None);
    }

    #[test]
    fn unknown_binding_fails_closed_during_assignment_and_merge() {
        let source = model(&[10]);
        let mut ui = SkillBindingUi::default();
        ui.set_assign_key(true);
        assert!(!ui.assign_key_to_skill(999, 1, &source));
        assert!(ui.bindings.is_empty());

        let mut polluted = ui;
        polluted.bindings.push(SkillHotkeyBinding {
            skill_id: 999,
            hotkey: 1,
        });
        let merged = polluted.merge_skill_model(&source);
        assert_eq!(merged.skills, source.skills);
        assert_eq!(merged.bindings, source.bindings);
    }

    #[test]
    fn refresh_imports_server_keys_and_removes_stale_entries() {
        let first = model(&[10, 20]);
        let mut ui = SkillBindingUi::default();
        ui.set_assign_key(true);
        assert!(ui.select_skill(20, &first));
        assert!(ui.assign_key_to_skill(10, 1, &first));
        ui.bindings.push(SkillHotkeyBinding {
            skill_id: 999,
            hotkey: 8,
        });

        let mut second = model(&[20, 30]);
        second.bindings[0].hotkey = Some(3);
        second.bindings[1].hotkey = Some(99);
        ui.refresh(&second);

        assert_eq!(ui.selected_skill_id(), Some(20));
        assert_eq!(ui.binding_for_skill(10), None);
        assert_eq!(ui.binding_for_skill(999), None);
        assert_eq!(ui.skill_for_hotkey(3), Some(20));
        assert_eq!(ui.binding_for_skill(30), None);

        let third = model(&[30]);
        ui.refresh(&third);
        assert_eq!(ui.selected_skill_id(), None);
        assert!(ui.bindings.is_empty());
    }

    #[test]
    fn refresh_scans_later_learned_skill_for_explicit_server_hotkey() {
        let ids = (100..110).collect::<Vec<_>>();
        let mut source = model(&ids);
        source.bindings[9].hotkey = Some(6);

        let mut ui = SkillBindingUi::default();
        ui.refresh(&source);

        assert_eq!(ui.skill_for_hotkey(6), Some(109));
        assert_eq!(ui.bindings.len(), 1);
    }

    #[test]
    fn serde_round_trip_and_invalid_disk_state_are_bounded() {
        let mut raw = serde_json::json!({
            "selectedSkillId": 10,
            "assignKey": true,
            "bindings": []
        });
        let bindings = raw["bindings"].as_array_mut().unwrap();
        for id in 1..=32 {
            bindings.push(serde_json::json!({ "skillId": id, "hotkey": id }));
        }
        bindings.push(serde_json::json!({ "skillId": 700, "hotkey": -1 }));

        let ui: SkillBindingUi = serde_json::from_value(raw).unwrap();
        assert_eq!(ui.bindings.len(), MAX_SKILL_HOTKEY_BINDINGS);
        assert_eq!(ui.skill_for_hotkey(1), Some(1));
        assert_eq!(ui.skill_for_hotkey(8), Some(8));

        let encoded = serde_json::to_value(&ui).unwrap();
        assert!(encoded.get("selectedSkillId").is_none());
        assert!(encoded.get("assignKey").is_none());
        let decoded: SkillBindingUi = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.selected_skill_id(), None);
        assert!(!decoded.is_assign_key_enabled());
        assert_eq!(decoded.bindings, ui.bindings);
    }

    #[test]
    fn missing_binding_row_is_added_without_touching_known_skill_data() {
        let source = SkillModel {
            skills: vec![skill(10, "FireBall")],
            bindings: Vec::new(),
        };
        let mut ui = SkillBindingUi::default();
        ui.set_assign_key(true);
        assert!(ui.assign_key_to_skill(10, 4, &source));
        let merged = ui.merge_skill_model(&source);
        assert_eq!(merged.bindings.len(), 1);
        assert_eq!(merged.bindings[0].skill_id, 10);
        assert_eq!(merged.bindings[0].hotkey, Some(4));
        assert_eq!(merged.skills, source.skills);
    }
}

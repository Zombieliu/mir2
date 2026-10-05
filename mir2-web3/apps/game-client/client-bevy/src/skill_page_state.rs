//! Shared Spells state. Server keys remain authority; projection is request-scoped.
use crate::skill_model::{SkillModel, SkillModelAuthority};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const REQUEST_BLOCK: u64 = 1 << 22;
pub const MAX_REQUEST_RUN: u64 = (1 << 31) - 1;
pub const MAX_SAFE_ID: u64 = (1 << 53) - 1;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableRequestNamespace {
    run: u64,
    counter: u64,
}
impl PortableRequestNamespace {
    pub fn new(run: u64) -> Option<Self> {
        (1..=MAX_REQUEST_RUN)
            .contains(&run)
            .then_some(Self { run, counter: 0 })
    }
    /// Burn on allocation, including failed queue/send. Never wrap or recycle.
    pub fn allocate(&mut self) -> Option<u64> {
        let counter = self.counter.checked_add(1)?;
        if counter >= REQUEST_BLOCK {
            return None;
        }
        let id = self.run.checked_mul(REQUEST_BLOCK)?.checked_add(counter)?;
        if id > MAX_SAFE_ID {
            return None;
        }
        self.counter = counter;
        Some(id)
    }
    pub fn owns(&self, id: u64) -> bool {
        id <= MAX_SAFE_ID && id / REQUEST_BLOCK == self.run && id % REQUEST_BLOCK != 0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillKeyRequest {
    pub request_id: u64,
    pub spell: String,
    pub key: u8,
    pub old_key: u8,
}
fn same_owner(a: SkillModelAuthority, b: SkillModelAuthority) -> bool {
    a.session_epoch == b.session_epoch && a.player_object_id == b.player_object_id
}
/// Ordinal IDs are not stable identity. Both ID and unique explicit spell must agree.
pub fn exact_skill(skills: &SkillModel, id: u32, spell: &str) -> bool {
    !spell.is_empty()
        && skills.skills.iter().filter(|s| s.id == id).count() == 1
        && skills.bindings.iter().filter(|b| b.skill_id == id).count() == 1
        && skills.binding_for(id).spell.as_deref() == Some(spell)
        && skills
            .skills
            .iter()
            .filter(|s| skills.binding_for(s.id).spell.as_deref() == Some(spell))
            .count()
            == 1
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillAssignUi {
    pub open: bool,
    pub request_id: u64,
    pub skill_id: u32,
    pub key: u8,
    pub old_key: u8,
    pub spell: String,
    pub pending: bool,
    pub result: Option<bool>,
    pub notice: Option<String>,
    pub owner: SkillModelAuthority,
}
impl SkillAssignUi {
    pub fn show(&mut self, id: u32, skills: &SkillModel) {
        // A failed selection retires the previous draft; it never preserves a stale target.
        *self = Self::default();
        let binding = skills.binding_for(id);
        let Some(spell) = binding.spell.filter(|s| exact_skill(skills, id, s)) else {
            return;
        };
        let old_key = binding
            .hotkey
            .and_then(|v| u8::try_from(v).ok())
            .filter(|v| *v <= 16)
            .unwrap_or(0);
        *self = Self {
            open: true,
            skill_id: id,
            key: old_key,
            old_key,
            spell,
            owner: skills.authority,
            ..Self::default()
        };
    }
    pub fn valid(&self, skills: &SkillModel) -> bool {
        self.open
            && same_owner(self.owner, skills.authority)
            && exact_skill(skills, self.skill_id, &self.spell)
    }
    pub fn choose(&mut self, key: u8) {
        if !self.pending && key <= 16 {
            self.key = key;
        }
    }
    pub fn request(&mut self, id: u64, skills: &SkillModel) -> Option<SkillKeyRequest> {
        if !self.valid(skills) || self.pending || id == 0 || self.key > 16 || self.old_key > 16 {
            return None;
        }
        self.request_id = id;
        Some(SkillKeyRequest {
            request_id: id,
            spell: self.spell.clone(),
            key: self.key,
            old_key: self.old_key,
        })
    }
    pub fn queued(&mut self, accepted: bool) {
        self.pending = accepted;
        self.notice = (!accepted).then(|| "Unable to send. Please try again.".into());
    }
    /// A transport callback is not a server receipt. Current-row validation occurs before projection.
    pub fn dispatched(
        &mut self,
        request_id: u64,
        spell: &str,
        key: u8,
        old_key: u8,
        success: bool,
    ) {
        if self.pending
            && self.request_id == request_id
            && self.spell == spell
            && self.key == key
            && self.old_key == old_key
        {
            self.result = Some(success);
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillAuthorityUi {
    pub(crate) pending: Option<SkillKeyPending>,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SkillKeyPending {
    base: SkillModelAuthority,
    draft: SkillAssignUi,
    projection: bool,
}
impl SkillAuthorityUi {
    pub fn begin(&mut self, skills: &SkillModel, draft: SkillAssignUi) {
        self.begin_with_projection(skills, draft, true);
    }
    /// A possibly sent request locks its tuple but cannot project keys before an exact receipt.
    pub fn begin_unknown(&mut self, skills: &SkillModel, draft: SkillAssignUi) {
        self.begin_with_projection(skills, draft, false);
    }
    fn begin_with_projection(
        &mut self,
        skills: &SkillModel,
        draft: SkillAssignUi,
        projection: bool,
    ) {
        if draft.valid(skills) {
            self.pending = Some(SkillKeyPending {
                base: skills.authority,
                draft,
                projection,
            });
        }
    }
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn reconcile(&mut self, skills: &mut SkillModel) -> Option<SkillAssignUi> {
        let p = self.pending.as_ref()?;
        if !same_owner(skills.authority, p.base)
            || !exact_skill(skills, p.draft.skill_id, &p.draft.spell)
        {
            self.pending = None;
            return None;
        }
        if let Some(ack) = skills.skill_key_ack.as_ref().filter(|a| {
            a.request_id == p.draft.request_id
                && a.spell == p.draft.spell
                && a.key == p.draft.key
                && a.old_key == p.draft.old_key
        }) {
            let restore = (!ack.accepted).then(|| {
                let mut d = p.draft.clone();
                d.pending = false;
                d.result = None;
                d.notice = Some("Unable to save this key. Please try again.".into());
                d
            });
            self.pending = None;
            return restore;
        }
        if p.projection {
            apply_source_key(skills, p.draft.skill_id, &p.draft.spell, p.draft.key);
        }
        None
    }
}
pub fn apply_source_key(skills: &mut SkillModel, id: u32, spell: &str, key: u8) -> bool {
    if key > 16 || !exact_skill(skills, id, spell) {
        return false;
    }
    for entry in &mut skills.bindings {
        if entry.skill_id == id || (key > 0 && entry.hotkey == Some(i32::from(key))) {
            entry.hotkey = Some(if entry.skill_id == id {
                i32::from(key)
            } else {
                0
            });
        }
    }
    true
}

/// One monotonic clock, shared by the learned page and existing Native bars.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillCooldownClock {
    owner: Option<(u64, u32)>,
    last_now: u64,
    starts: HashMap<u32, (String, u64, u64)>,
}
impl SkillCooldownClock {
    pub fn observe(&mut self, skills: &SkillModel, now_ms: u64) {
        self.observe_with_starts(skills, now_ms, |_, _, _| None);
    }
    /// Portable timestamps come only from the captured actual cast packet; Native uses now.
    pub fn observe_with_starts(
        &mut self,
        skills: &SkillModel,
        now_ms: u64,
        start_for: impl Fn(u32, &str, u64) -> Option<u64>,
    ) {
        let owner = (
            skills.authority.session_epoch,
            skills.authority.player_object_id,
        );
        if self.owner != Some(owner) || now_ms < self.last_now {
            self.starts.clear();
        }
        self.owner = Some(owner);
        self.last_now = now_ms;
        self.starts
            .retain(|id, (spell, _, _)| exact_skill(skills, *id, spell));
        for b in &skills.bindings {
            let Some(spell) = b
                .spell
                .as_ref()
                .filter(|s| exact_skill(skills, b.skill_id, s))
            else {
                continue;
            };
            if b.cast_sequence != 0
                && self
                    .starts
                    .get(&b.skill_id)
                    .is_none_or(|(_, seq, _)| *seq != b.cast_sequence)
            {
                let start = start_for(b.skill_id, spell, b.cast_sequence)
                    .filter(|at| *at <= now_ms)
                    .unwrap_or(now_ms);
                self.starts
                    .insert(b.skill_id, (spell.clone(), b.cast_sequence, start));
            }
        }
    }
    pub fn remaining_ms(&self, id: u32, skills: &SkillModel, now_ms: u64) -> u32 {
        let Some((spell, _, start)) = self.starts.get(&id) else {
            return 0;
        };
        if self.owner
            != Some((
                skills.authority.session_epoch,
                skills.authority.player_object_id,
            ))
            || !exact_skill(skills, id, spell)
            || now_ms < *start
        {
            return 0;
        }
        skills
            .binding_for(id)
            .delay_ms
            .unwrap_or(0)
            .saturating_sub(now_ms.saturating_sub(*start).min(u64::from(u32::MAX)) as u32)
    }
    pub fn frame(
        &self,
        id: u32,
        skills: &SkillModel,
        now_ms: u64,
        base: u16,
        frames: u32,
    ) -> Option<u16> {
        let remaining = self.remaining_ms(id, skills, now_ms);
        let delay = skills.binding_for(id).delay_ms?;
        (frames > 0 && remaining >= 100 && delay >= frames)
            .then(|| base + (frames - (remaining / (delay / frames)).min(frames)) as u16)
    }
}

/// Portable raw ingress alone enriches ABSENT data. Native normalized None stays unknown.
pub fn normalize_raw_skills(rows: &[serde_json::Value]) -> Result<SkillModel, serde_json::Error> {
    let mut normalized = Vec::new();
    for (index, raw) in rows
        .iter()
        .take(crate::skill_model::MAX_LEARNED_SKILLS)
        .enumerate()
    {
        let mut row = raw.clone();
        let Some(obj) = row.as_object_mut() else {
            continue;
        };
        if !obj.contains_key("id") {
            obj.insert("id".into(), serde_json::json!(index));
        }
        if !obj.contains_key("name") {
            obj.insert("name".into(), serde_json::json!(""));
        }
        if let Some(name) = obj.get("magicName").cloned() {
            obj.insert(
                "name".into(),
                if name.is_null() {
                    serde_json::json!("")
                } else {
                    name
                },
            );
        } else if obj.get("name").is_some_and(serde_json::Value::is_null) {
            obj.insert("name".into(), serde_json::json!(""));
        }
        if let Some(template) = obj
            .get("spell")
            .and_then(serde_json::Value::as_str)
            .and_then(mir2_game_data::crystal_magic_by_spell)
        {
            for (key, value) in [
                ("icon", serde_json::json!(template.icon)),
                ("need1", serde_json::json!(template.need1)),
                ("need2", serde_json::json!(template.need2)),
                ("need3", serde_json::json!(template.need3)),
            ] {
                obj.entry(key).or_insert(value);
            }
            if !obj.contains_key("magicName") && !raw.get("name").is_some() {
                obj.insert("name".into(), serde_json::json!(template.name));
            }
        }
        for key in ["delayMs", "delay_ms", "cooldown_ms", "cooldownMs"] {
            if obj.get(key).is_some_and(serde_json::Value::is_null) {
                obj.remove(key);
            }
        }
        normalized.push(row);
    }
    let mut model: SkillModel = serde_json::from_value(serde_json::json!({"skills": normalized}))?;
    for (binding, raw) in model.bindings.iter_mut().zip(rows.iter()) {
        if !["delayMs", "delay_ms", "cooldown_ms", "cooldownMs"]
            .into_iter()
            .any(|key| raw.get(key).is_some_and(|v| !v.is_null()))
        {
            binding.delay_ms = None;
        }
    }
    Ok(model)
}

#[cfg(test)]
#[path = "skill_page_state_tests.rs"]
mod tests;

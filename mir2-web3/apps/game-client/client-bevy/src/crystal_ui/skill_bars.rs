use super::*;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
#[path = "skill_bars_host.rs"]
pub mod host;
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillBarsUi {
    pub cursor: Option<[f32; 2]>,
    pub pending_casts: VecDeque<(u8, Option<[f32; 2]>)>,
    pub dragging: Option<(usize, [f32; 2])>,
    pub hovered: bool,
    pub armed_slot: Option<u8>,
    pub visible: [bool; 2],
    epoch: Option<(u64, u32)>,
    cast_starts: HashMap<u32, (u64, Instant)>,
}
impl SkillBarsUi {
    pub fn has_skill(skills: &SkillModel, bar: usize) -> bool {
        skills.bindings.iter().any(|b| {
            b.hotkey
                .is_some_and(|key| key >= bar as i32 * 8 + 1 && key <= (bar as i32 + 1) * 8 + 1)
        })
    }
    pub fn observe(&mut self, skills: &SkillModel, now: Instant) {
        let epoch = (
            skills.authority.session_epoch,
            skills.authority.player_object_id,
        );
        if self.epoch != Some(epoch) {
            self.cast_starts.clear();
            self.pending_casts.clear();
            self.epoch = Some(epoch);
        }
        self.cast_starts
            .retain(|id, _| skills.skills.iter().any(|s| s.id == *id));
        for binding in &skills.bindings {
            if binding.cast_sequence != 0
                && self
                    .cast_starts
                    .get(&binding.skill_id)
                    .is_none_or(|(seq, _)| *seq != binding.cast_sequence)
            {
                self.cast_starts
                    .insert(binding.skill_id, (binding.cast_sequence, now));
            }
        }
    }
    pub fn remaining_ms(&self, id: u32, skills: &SkillModel, now: Instant) -> u32 {
        let Some((_, started)) = self.cast_starts.get(&id) else {
            return 0;
        };
        let delay = skills.binding_for(id).delay_ms.unwrap_or(0);
        delay.saturating_sub(
            now.saturating_duration_since(*started)
                .as_millis()
                .min(u32::MAX as u128) as u32,
        )
    }
    pub fn cooldown_frame(&self, id: u32, skills: &SkillModel, now: Instant) -> Option<u16> {
        let remaining = self.remaining_ms(id, skills, now);
        let delay = skills.binding_for(id).delay_ms?;
        if remaining < 100 || delay < 22 {
            return None;
        }
        Some((1260 + 22 - (remaining / (delay / 22)).min(22)) as u16)
    }
    pub fn queue_cast(&mut self, slot: u8) {
        if (1..=16).contains(&slot) && self.pending_casts.len() < 16 {
            self.pending_casts.push_back((slot, self.cursor));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn skills() -> SkillModel {
        serde_json::from_value(serde_json::json!({"skills":[{"id":1,"name":"Fire Ball","spell":"FireBall","hotkey":9,"icon":1,"cooldown_ms":2200,"castSequence":7}]})).unwrap()
    }
    #[test]
    fn source_show_preserves_inclusive_bar_boundary_and_server_key() {
        let s = skills();
        assert!(SkillBarsUi::has_skill(&s, 0));
        assert!(SkillBarsUi::has_skill(&s, 1));
        assert_eq!(s.skill_for_shortcut(9).unwrap().id, 1);
        assert!(s.skill_for_shortcut(1).is_none());
    }
    #[test]
    fn server_cast_sequence_starts_once_and_delay_changes_do_not_restart_clock() {
        let now = Instant::now();
        let mut s = skills();
        let mut ui = SkillBarsUi::default();
        ui.observe(&s, now);
        assert_eq!(ui.cooldown_frame(1, &s, now), Some(1260));
        ui.observe(&s, now + std::time::Duration::from_secs(1));
        assert_eq!(
            ui.remaining_ms(1, &s, now + std::time::Duration::from_secs(1)),
            1200
        );
        s.bindings[0].delay_ms = Some(3300);
        assert_eq!(
            ui.remaining_ms(1, &s, now + std::time::Duration::from_secs(1)),
            2300
        );
        assert_eq!(
            ui.cooldown_frame(1, &s, now + std::time::Duration::from_millis(3250)),
            None
        );
        s.authority.session_epoch = 9;
        ui.observe(&s, now);
        assert_eq!(ui.remaining_ms(1, &s, now), 3300);
    }
}

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
    clock: crate::skill_page_state::SkillCooldownClock,
    origin: Option<Instant>,
}
impl SkillBarsUi {
    pub fn has_skill(skills: &SkillModel, bar: usize) -> bool {
        skills.bindings.iter().any(|b| {
            b.hotkey
                .is_some_and(|key| key >= bar as i32 * 8 + 1 && key <= (bar as i32 + 1) * 8 + 1)
        })
    }
    fn now_ms(&self, now: Instant) -> u64 {
        self.origin
            .map(|origin| {
                now.saturating_duration_since(origin)
                    .as_millis()
                    .min(u64::MAX as u128) as u64
            })
            .unwrap_or(0)
    }
    pub fn observe(&mut self, skills: &SkillModel, now: Instant) {
        let epoch = (
            skills.authority.session_epoch,
            skills.authority.player_object_id,
        );
        if self.epoch != Some(epoch) {
            self.pending_casts.clear();
            self.epoch = Some(epoch);
        }
        self.origin.get_or_insert(now);
        let elapsed = self.now_ms(now);
        self.clock.observe(skills, elapsed);
    }
    pub fn remaining_ms(&self, id: u32, skills: &SkillModel, now: Instant) -> u32 {
        self.clock.remaining_ms(id, skills, self.now_ms(now))
    }
    pub fn cooldown_frame(&self, id: u32, skills: &SkillModel, now: Instant) -> Option<u16> {
        self.clock.frame(id, skills, self.now_ms(now), 1260, 22)
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

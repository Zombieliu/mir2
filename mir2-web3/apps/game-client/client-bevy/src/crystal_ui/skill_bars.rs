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
    readiness_samples: HashMap<u32, SkillReadinessSample>,
}

#[derive(Debug, Clone, PartialEq)]
struct SkillReadinessSample {
    snapshot_serial: u64,
    remaining_ms: u32,
    observed_at: Instant,
}

fn snapshot_readiness_ms(binding: &crate::skill_model::SkillBinding) -> u32 {
    binding
        .cooldown_remaining_ms
        .unwrap_or_else(|| binding.cooldown_remaining_ticks.saturating_mul(1_000))
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
            self.readiness_samples.clear();
            self.pending_casts.clear();
            self.epoch = Some(epoch);
        }
        self.cast_starts
            .retain(|id, _| skills.skills.iter().any(|s| s.id == *id));
        self.readiness_samples
            .retain(|id, _| skills.skills.iter().any(|s| s.id == *id));
        for binding in &skills.bindings {
            if binding.cast_sequence != 0
                && self
                    .cast_starts
                    .get(&binding.skill_id)
                    .is_none_or(|(seq, _)| binding.cast_sequence > *seq)
            {
                self.cast_starts
                    .insert(binding.skill_id, (binding.cast_sequence, now));
            }
            let remaining_ms = snapshot_readiness_ms(binding);
            // The same retained snapshot can be processed every frame or
            // republished after an unrelated packet. Neither restarts its
            // cooldown. A stale serial also cannot extend a newer sample.
            let replace = self
                .readiness_samples
                .get(&binding.skill_id)
                .is_none_or(|sample| {
                    skills.authority.snapshot_serial > sample.snapshot_serial
                        || (skills.authority.snapshot_serial == 0
                            && sample.snapshot_serial == 0
                            && sample.remaining_ms != remaining_ms)
                });
            if replace {
                self.readiness_samples.insert(
                    binding.skill_id,
                    SkillReadinessSample {
                        snapshot_serial: skills.authority.snapshot_serial,
                        remaining_ms,
                        observed_at: now,
                    },
                );
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
    /// Local input readiness includes both the accepted spell's own delay and
    /// the sampled authoritative action gate. It keeps counting down while
    /// the Gateway is idle and no new world snapshot is required.
    ///
    /// Call `observe` before checking a freshly ingested model. Rendering uses
    /// `remaining_ms` instead, so another spell's short global gate does not
    /// draw this spell's cooldown animation.
    pub fn readiness_remaining_ms(&self, id: u32, skills: &SkillModel, now: Instant) -> u32 {
        let binding = skills.binding_for(id);
        let epoch = (
            skills.authority.session_epoch,
            skills.authority.player_object_id,
        );
        let sampled_remaining = self
            .readiness_samples
            .get(&id)
            .filter(|sample| {
                self.epoch == Some(epoch)
                    && sample.snapshot_serial >= skills.authority.snapshot_serial
            })
            .map(|sample| {
                sample.remaining_ms.saturating_sub(
                    now.saturating_duration_since(sample.observed_at)
                        .as_millis()
                        .min(u128::from(u32::MAX)) as u32,
                )
            })
            .unwrap_or_else(|| snapshot_readiness_ms(&binding));
        // Crystal's SpellToggle shortcuts do not pass through CanCast or
        // ClientMagic.CastTime. A late FlamingSword hit can draw its ordinary
        // spell overlay, but must not extend the authoritative arm deadline.
        let toggle = binding
            .cast_kind
            .as_deref()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("toggle"));
        let cast_remaining = if self.epoch == Some(epoch) && !toggle {
            self.remaining_ms(id, skills, now)
        } else {
            0
        };
        sampled_remaining.max(cast_remaining)
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
    use std::time::Duration;
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

    fn sampled_skills(remaining_ms: Option<u32>, ticks: u32) -> SkillModel {
        serde_json::from_value(serde_json::json!({
            "authority":{"sessionEpoch":1,"snapshotSerial":1,"playerObjectId":1001},
            "skills":[{"id":1,"spell":"Healing","hotkey":1,"cooldown_ms":2200,
                "cooldownRemainingTicks":ticks,"cooldownRemainingMs":remaining_ms}]
        }))
        .unwrap()
    }

    #[test]
    fn exact_readiness_expires_while_snapshot_is_idle_without_drawing_a_cast() {
        let now = Instant::now();
        let skills = sampled_skills(Some(300), 1);
        let mut ui = SkillBarsUi::default();
        ui.observe(&skills, now);
        assert_eq!(ui.readiness_remaining_ms(1, &skills, now), 300);
        assert_eq!(ui.cooldown_frame(1, &skills, now), None);
        ui.observe(&skills, now + Duration::from_millis(200));
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(299)),
            1
        );
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(300)),
            0
        );
        ui.observe(&skills, now + Duration::from_secs(20));
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_secs(20)),
            0,
            "a repeated positive tick sample must not resurrect an expired cooldown"
        );
    }

    #[test]
    fn legacy_tick_readiness_ages_instead_of_waiting_for_a_snapshot() {
        let now = Instant::now();
        let skills = sampled_skills(None, 2);
        let mut ui = SkillBarsUi::default();
        ui.observe(&skills, now);
        assert_eq!(ui.readiness_remaining_ms(1, &skills, now), 2000);
        ui.observe(&skills, now + Duration::from_millis(1000));
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(1999)),
            1
        );
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(2000)),
            0
        );
    }

    #[test]
    fn fresh_readiness_samples_replace_old_ones_but_stale_serials_cannot_restart_them() {
        let now = Instant::now();
        let mut skills = sampled_skills(Some(400), 1);
        let mut ui = SkillBarsUi::default();
        ui.observe(&skills, now);
        let old = skills.clone();
        skills.authority.snapshot_serial = 2;
        skills.bindings[0].cooldown_remaining_ms = Some(50);
        ui.observe(&skills, now + Duration::from_millis(300));
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(349)),
            1
        );
        ui.observe(&old, now + Duration::from_millis(350));
        assert_eq!(
            ui.readiness_remaining_ms(1, &old, now + Duration::from_millis(350)),
            0
        );
        skills.authority.snapshot_serial = 3;
        skills.bindings[0].cooldown_remaining_ms = Some(0);
        ui.observe(&skills, now + Duration::from_millis(360));
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(360)),
            0,
            "exact zero overrides rounded legacy ticks"
        );
    }

    #[test]
    fn reconnect_and_character_identity_clear_previous_readiness() {
        let now = Instant::now();
        let mut skills = sampled_skills(Some(2000), 2);
        let mut ui = SkillBarsUi::default();
        ui.observe(&skills, now);
        ui.queue_cast(1);
        skills.authority.session_epoch = 2;
        skills.bindings[0].cooldown_remaining_ms = Some(10);
        ui.observe(&skills, now + Duration::from_millis(50));
        assert!(ui.pending_casts.is_empty());
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(60)),
            0
        );
        skills.authority.player_object_id = 2002;
        skills.bindings[0].cooldown_remaining_ms = Some(0);
        ui.observe(&skills, now + Duration::from_millis(70));
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(70)),
            0
        );
    }

    #[test]
    fn spell_cooldown_visual_is_separate_from_another_spells_global_action_gate() {
        let now = Instant::now();
        let mut skills = sampled_skills(Some(300), 1);
        skills.bindings[0].cast_sequence = 1;
        skills.bindings[0].delay_ms = Some(150);
        let mut ui = SkillBarsUi::default();
        ui.observe(&skills, now);
        assert!(ui.cooldown_frame(1, &skills, now).is_some());
        assert_eq!(
            ui.cooldown_frame(1, &skills, now + Duration::from_millis(150)),
            None
        );
        assert_eq!(
            ui.readiness_remaining_ms(1, &skills, now + Duration::from_millis(150)),
            150
        );
        skills.bindings[0].delay_ms = Some(500);
        ui.observe(&skills, now + Duration::from_millis(200));
        assert_eq!(
            ui.remaining_ms(1, &skills, now + Duration::from_millis(200)),
            300,
            "a delay metadata update changes duration without restarting the accepted cast"
        );
    }

    #[test]
    fn stale_cast_sequence_cannot_restart_a_newer_accepted_spell_clock() {
        let now = Instant::now();
        let mut skills = skills();
        skills.authority.session_epoch = 1;
        skills.authority.snapshot_serial = 2;
        let mut ui = SkillBarsUi::default();
        ui.observe(&skills, now);
        let mut stale = skills.clone();
        stale.authority.snapshot_serial = 1;
        stale.bindings[0].cast_sequence = 6;
        ui.observe(&stale, now + Duration::from_millis(1000));
        assert_eq!(
            ui.remaining_ms(1, &skills, now + Duration::from_millis(1000)),
            1200
        );
        ui.observe(&skills, now + Duration::from_millis(2000));
        assert_eq!(
            ui.remaining_ms(1, &skills, now + Duration::from_millis(2200)),
            0
        );
    }

    #[test]
    fn late_flaming_sword_hit_overlay_does_not_extend_authoritative_arm_readiness() {
        let now = Instant::now();
        let mut skills = sampled_skills(Some(10000), 10);
        skills.bindings[0].spell = Some("FlamingSword".into());
        skills.bindings[0].cast_kind = Some("toggle".into());
        skills.bindings[0].delay_ms = Some(1800);
        let mut ui = SkillBarsUi::default();
        ui.observe(&skills, now);
        assert_eq!(ui.readiness_remaining_ms(1, &skills, now), 10000);

        let late_hit = now + Duration::from_millis(9900);
        skills.authority.snapshot_serial = 2;
        skills.bindings[0].cooldown_remaining_ms = Some(100);
        skills.bindings[0].cooldown_remaining_ticks = 1;
        skills.bindings[0].cast_sequence = 1;
        ui.observe(&skills, late_hit);
        assert_eq!(ui.remaining_ms(1, &skills, late_hit), 1800);
        assert_eq!(ui.readiness_remaining_ms(1, &skills, late_hit), 100);

        let arm_ready = now + Duration::from_millis(10000);
        assert_eq!(ui.readiness_remaining_ms(1, &skills, arm_ready), 0);
        assert_eq!(ui.remaining_ms(1, &skills, arm_ready), 1700);
        assert!(ui.cooldown_frame(1, &skills, arm_ready).is_some());
        skills.authority.snapshot_serial = 3;
        skills.bindings[0].cooldown_remaining_ms = Some(0);
        skills.bindings[0].cooldown_remaining_ticks = 0;
        ui.observe(&skills, arm_ready);
        assert_eq!(ui.readiness_remaining_ms(1, &skills, arm_ready), 0);
    }

    #[test]
    fn ordinary_spell_toggles_do_not_inherit_a_visual_cast_delay() {
        let now = Instant::now();
        for spell in ["Thrusting", "HalfMoon"] {
            let mut skills = sampled_skills(Some(0), 0);
            skills.bindings[0].spell = Some(spell.into());
            skills.bindings[0].cast_kind = Some("toggle".into());
            skills.bindings[0].cast_sequence = 1;
            let mut ui = SkillBarsUi::default();
            ui.observe(&skills, now);
            assert!(ui.remaining_ms(1, &skills, now) > 0);
            assert_eq!(ui.readiness_remaining_ms(1, &skills, now), 0, "{spell}");
        }
    }
}

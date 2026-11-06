//! Source Hero AssignKeyPanel state. C.MagicKey uses the shared request namespace.
use crate::hero_model::{HeroKeyOutcome, HeroKeyPending, HeroModel};
use crate::skill_model::next_skill_key_request_id;
use mir2_protocol::Spell;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeroAssignUi {
    pub open: bool,
    pub spell: Option<Spell>,
    pub key: u8,
    pub old_key: u8,
    pub notice: Option<String>,
    pub pending: Option<HeroKeyPending>,
    pub dispatched: bool,
    pub epoch: u64,
    pub actor: u32,
    pub generation: u64,
}
impl HeroAssignUi {
    pub fn show(&mut self, model: &HeroModel, index: usize) {
        let Some(info) = model.info.as_ref().filter(|i| i.hp > 0) else {
            return;
        };
        let Some(magic) = info.magics.get(index) else {
            return;
        };
        if self.pending.is_some() {
            return;
        }
        *self = Self {
            open: true,
            spell: Some(magic.spell),
            key: magic.key,
            old_key: magic.key,
            epoch: model.session_epoch,
            actor: info.object_id,
            generation: model.hero_generation,
            ..Default::default()
        };
    }
    pub fn choose(&mut self, key: u8) {
        if self.open && self.pending.is_none() && (key == 0 || (17..=24).contains(&key)) {
            self.key = key;
        }
    }
    pub fn request(&mut self, model: &HeroModel) -> Option<HeroKeyPending> {
        let info = model.info.as_ref()?;
        if !self.open
            || self.pending.is_some()
            || info.hp <= 0
            || info.object_id != self.actor
            || model.session_epoch != self.epoch
            || model.hero_generation != self.generation
        {
            return None;
        }
        let spell = self.spell?;
        if !info.magics.iter().any(|m| m.spell == spell) {
            return None;
        }
        // Original 0/0 accidentally routes to the PLAYER and clears its same-spell key.
        // A Hero None -> None save is a local no-op, never a network mutation.
        if self.key == 0 && self.old_key == 0 {
            if info.magics.iter().any(|m| m.spell == spell && m.key == 0) {
                self.open = false;
                self.notice = None;
            } else {
                self.notice = Some("This skill binding changed. Reopen it and try again.".into());
            }
            return None;
        }
        Some(HeroKeyPending {
            hero_generation: model.hero_generation,
            request_id: next_skill_key_request_id(),
            session_epoch: self.epoch,
            object_id: self.actor,
            snapshot_serial: model.skill_snapshot_serial,
            spell,
            key: self.key,
            old_key: self.old_key,
        })
    }
    pub fn queued(&mut self, request: HeroKeyPending, success: bool) {
        if success {
            self.pending = Some(request);
            self.notice = None;
            self.dispatched = false;
        } else {
            self.notice = Some("Unable to send. Please try again.".into());
        }
    }
    pub fn transport_result(&mut self, id: u64, spell: &str, key: u8, old_key: u8, success: bool) {
        let Some(p) = self.pending.as_ref() else {
            return;
        };
        if p.request_id != id
            || format!("{:?}", p.spell) != spell
            || p.key != key
            || p.old_key != old_key
        {
            return;
        }
        if success {
            self.open = false;
            self.dispatched = true;
        } else {
            self.pending = None;
            self.open = true;
            self.notice = Some("Unable to send. Please try again.".into());
        }
    }
    pub fn reconcile(&mut self, model: &HeroModel) {
        if self.actor != 0
            && (model.hero_generation != self.generation
                || model.session_epoch != self.epoch
                || model
                    .info
                    .as_ref()
                    .is_none_or(|i| i.object_id != self.actor))
        {
            *self = Self::default();
            return;
        }
        let Some(p) = self.pending.as_ref() else {
            return;
        };
        if let Some(result) = p.outcome(model) {
            self.pending = None;
            self.dispatched = false;
            match result {
                HeroKeyOutcome::Applied | HeroKeyOutcome::Unchanged => self.open = false,
                HeroKeyOutcome::Rejected => {
                    self.open = true;
                    self.notice = Some("Unable to save this key. Please try again.".into());
                }
            }
        }
    }
    pub fn overlay(&self, info: &mut mir2_protocol::HeroUserInformation) {
        let Some(p) = self.pending.as_ref().filter(|_| self.dispatched) else {
            return;
        };
        if info.object_id != p.object_id || (p.key == 0 && p.old_key == 0) {
            return;
        }
        for magic in &mut info.magics {
            if magic.spell == p.spell {
                magic.key = p.key;
            } else if p.key > 0 && magic.key == p.key {
                magic.key = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> HeroModel {
        let info=serde_json::from_value(serde_json::json!({
            "object_id":12,"name":"Hero","class":"Warrior","gender":"Male","level":7,"hair":0,
            "hp":30,"mp":10,"experience":0,"max_experience":100,"inventory":null,"equipment":null,
            "auto_pot":false,"auto_hp_percent":30,"auto_mp_percent":30,"hp_item_index":0,"mp_item_index":0,
            "magics":[{"name":"Fire Ball","spell":"FireBall","base_cost":1,"level_cost":0,"icon":1,
                "level1":1,"level2":2,"level3":3,"need1":1,"need2":2,"need3":3,
                "level":1,"key":17,"experience":0,"delay":10,"range":8,"cast_time":0}]
        })).unwrap();
        HeroModel {
            session_epoch: 7,
            info: Some(info),
            ..Default::default()
        }
    }
    #[test]
    fn hero_none_to_none_never_dispatches_the_player_unbind_branch() {
        let mut model = model();
        model.info.as_mut().unwrap().magics[0].key = 0;
        let player = crate::skill_model::SkillModel {
            bindings: vec![crate::skill_model::SkillBinding {
                spell: Some("FireBall".into()),
                hotkey: Some(5),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut ui = HeroAssignUi::default();
        ui.show(&model, 0);
        ui.choose(0);
        let mut wire = Vec::new();
        if let Some(request) = ui.request(&model) {
            wire.push(request);
        }
        assert!(wire.is_empty());
        assert!(!ui.open);
        assert!(ui.pending.is_none());
        assert_eq!(player.bindings[0].hotkey, Some(5));
        model.info.as_mut().unwrap().magics[0].key = 17;
        ui.show(&model, 0);
        ui.choose(0);
        let request = ui.request(&model).unwrap();
        assert_eq!((request.key, request.old_key), (0, 17));
    }
    #[test]
    fn hero_draft_survives_send_failure_and_does_not_accept_player_keys() {
        let model = model();
        let mut ui = HeroAssignUi::default();
        ui.show(&model, 0);
        ui.choose(8);
        assert_eq!(ui.key, 17);
        ui.choose(24);
        let request = ui.request(&model).unwrap();
        ui.queued(request.clone(), true);
        ui.transport_result(request.request_id + 1, "FireBall", 24, 17, false);
        assert!(ui.pending.is_some());
        ui.transport_result(request.request_id, "FireBall", 24, 17, false);
        assert!(ui.open);
        assert_eq!(ui.key, 24);
        assert!(ui.pending.is_none());
        ui.queued(request, true);
        let mut next = model.clone();
        next.session_epoch += 1;
        ui.reconcile(&next);
        assert_eq!(ui, HeroAssignUi::default());
    }
    #[test]
    fn hero_optimistic_key_waits_for_transport_then_exact_receipt() {
        let mut model = model();
        let mut ui = HeroAssignUi::default();
        ui.show(&model, 0);
        ui.choose(24);
        let request = ui.request(&model).unwrap();
        ui.queued(request.clone(), true);
        let mut displayed = model.info.clone().unwrap();
        ui.overlay(&mut displayed);
        assert_eq!(displayed.magics[0].key, 17);
        ui.transport_result(request.request_id, "FireBall", 24, 17, true);
        ui.overlay(&mut displayed);
        assert_eq!(displayed.magics[0].key, 24);
        assert!(ui.pending.is_some());
        assert!(!ui.open);
        model.observe_snapshot(&serde_json::json!({"stage5Systems":{"heroLearnedMagics":[{"spell":"FireBall","key":17}]},
            "skillKeyAck":{"requestId":request.request_id,"spell":"FireBall","key":24,"oldKey":17,"accepted":false}}));
        ui.reconcile(&model);
        assert!(ui.open);
        assert!(ui.pending.is_none());
        assert_eq!(ui.key, 24);
    }
}

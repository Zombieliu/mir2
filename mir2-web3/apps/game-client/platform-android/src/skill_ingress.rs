//! Android connection ownership and delivery for the shared Windows skill adapter.
use mir2_client_bevy::{
    native_skill_ingress::{
        project_native_skill_model, world_payload_tick_from, NativeSkillPacketCursor,
    },
    skill_model::SkillModel,
};
use serde_json::Value;
use std::collections::VecDeque;

const MAX_PENDING_RECEIPTS: usize = 16;

#[derive(Default)]
pub(crate) struct AndroidSkillIngress {
    cursor: NativeSkillPacketCursor,
    identity: Option<(u32, String)>,
    world: Option<Value>,
    receipts: VecDeque<String>,
    latest: Option<String>,
}

impl AndroidSkillIngress {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn clear_scene(&mut self) {
        // Preserve the private personal source for owner skill packets during
        // a same-character map transition. It is used only by SkillModel
        // projection: no host world/map/entity/render state is republished.
        // A different owner/character or connection still resets everything.
    }

    /// Called only after world_projection validates authenticated self/map data.
    pub(crate) fn snapshot(&mut self, raw: &str) -> Result<(), &'static str> {
        let mut world: Value = serde_json::from_str(raw).map_err(|_| "Invalid skill snapshot")?;
        let owner = world["playerObjectId"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .filter(|id| *id != 0)
            .ok_or("Missing skill owner")?;
        let name = world["entities"]
            .as_array()
            .and_then(|actors| {
                actors.iter().find(|actor| {
                    actor["kind"] == "selfPlayer"
                        && actor["objectId"].as_u64() == Some(u64::from(owner))
                })
            })
            .and_then(|actor| actor["name"].as_str())
            .filter(|name| !name.trim().is_empty())
            .ok_or("Missing skill character")?
            .to_owned();
        for field in ["knownSkills", "known_skills", "skills"] {
            if world
                .get(field)
                .is_some_and(|v| !v.is_null() && !v.is_array())
            {
                return Err("Invalid learned skill list");
            }
        }
        let identity = (owner, name);
        if self.identity.as_ref().is_some_and(|old| old != &identity) {
            self.reset();
        }
        self.identity = Some(identity);
        self.cursor.observe_snapshot(&mut world);
        self.queue_model(&world)?;
        // Exact results belong only to their original snapshot. An unrelated
        // cast/delta must never fabricate another copy of that key receipt.
        world
            .as_object_mut()
            .ok_or("Invalid skill snapshot")?
            .remove("skillKeyAck");
        self.world = Some(world);
        Ok(())
    }

    pub(crate) fn packet(&mut self, raw: &str) -> Result<bool, &'static str> {
        let envelope: Value = serde_json::from_str(raw).map_err(|_| "Invalid skill packet")?;
        if envelope["type"] != "packet" {
            return Ok(false);
        }
        let Some(packet) = envelope["packet"].as_str() else {
            return Ok(false);
        };
        let Some(payload) = envelope["payload"].as_object() else {
            return Ok(false);
        };
        let Some((owner, _)) = self.identity.as_ref() else {
            return Ok(false);
        };
        if self.world.is_none() {
            return Ok(false);
        }
        // Some owner-only packets omit objectId; explicit Hero/foreign/malformed
        // identities can never change this character's personal presentation.
        // UserInformation must not rebind the authenticated owner.
        if payload
            .get("hero")
            .is_some_and(|v| v.as_bool() != Some(false))
            || payload
                .get("objectId")
                .is_some_and(|v| v.as_u64() != Some(u64::from(*owner)))
            || (packet == "UserInformation" && !payload.contains_key("objectId"))
        {
            return Ok(false);
        }
        let tick = world_payload_tick_from(self.world.as_ref());
        if !self.cursor.apply_packet(packet, &envelope["payload"], tick) {
            return Ok(false);
        }
        let mut world = self.world.take().ok_or("Missing skill snapshot")?;
        self.cursor.apply_active_patches(&mut world, tick);
        let result = self.queue_model(&world);
        self.world = Some(world);
        result.map(|()| true)
    }

    fn queue_model(&mut self, world: &Value) -> Result<(), &'static str> {
        let mut model = project_native_skill_model(world);
        // Hero key-bank results are not player results; the Hero ingress leaf
        // remains separate rather than confirming a player's pending command.
        if model.get("skillKeyAck").is_some_and(|ack| {
            ack["key"].as_u64().unwrap_or(0) > 16 || ack["oldKey"].as_u64().unwrap_or(0) > 16
        }) {
            model["skillKeyAck"] = Value::Null;
        }
        let typed: SkillModel =
            serde_json::from_value(model.clone()).map_err(|_| "Invalid skill model")?;
        let json = model.to_string();
        if let Some(ack) = typed.skill_key_ack {
            if ack.request_id == 0
                || ack.spell.trim().is_empty()
                || ack.key > 16
                || ack.old_key > 16
            {
                return Err("Invalid skill key receipt");
            }
            if self.receipts.len() >= MAX_PENDING_RECEIPTS {
                return Err("Skill receipt queue full");
            }
            self.receipts.push_back(json);
            self.latest = None;
        } else {
            self.latest = Some(json);
        }
        Ok(())
    }

    pub(crate) fn flush(&mut self, mut push: impl FnMut(String) -> bool) -> bool {
        while let Some(receipt) = self.receipts.front() {
            if !push(receipt.clone()) {
                return false;
            }
            self.receipts.pop_front();
        }
        if let Some(latest) = self.latest.as_ref() {
            if !push(latest.clone()) {
                return false;
            }
            self.latest = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::crystal_ui::overlays::skill_bars::SkillBarsUi;
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn snapshot(owner: u32, spell: &str, tick: u64) -> Value {
        let cast_kind = if spell == "LionRoar" {
            "direction"
        } else {
            "target"
        };
        json!({"tick":tick,"playerObjectId":owner,"mapFileName":"0",
            "entities":[{"kind":"selfPlayer","objectId":owner,"name":"Fixture"}],
            "knownSkills":[{"id":1,"spell":spell,"castKind":cast_kind,"hotkey":1,"delayMs":2200,
                "cooldownRemainingTicks":1,"cooldownRemainingMs":300,"mpCost":7}]})
    }
    fn packet(name: &str, payload: Value) -> String {
        json!({"type":"packet","packet":name,"payload":payload}).to_string()
    }
    fn drain(ingress: &mut AndroidSkillIngress) -> Vec<SkillModel> {
        let mut models = vec![];
        assert!(ingress.flush(|raw| {
            models.push(serde_json::from_str(&raw).unwrap());
            true
        }));
        models
    }
    #[test]
    fn real_typed_skills_preserve_exact_ms_and_idle_readiness_for_three_classes() {
        for spell in ["LionRoar", "FireBall", "SoulFire"] {
            let mut ingress = AndroidSkillIngress::default();
            ingress
                .snapshot(&snapshot(42, spell, 100).to_string())
                .unwrap();
            let model = drain(&mut ingress).pop().unwrap();
            assert_ne!(model.authority.session_epoch, 0);
            assert_eq!(model.authority.player_object_id, 42);
            assert_eq!(model.authority.snapshot_serial, 1);
            assert_eq!(
                model.selection_for_shortcut(1).unwrap().spell.as_deref(),
                Some(spell)
            );
            assert_eq!(model.binding_for(1).cooldown_remaining_ms, Some(300));
            assert_eq!(model.binding_for(1).mp_cost, Some(7));
            let now = Instant::now();
            let mut bars = SkillBarsUi::default();
            bars.observe(&model, now);
            assert_eq!(
                bars.readiness_remaining_ms(1, &model, now + Duration::from_millis(300)),
                0
            );
            assert!(bars.cooldown_frame(1, &model, now).is_none());
        }
    }
    #[test]
    fn only_successful_owner_casts_start_shared_overlay_and_republish_does_not_restart_it() {
        let mut ingress = AndroidSkillIngress::default();
        ingress
            .snapshot(&snapshot(42, "FireBall", 100).to_string())
            .unwrap();
        drain(&mut ingress);
        for payload in [
            json!({"spell":"FireBall","cast":false}),
            json!({"spell":"FireBall"}),
            json!({"spell":"FireBall","cast":true,"hero":true}),
            json!({"spell":"FireBall","cast":true,"objectId":99}),
        ] {
            assert!(!ingress.packet(&packet("Magic", payload)).unwrap());
        }
        assert!(!ingress
            .packet(&packet(
                "ObjectMagic",
                json!({"objectId":42,"spell":"FireBall","cast":true})
            ))
            .unwrap());
        assert!(drain(&mut ingress).is_empty());
        assert!(ingress
            .packet(&packet("Magic", json!({"spell":"FireBall","cast":true})))
            .unwrap());
        let model = drain(&mut ingress).pop().unwrap();
        assert_eq!(model.binding_for(1).cast_sequence, 1);
        let now = Instant::now();
        let mut bars = SkillBarsUi::default();
        bars.observe(&model, now);
        assert!(ingress
            .packet(&packet(
                "MagicDelay",
                json!({"objectId":42,"spell":"FireBall","delay":2200})
            ))
            .unwrap());
        let delta = drain(&mut ingress).pop().unwrap();
        assert_eq!(
            delta.authority.snapshot_serial,
            model.authority.snapshot_serial
        );
        bars.observe(&delta, now + Duration::from_millis(500));
        assert_eq!(
            bars.remaining_ms(1, &delta, now + Duration::from_millis(500)),
            1700
        );
    }
    #[test]
    fn packet_metadata_wins_over_stale_tick_and_fresh_authority_retires_it() {
        let mut ingress = AndroidSkillIngress::default();
        let original = snapshot(42, "FireBall", 100);
        ingress.snapshot(&original.to_string()).unwrap();
        drain(&mut ingress);
        assert!(ingress
            .packet(&packet(
                "MagicDelay",
                json!({"objectId":42,"spell":"FireBall","delay":3100})
            ))
            .unwrap());
        drain(&mut ingress);
        ingress.snapshot(&original.to_string()).unwrap();
        assert_eq!(drain(&mut ingress)[0].binding_for(1).delay_ms, Some(3100));
        let mut fresh = original;
        fresh["tick"] = json!(101);
        fresh["knownSkills"][0]["delayMs"] = json!(1200);
        ingress.snapshot(&fresh.to_string()).unwrap();
        assert_eq!(drain(&mut ingress)[0].binding_for(1).delay_ms, Some(1200));
    }
    #[test]
    fn scene_retains_personal_cast_identity_but_reset_or_character_change_cannot_leak_it() {
        let mut ingress = AndroidSkillIngress::default();
        ingress
            .snapshot(&snapshot(42, "FireBall", 100).to_string())
            .unwrap();
        drain(&mut ingress);
        ingress
            .packet(&packet("MagicCast", json!({"spell":"FireBall"})))
            .unwrap();
        let prior = drain(&mut ingress).pop().unwrap();
        ingress.clear_scene();
        assert!(ingress
            .packet(&packet("MagicCast", json!({"spell":"FireBall"})))
            .unwrap());
        assert!(ingress
            .packet(&packet(
                "MagicDelay",
                json!({"objectId":42,"spell":"FireBall","delay":3100})
            ))
            .unwrap());
        assert!(ingress
            .packet(&packet(
                "NewMagic",
                json!({"hero":false,"magic":{"spell":"FireBall","name":"Fireball authoritative","icon":27}})
            ))
            .unwrap());
        let during = drain(&mut ingress).pop().unwrap();
        assert_eq!(during.authority, prior.authority);
        assert_eq!(during.binding_for(1).cast_sequence, 2);
        assert_eq!(during.binding_for(1).delay_ms, Some(3100));
        assert_eq!(during.binding_for(1).icon, Some(27));
        let now = Instant::now();
        let mut bars = SkillBarsUi::default();
        bars.observe(&during, now);
        ingress
            .snapshot(&snapshot(42, "FireBall", 101).to_string())
            .unwrap();
        let scene = drain(&mut ingress).pop().unwrap();
        assert_eq!(scene.authority.session_epoch, prior.authority.session_epoch);
        assert_eq!(scene.binding_for(1).cast_sequence, 2);
        assert_eq!(scene.binding_for(1).icon, Some(27));
        bars.observe(&scene, now + Duration::from_millis(500));
        assert_eq!(
            bars.remaining_ms(1, &scene, now + Duration::from_millis(500)),
            1700
        );
        ingress
            .snapshot(&snapshot(43, "FireBall", 1).to_string())
            .unwrap();
        let other = drain(&mut ingress).pop().unwrap();
        assert_ne!(other.authority.session_epoch, prior.authority.session_epoch);
        assert_eq!(other.binding_for(1).cast_sequence, 0);
        ingress.reset();
        assert!(!ingress
            .packet(&packet("MagicCast", json!({"spell":"FireBall"})))
            .unwrap());
        assert!(drain(&mut ingress).is_empty());
    }
    #[test]
    fn failed_exact_receipt_keeps_own_keys_through_backpressure_and_is_never_replayed() {
        let mut ingress = AndroidSkillIngress::default();
        let mut result = snapshot(42, "FireBall", 100);
        result["knownSkills"][0]["hotkey"] = json!(3);
        result["skillKeyAck"] =
            json!({"requestId":9,"spell":"FireBall","key":4,"oldKey":3,"accepted":false});
        ingress.snapshot(&result.to_string()).unwrap();
        assert!(!ingress.flush(|_| false));
        let mut next = snapshot(42, "FireBall", 101);
        next["knownSkills"][0]["hotkey"] = json!(5);
        ingress.snapshot(&next.to_string()).unwrap();
        let models = drain(&mut ingress);
        assert_eq!(models.len(), 2);
        assert_eq!(
            models[0]
                .selection_for_shortcut(3)
                .unwrap()
                .spell
                .as_deref(),
            Some("FireBall")
        );
        assert!(!models[0].skill_key_ack.as_ref().unwrap().accepted);
        assert_eq!(
            models[1]
                .selection_for_shortcut(5)
                .unwrap()
                .spell
                .as_deref(),
            Some("FireBall")
        );
        assert!(models[1].skill_key_ack.is_none());
        ingress
            .packet(&packet("MagicCast", json!({"spell":"FireBall"})))
            .unwrap();
        assert!(drain(&mut ingress)[0].skill_key_ack.is_none());
    }
    #[test]
    fn malformed_and_hero_results_cannot_confirm_player_key_operations() {
        let mut ingress = AndroidSkillIngress::default();
        let mut invalid = snapshot(42, "FireBall", 100);
        invalid["skillKeyAck"] =
            json!({"requestId":0,"spell":"FireBall","key":1,"oldKey":0,"accepted":true});
        assert!(ingress.snapshot(&invalid.to_string()).is_err());
        assert!(drain(&mut ingress).is_empty());
        invalid["skillKeyAck"]["requestId"] = json!(7);
        invalid["skillKeyAck"]["key"] = json!(17);
        ingress.snapshot(&invalid.to_string()).unwrap();
        assert!(drain(&mut ingress)[0].skill_key_ack.is_none());
        assert!(!ingress
            .packet(&packet("UserInformation", json!({"objectId":99,"mp":7})))
            .unwrap());
        assert!(!ingress
            .packet(&packet("RemoveMagic", json!({"objectId":99,"placeId":1})))
            .unwrap());
        assert!(!ingress
            .packet(&packet("RemoveMagic", json!({"hero":true,"placeId":1})))
            .unwrap());
        for payload in [
            json!({"hero":false,"objectId":99,"magic":{"spell":"FireBall","icon":27}}),
            json!({"hero":false,"objectId":"42","magic":{"spell":"FireBall","icon":27}}),
        ] {
            assert!(!ingress.packet(&packet("NewMagic", payload)).unwrap());
        }
        assert!(!ingress
            .packet(&packet(
                "MagicDelay",
                json!({"hero":true,"objectId":42,"spell":"FireBall","delay":3100})
            ))
            .unwrap());
    }
    #[test]
    fn receipt_capacity_is_bounded_without_evicting_prior_exact_results() {
        let mut ingress = AndroidSkillIngress::default();
        for id in 1..=MAX_PENDING_RECEIPTS {
            let mut result = snapshot(42, "FireBall", id as u64);
            result["skillKeyAck"] =
                json!({"requestId":id,"spell":"FireBall","key":1,"oldKey":0,"accepted":true});
            ingress.snapshot(&result.to_string()).unwrap();
        }
        let mut overflow = snapshot(42, "FireBall", 100);
        overflow["skillKeyAck"] =
            json!({"requestId":100,"spell":"FireBall","key":1,"oldKey":0,"accepted":false});
        assert!(ingress.snapshot(&overflow.to_string()).is_err());
        let models = drain(&mut ingress);
        assert_eq!(models.len(), MAX_PENDING_RECEIPTS);
        assert_eq!(models[0].skill_key_ack.as_ref().unwrap().request_id, 1);
        assert_eq!(
            models
                .last()
                .unwrap()
                .skill_key_ack
                .as_ref()
                .unwrap()
                .request_id,
            MAX_PENDING_RECEIPTS as u64
        );
    }
}

// Included by effects.rs: original MonsterObject intelligent-creature action effects.
fn pet_actor_index(actor: &Value) -> Option<u8> {
    if actor.get("kind").and_then(Value::as_str) != Some("monster") {
        return None;
    }
    let index = actor.get("image")?.as_u64()?.checked_sub(10000)?;
    (index <= 14).then_some(index as u8)
}
fn pet_action_sound(index: u8) -> Option<&'static str> {
    match index {
        0 => Some("pet_pig.wav"),
        1 => Some("pet_chick.wav"),
        2 | 6 => Some("pet_kitty.wav"),
        3 => Some("pet_skeleton.wav"),
        4 => Some("pet_pigman.wav"),
        5 => Some("pet_weman.wav"),
        7 => Some("pet_blackdragon.wav"),
        8 => Some("pet_olympicmascot.wav"),
        10 => Some("pet_frog.wav"),
        11 => Some("pet_monkey.wav"),
        _ => None,
    }
}
fn pet_effect_animation(index: u8, base: u32, count: u32, duration: u64) -> Option<Animation> {
    if index > 14 || count == 0 {
        return None;
    }
    let library = format!("Pet/{index:02}");
    let path = crate::assets::asset_path(&format!("original-ui/{library}/meta.json"))?;
    let raw: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let source = raw.get("frames")?.as_array()?;
    let frames = (base..base + count)
        .map(|i| {
            let frame = source
                .iter()
                .find(|f| f.get("index").and_then(Value::as_u64) == Some(i as u64))?;
            let frame: EffectFrameMeta = serde_json::from_value(frame.clone()).ok()?;
            let expected = format!("/original-ui/{library}/{i}.png");
            (frame.path == expected
                && frame.width > 0.
                && frame.height > 0.
                && crate::frame_png_exists(&frame.path))
            .then_some(frame)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Animation {
        name: format!("{library}/{base}"),
        kind: "cast".into(),
        frames,
        interval: (duration / count as u64).max(1),
        blend: true,
        opacity: 1.,
        repeat: false,
        offset_x: 0.,
        offset_y: 0.,
        duration_ms: duration,
        light: None,
    })
}
impl NativeEffects {
    fn cancel_pet_prestart(&mut self, packet: &str, payload: &Value) {
        if !matches!(
            packet,
            "ObjectAttack"
                | "ObjectRangeAttack"
                | "ObjectWalk"
                | "ObjectRun"
                | "ObjectTurn"
                | "ObjectStruck"
                | "ObjectDied"
                | "ObjectHide"
                | "ObjectRemove"
        ) {
            return;
        }
        let Some(id) = payload.get("objectId").and_then(Value::as_u64) else {
            return;
        };
        let prefix = format!("pet-{id}-");
        let keys = self
            .active
            .iter()
            .filter(|e| e.key.starts_with(&prefix) && self.now_ms < e.start_at)
            .map(|e| e.key.clone())
            .collect::<Vec<_>>();
        self.active.retain(|e| !keys.contains(&e.key));
        for key in keys {
            self.anchor_object_ids.remove(&key);
            self.prestart_source_object_ids.remove(&key);
        }
    }

    fn apply_pet_pickup_sound(&mut self, payload: &Value, provenance: &EffectProvenance) {
        // GameScene checks a real MonsterObject before calling PlayPickupSound.
        let Some(actor) = payload
            .get("_nativeTarget")
            .filter(|a| a.get("kind").and_then(Value::as_str) == Some("monster"))
        else {
            return;
        };
        let Some(id) = actor_sound_key(actor) else {
            return;
        };
        self.queue_immediate_sound(provenance, &format!("Pet.{id}.Pickup"), "pet_pickup.wav");
    }
    fn apply_pet_attack_effects(&mut self, payload: &Value, provenance: &EffectProvenance) {
        let Some(actor) = payload.get("_nativeAttacker") else {
            return;
        };
        let Some(index) = pet_actor_index(actor) else {
            return;
        };
        let Some(id) = payload
            .get("objectId")
            .and_then(Value::as_u64)
            .and_then(|i| u32::try_from(i).ok())
        else {
            return;
        };
        let ty = payload
            .get("attackType")
            .or_else(|| payload.get("type"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if ty == 1 {
            if let Some(sound) = pet_action_sound(index) {
                self.queue_immediate_sound(provenance, &format!("Pet.{id}.Attack2"), sound);
            }
        }
        let action = match ty {
            1 => mir2_bevy_runtime::entity_animation::AnimationAction::Attack2,
            2 => mir2_bevy_runtime::entity_animation::AnimationAction::Attack3,
            _ => return,
        };
        let catalog = crate::frame_sets::animation_catalog_for(
            mir2_bevy_runtime::entity_animation::EntityKind::Monster,
            &format!("Pet/{index:02}"),
            false,
        );
        let Some(desc) = catalog.descriptor(action) else {
            return;
        };
        let parts: &[(&str, u32, u32, u64, u16)] = match (index, ty) {
            (9, 1) => &[("SnowmanSnow", 208, 11, 1500, 1)],
            (8, 2) => &[
                ("CreatureFlame", 280, 4, 800, 1),
                ("CreatureSmoke", 256, 3, 1000, 4),
            ],
            _ => return,
        };
        for (name, base, count, duration, frame) in parts {
            if *frame >= desc.frame_count {
                continue;
            }
            let key = format!("pet-{id}-{name}");
            // TrackableEffect uses one named owner effect; an overlapping action must not restart it.
            if self.active.iter().any(|e| e.key == key) {
                continue;
            }
            let Some(animation) = pet_effect_animation(index, *base, *count, *duration) else {
                continue;
            };
            let (Some(x), Some(y)) = (
                value_f32(payload, "location", "x"),
                value_f32(payload, "location", "y"),
            ) else {
                continue;
            };
            let (x, y) = (x as i32, y as i32);
            self.anchor_object_ids.insert(key.clone(), id);
            self.prestart_source_object_ids.insert(key.clone(), id);
            self.active.push(EffectInstance {
                key,
                kind: EffectKindTag::Cast,
                tile_x: x,
                tile_y: y,
                from_x: None,
                from_y: None,
                current: Some(animation),
                queued: None,
                return_queued: None,
                started_at: self.now_ms,
                start_at: self
                    .now_ms
                    .saturating_add(desc.frame_interval_ms.saturating_mul(*frame as u64)),
                persistent_object_id: None,
                provenance: provenance.clone(),
            });
        }
    }
}

#[cfg(test)]
mod pet_tests {
    use super::*;
    fn attack(index: u8, ty: u8, id: u32) -> Value {
        serde_json::json!({"objectId":id,"attackType":ty,"location":{"x":11,"y":12},"_nativeAttacker":{"objectId":id,"kind":"monster","image":10000+index as u32,"ai":64}})
    }
    #[test]
    fn source_pet_effects_use_real_frames_offsets_and_frame_delays() {
        let mut fx = NativeEffects::default();
        let p = EffectProvenance::default();
        fx.apply_pet_attack_effects(&attack(8, 2, 7), &p);
        assert_eq!(fx.active.len(), 2);
        assert_eq!(fx.active[0].start_at, 100);
        assert_eq!(fx.active[1].start_at, 400);
        let fire = fx.active[0].current.as_ref().unwrap();
        assert_eq!(fire.frames.len(), 4);
        assert_eq!(fire.frames[0].path, "/original-ui/Pet/08/280.png");
        assert_eq!(fire.frames[0].height, 76.);
        assert_eq!(fire.duration_ms, 800);
        let smoke = fx.active[1].current.as_ref().unwrap();
        assert_eq!(smoke.frames.len(), 3);
        assert_eq!(smoke.duration_ms, 1000);
        fx.apply_pet_attack_effects(&attack(9, 1, 8), &p);
        assert_eq!(fx.active.len(), 3);
        assert_eq!(fx.active[2].start_at, 100);
        assert_eq!(fx.active[2].current.as_ref().unwrap().frames.len(), 11);
    }
    #[test]
    fn trackable_effects_deduplicate_per_owner_and_cancel_only_before_trigger() {
        let mut fx = NativeEffects::default();
        let p = EffectProvenance::default();
        let a = attack(8, 2, 7);
        fx.apply_pet_attack_effects(&a, &p);
        fx.apply_pet_attack_effects(&a, &p);
        assert_eq!(fx.active.len(), 2);
        fx.now_ms = 200;
        fx.cancel_pet_prestart("ObjectWalk", &a);
        assert_eq!(fx.active.len(), 1);
        assert!(fx.active[0].key.ends_with("CreatureFlame"));
        fx.apply_pet_attack_effects(&attack(8, 2, 8), &p);
        assert_eq!(fx.active.len(), 3);
    }
    #[test]
    fn pickup_requires_authoritative_monster_actor_and_source_missing_sounds_stay_missing() {
        let mut fx = NativeEffects::default();
        let p = EffectProvenance::default();
        fx.apply_pet_pickup_sound(&serde_json::json!({"objectId":7}), &p);
        assert!(fx.ready_sounds.is_empty());
        fx.apply_pet_pickup_sound(&serde_json::json!({"objectId":7,"_nativeTarget":{"objectId":7,"kind":"monster","image":10000}}),&p);
        assert_eq!(fx.ready_sounds.len(), 1);
        assert_eq!(fx.ready_sounds[0].file_name, "pet_pickup.wav");
        for i in [9, 12, 13, 14, 99] {
            assert_eq!(pet_action_sound(i), None);
        }
        for i in [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11] {
            let name = pet_action_sound(i).unwrap();
            assert!(mir2_client_bevy::audio::NATIVE_GAMEPLAY_SOUND_FILES.contains(&name));
            assert!(
                crate::assets::asset_path(&format!("original-ui/Sound/{name}"))
                    .unwrap()
                    .is_file()
            );
        }
    }
}

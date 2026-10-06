use super::*;
fn packet(name: &str, body: Value) -> String {
    json!({"type":"packet","packet":name,"payload":body}).to_string()
}
fn positions() -> HashMap<u32, (i32, i32)> {
    HashMap::from([(42, (302, 634)), (43, (304, 634))])
}
#[test]
fn original_manifest_cast_projectile_impact_and_ground_light_fields_are_loaded() {
    let catalog = EffectCatalog::load().unwrap();
    assert_eq!(
        catalog.cast_animation("FireBall", 4).unwrap().light,
        Some(6)
    );
    let phases = catalog.projectile_phases("FireBall", 4).unwrap();
    assert!(phases.iter().all(|phase| phase.light == Some(6)));
    assert_eq!(catalog.world_animation(39, 4, 0).unwrap().light, Some(3));
}
#[test]
fn real_cast_packet_snapshot_follows_active_visible_phase_and_generation() {
    let mut effects = SceneEffects::default();
    let raw = packet(
        "ObjectMagic",
        json!({"objectId":42,"location":{"x":302,"y":634},"direction":"Right","spell":"FireBall",
        "targetId":43,"target":{"x":304,"y":634},"cast":true,"level":0,"selfBroadcast":false,"secondaryTargetIds":[]}),
    );
    assert_eq!(
        effects.observe_packet(&raw, 100, &positions()),
        EffectPacketOutcome::Applied
    );
    assert!(effects.light_snapshots(99, 7, true).is_empty());
    let lights = effects.light_snapshots(100, 7, true);
    assert_eq!(lights.len(), 1);
    assert_eq!(lights[0].light, 6);
    assert_eq!(lights[0].generation, 7);
    assert_eq!((lights[0].tile_x, lights[0].tile_y), (302.0, 634.0));
    assert!(effects.light_snapshots(100, 7, false).is_empty());
    assert!(effects.light_snapshots(60_000, 7, true).is_empty());
}
#[test]
fn projectile_light_uses_the_same_fractional_tile_function_as_the_visible_frame() {
    let mut effects = SceneEffects::default();
    let raw = packet(
        "ObjectProjectile",
        json!({"sourceId":42,"destinationId":43,"spell":"FireBall"}),
    );
    assert_eq!(
        effects.observe_packet(&raw, 0, &positions()),
        EffectPacketOutcome::Applied
    );
    let light = effects.light_snapshots(50, 11, true).pop().unwrap();
    assert_eq!(light.light, 6);
    assert_eq!(light.tile_x, 303.0);
    assert_eq!(light.tile_y, 634.0);
    let effect = &effects.active[0];
    let (animation, start) = active_phase(effect, 50).unwrap();
    let frame = animation.frame_at(50 - start, false).unwrap();
    let entry = render_entry(effect, 50, (302, 634)).unwrap();
    assert_eq!(
        entry["left"].as_f64().unwrap() as f32,
        ENTITY_ORIGIN_X + (light.tile_x - 302.0) * CELL_WIDTH + frame.x + animation.offset.x
    );
    assert_eq!(
        entry["top"].as_f64().unwrap() as f32,
        ENTITY_ORIGIN_Y + (light.tile_y - 634.0) * CELL_HEIGHT + frame.y + animation.offset.y
    );
}
#[test]
fn ground_spell_light_is_persistent_but_remove_and_scene_reset_retire_it() {
    let mut effects = SceneEffects::default();
    let raw = packet(
        "ObjectSpell",
        json!({"objectId":9201,"location":{"x":300,"y":633},"spell":39,"direction":"Down","param":0}),
    );
    assert_eq!(
        effects.observe_packet(&raw, 0, &positions()),
        EffectPacketOutcome::Applied
    );
    let lights = effects.light_snapshots(10_000, 7, true);
    assert_eq!(lights.len(), 1);
    assert_eq!(lights[0].light, 3);
    assert_eq!((lights[0].tile_x, lights[0].tile_y), (300.0, 633.0));
    assert_eq!(
        effects.observe_packet(
            &packet("ObjectRemove", json!({"objectId":9201})),
            10_000,
            &positions()
        ),
        EffectPacketOutcome::Applied
    );
    assert!(effects.light_snapshots(10_000, 7, true).is_empty());
    effects.observe_packet(&raw, 0, &positions());
    effects.clear();
    assert!(effects.light_snapshots(0, 8, true).is_empty());
}
#[test]
fn rendered_effect_without_manifest_light_never_invents_a_light_source() {
    let mut effects = SceneEffects::default();
    let mut animation = effects
        .catalog
        .as_ref()
        .unwrap()
        .cast_animation("FireBall", 4)
        .unwrap();
    animation.light = None;
    effects.push(ActiveEffect {
        key: "no-light".into(),
        from: None,
        to: (302.0, 634.0),
        start_at_ms: 0,
        phases: vec![animation],
        persistent: false,
        anchor_object_id: None,
        source_object_id: None,
        destination_object_id: None,
    });
    assert!(render_entry(&effects.active[0], 0, (302, 634)).is_some());
    assert!(effects.light_snapshots(0, 7, true).is_empty());
}

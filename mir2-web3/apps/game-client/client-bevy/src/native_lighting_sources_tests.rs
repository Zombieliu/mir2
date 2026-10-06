use super::*;

fn world() -> Value {
    json!({"mapFileName":"0","playerObjectId":42,
        "entities":[{"objectId":42,"kind":"selfPlayer","x":10,"y":20},
        {"objectId":43,"kind":"npc","x":11,"y":19,"light":0},
        {"objectId":44,"kind":"monster","x":12,"y":20,"light":1}]})
}
fn environment() -> NativeLightingEnvironment {
    let mut environment = NativeLightingEnvironment::default();
    environment.observe_world_snapshot(&json!({"mapFileName":"0","lightSetting":4}));
    environment
}
fn render(
    world: &Value,
    map: Vec<NativeMapLightCell>,
    motion: &NativeLightingMotion,
    assets: &NativeLightAssets,
    effects: &[NativeEffectLightSnapshot],
) -> Value {
    build_native_lighting_render_state(
        &environment(),
        Some(7),
        false,
        world,
        (10, 20),
        map,
        motion,
        assets,
        effects,
    )
}
fn cells() -> Vec<NativeMapLightCell> {
    vec![NativeMapLightCell {
        key: "11:20:1".into(),
        x: 11,
        y: 20,
        light: 1,
        offset_x: -50,
        offset_y: -100,
    }]
}
fn effect(key: &str, generation: u64, x: f32, y: f32, light: i32) -> NativeEffectLightSnapshot {
    NativeEffectLightSnapshot {
        key: key.into(),
        generation,
        tile_x: x,
        tile_y: y,
        light,
    }
}

#[test]
fn original_entity_defaults_npc_priority_anchors_and_motion_are_preserved() {
    let motion = NativeLightingMotion {
        camera_offset_x: 8.0,
        camera_offset_y: -4.0,
        entity_offsets: HashMap::from([("44".into(), (3.0, 5.0))]),
    };
    let state = render(
        &world(),
        cells(),
        &motion,
        &NativeLightAssets::complete_fixture(),
        &[],
    );
    assert_eq!(state["enabled"], true);
    assert_eq!(state["entityLights"][0]["key"], "42");
    assert_eq!(state["entityLights"][0]["light"], 3);
    assert_eq!(state["entityLights"][0]["drawX"], 488.0);
    assert_eq!(state["entityLights"][0]["drawY"], 348.0);
    assert_eq!(state["entityLights"][1]["key"], "43");
    assert_eq!(state["entityLights"][1]["light"], 10);
    assert_eq!(state["entityLights"][2]["drawX"], 587.0);
    assert_eq!(state["entityLights"][2]["drawY"], 353.0);
    assert_eq!(state["mapLights"][0]["drawX"], 536.0);
    assert_eq!(state["mapLights"][0]["drawY"], 348.0);
    assert_eq!(state["mapLights"][0]["offsetX"], -50);
    assert_eq!(state["mapLights"][0]["offsetY"], -100);
}
#[test]
fn asset_completeness_is_actual_presence_and_partial_pack_fails_closed() {
    let mut presence = [true; 10];
    presence[9] = false;
    let assets = NativeLightAssets::from_presence(presence);
    assert!(!assets.complete());
    let state = render(&world(), cells(), &default_motion(), &assets, &[]);
    assert_eq!(state["enabled"], false);
    assert_eq!(state["entityLights"], json!([]));
    assert_eq!(state["mapLights"], json!([]));
}
fn default_motion() -> NativeLightingMotion {
    NativeLightingMotion::default()
}
#[test]
fn stale_map_disabled_and_windows_force_daylight_never_overwrites_environment() {
    let env = environment();
    let bad = json!({"mapFileName":"other","entities":world()["entities"]});
    let state = build_native_lighting_render_state(
        &env,
        Some(7),
        false,
        &bad,
        (10, 20),
        cells(),
        &default_motion(),
        &NativeLightAssets::complete_fixture(),
        &[],
    );
    assert_eq!(state["enabled"], false);
    let state = build_native_lighting_render_state(
        &env,
        Some(7),
        true,
        &world(),
        (10, 20),
        cells(),
        &default_motion(),
        &NativeLightAssets::default(),
        &[],
    );
    assert_eq!(state["enabled"], true);
    assert_eq!(state["mapLightSetting"], 2);
    assert_eq!(state["mapDarkLight"], 0);
    assert_eq!(env.time_of_day_light_setting, Some(4));
}
#[test]
fn nonfinite_camera_or_any_entity_pose_fails_closed() {
    for motion in [
        NativeLightingMotion {
            camera_offset_x: f32::NAN,
            ..Default::default()
        },
        NativeLightingMotion {
            camera_offset_y: f32::INFINITY,
            ..Default::default()
        },
        NativeLightingMotion {
            entity_offsets: HashMap::from([("unknown".into(), (f32::NEG_INFINITY, 0.0))]),
            ..Default::default()
        },
    ] {
        assert_eq!(
            render(
                &world(),
                cells(),
                &motion,
                &NativeLightAssets::complete_fixture(),
                &[]
            )["enabled"],
            false
        );
    }
}
#[test]
fn effect_lights_use_fractional_position_and_exact_generation_and_stable_sort() {
    let state = render(
        &world(),
        cells(),
        &default_motion(),
        &NativeLightAssets::complete_fixture(),
        &[
            effect("z", 7, 11.5, 19.25, 6),
            effect("a", 7, 10.0, 20.0, 6),
            effect("stale", 6, 10.0, 20.0, 6),
            effect("nan", 7, f32::NAN, 20.0, 6),
            effect("zero", 7, 10.0, 20.0, 0),
            effect("control\n", 7, 10.0, 20.0, 6),
        ],
    );
    let lights = state["entityLights"].as_array().unwrap();
    assert_eq!(lights.len(), 5);
    assert_eq!(lights[3]["key"], "effect:a");
    assert_eq!(lights[4]["key"], "effect:z");
    assert_eq!(lights[4]["drawX"], 552.0);
    assert_eq!(lights[4]["drawY"], 328.0);
}
#[test]
fn authoritative_owner_npc_and_entities_cannot_be_evicted_by_effect_or_map_cap() {
    let mut world = world();
    let actors = world["entities"].as_array_mut().unwrap();
    for id in 100..500 {
        actors.push(json!({"objectId":id,"kind":"monster","x":10,"y":20,"light":1}));
    }
    let effects = (0..250)
        .map(|i| effect(&i.to_string(), 7, 10.0, 20.0, 6))
        .collect::<Vec<_>>();
    let state = render(
        &world,
        cells(),
        &default_motion(),
        &NativeLightAssets::complete_fixture(),
        &effects,
    );
    assert_eq!(state["entityLights"].as_array().unwrap().len(), 200);
    assert_eq!(state["entityLights"][0]["key"], "42");
    assert_eq!(state["entityLights"][1]["key"], "43");
    assert_eq!(state["mapLights"], json!([]));
    assert!(
        state["entityLights"]
            .as_array()
            .unwrap()
            .iter()
            .all(|light| light["kind"] != "effect")
    );
}
#[test]
fn dead_nonowners_are_absent_but_dead_self_and_spell_still_follow_original_rules() {
    let world = json!({"mapFileName":"0","playerObjectId":42,"entities":[
        {"objectId":42,"kind":"selfPlayer","x":10,"y":20,"dead":true},
        {"objectId":43,"kind":"npc","x":10,"y":20,"dead":true},
        {"objectId":44,"kind":"monster","x":10,"y":20,"dead":true,"light":6},
        {"objectId":45,"kind":"spell","x":10,"y":20,"dead":true,"light":6}]});
    let lights = render(
        &world,
        vec![],
        &default_motion(),
        &NativeLightAssets::complete_fixture(),
        &[],
    )["entityLights"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(lights.len(), 2);
    assert_eq!(lights[0]["key"], "42");
    assert_eq!(lights[1]["key"], "45");
}
#[test]
fn map_scan_order_and_original_visibility_limits_are_preserved() {
    let mut map = cells();
    map.extend([
        NativeMapLightCell {
            key: "outside".into(),
            x: 51,
            y: 20,
            light: 1,
            offset_x: 0,
            offset_y: 0,
        },
        NativeMapLightCell {
            key: "edge".into(),
            x: 50,
            y: 61,
            light: 1,
            offset_x: 0,
            offset_y: 0,
        },
    ]);
    let state = render(
        &world(),
        map,
        &default_motion(),
        &NativeLightAssets::complete_fixture(),
        &[],
    );
    assert_eq!(state["mapLights"].as_array().unwrap().len(), 2);
    assert_eq!(state["mapLights"][0]["key"], "11:20:1");
    assert_eq!(state["mapLights"][1]["key"], "edge");
}
#[test]
fn original_cell_hook_uses_x_major_order_and_only_animated_frame_offsets() {
    let cell = |light, front_index, front_image, front_animation_frame| NativeMapLightInput {
        light,
        front_index,
        front_image,
        front_animation_frame,
    };
    let inputs = [
        cell(1, 2, 1, 0),
        cell(2, 2, 1, 2),
        cell(0, 2, 1, 0),
        cell(9, 2, 1, 2),
        cell(10, 2, 1, 0),
        cell(1, -1, 1, 0),
        cell(1, 2, 0, 0),
        cell(1, 2, 1, 0),
    ];
    let out = native_map_light_cells(
        2,
        4,
        inputs,
        &HashMap::from([((0, 0), (99, 88)), ((0, 1), (-50, -100))]),
    );
    assert_eq!(
        out.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
        ["0:0:1", "0:1:2", "0:3:9", "1:3:1"]
    );
    assert_eq!((out[0].offset_x, out[0].offset_y), (0, 0));
    assert_eq!((out[1].offset_x, out[1].offset_y), (-50, -100));
    assert_eq!((out[2].offset_x, out[2].offset_y), (0, 0));
}
#[test]
fn malformed_or_empty_maps_and_trailing_cells_do_not_invent_sources() {
    let c = NativeMapLightInput {
        light: 1,
        front_index: 2,
        front_image: 1,
        front_animation_frame: 0,
    };
    assert!(native_map_light_cells(0, 1, [c], &HashMap::new()).is_empty());
    assert!(native_map_light_cells(1, 0, [c], &HashMap::new()).is_empty());
    assert_eq!(
        native_map_light_cells(1, 1, [c, c], &HashMap::new()).len(),
        1
    );
}

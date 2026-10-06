use super::*;
use std::collections::HashMap;

#[test]
fn actual_original_png_headers_are_checked_with_exact_paths_and_byte_caps() {
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[255, 255, 255, 255])
        .unwrap();
    let mut paths = Vec::new();
    let assets = load_light_assets(|path, cap| {
        assert_eq!(cap, 1024 * 1024);
        paths.push(path.to_owned());
        Ok(png.clone())
    });
    assert!(assets.complete());
    assert_eq!(
        paths,
        (0..10)
            .map(|range| format!("original-effects/Lighting/{range}.png"))
            .collect::<Vec<_>>()
    );
    let incomplete = load_light_assets(|path, _| {
        if path.ends_with("/9.png") {
            Err(crate::world_assets::WorldAssetError::new("Not packaged"))
        } else {
            Ok(png.clone())
        }
    });
    assert!(!incomplete.complete());
    assert!(!load_light_assets(|_, _| Ok(b"not a PNG".to_vec())).complete());
}

fn world(map: &str, x: i32) -> Value {
    json!({"mapFileName":map,"playerObjectId":42,"entities":[
        {"objectId":42,"kind":"selfPlayer","name":"Fixture","x":x,"y":20,"light":3},
        {"objectId":43,"kind":"npc","name":"NPC","x":x+1,"y":20,"light":0}]})
}
fn env(map: &str) -> NativeLightingEnvironment {
    let mut env = NativeLightingEnvironment::default();
    env.observe_world_snapshot(&json!({"mapFileName":map,"lightSetting":4}));
    env
}
fn sources(request: u64, map: &str, center: (i32, i32)) -> AndroidLightingAssetFrame {
    AndroidLightingAssetFrame {
        request_id: request,
        map_file_name: map.into(),
        center,
        cells: vec![NativeMapLightCell {
            key: "11:20:1".into(),
            x: 11,
            y: 20,
            light: 1,
            offset_x: -50,
            offset_y: -100,
        }],
        assets: NativeLightAssets::complete_fixture(),
    }
}
fn stage(render: &mut AndroidLightingRender, request: u64, map: &str, x: i32) {
    render.stage_world(world(map, x));
    assert!(render.expect_render(request, (x, 20)));
    assert!(render.admit_sources(sources(request, map, (x, 20))));
}
fn out(render: &mut AndroidLightingRender) -> Value {
    let mut output = None;
    assert!(render.flush(|raw| {
        output = Some(serde_json::from_str::<Value>(&raw).unwrap());
        true
    }));
    output.expect("Expected newly produced lighting frame")
}
fn produce(render: &mut AndroidLightingRender, map: &str, x: i32, ready: bool, visible: bool) {
    render
        .produce(
            &env(map),
            visible,
            Some((x, 20)),
            |_| ready,
            &NativeLightingMotion::default(),
            &[],
        )
        .unwrap();
}

#[test]
fn actual_owner_and_map_sources_leave_disabled_metadata_stage_after_exact_render_receipt() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    produce(&mut render, "0", 10, true, true);
    let state = out(&mut render);
    assert_eq!(
        state["enabled"], true,
        "Actual Android scene still has no lighting source production"
    );
    assert_eq!(state["entityLights"][0]["key"], "42");
    assert_eq!(state["entityLights"][1]["key"], "43");
    assert_eq!(state["mapLights"][0]["key"], "11:20:1");
    assert_eq!(state["mapLights"][0]["offsetX"], -50);
    assert_eq!(state["mapLights"][0]["offsetY"], -100);
}
#[test]
fn resource_queued_without_renderer_receipt_cannot_enable_lighting() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    produce(&mut render, "0", 10, false, true);
    assert_eq!(out(&mut render)["enabled"], false);
    assert!(render.active.is_none());
}
#[test]
fn coherent_center_is_required_even_for_ready_receipt() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    produce(&mut render, "0", 11, true, true);
    assert_eq!(out(&mut render)["enabled"], false);
    assert!(render.active.is_none());
}
#[test]
fn no_source_result_before_validated_snapshot_and_request_is_admitted() {
    let mut render = AndroidLightingRender::default();
    assert!(!render.admit_sources(sources(10, "0", (10, 20))));
    assert!(!render.expect_render(10, (10, 20)));
    render.stage_world(world("0", 10));
    assert!(!render.expect_render(0, (10, 20)));
    assert!(!render.admit_sources(sources(10, "0", (10, 20))));
    assert!(render.expect_render(10, (10, 20)));
    assert!(!render.admit_sources(sources(11, "0", (10, 20))));
    assert!(!render.admit_sources(sources(10, "other", (10, 20))));
    assert!(!render.admit_sources(sources(10, "0", (11, 20))));
}
#[test]
fn older_loaded_request_cannot_unlock_new_snapshot() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    render.stage_world(world("0", 11));
    assert!(render.expect_render(11, (11, 20)));
    assert!(!render.admit_sources(sources(10, "0", (10, 20))));
    produce(&mut render, "0", 11, true, true);
    assert_eq!(out(&mut render)["enabled"], false);
}
#[test]
fn same_map_inflight_candidate_retains_old_committed_sources_until_new_receipt() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    produce(&mut render, "0", 10, true, true);
    let first = out(&mut render);
    stage(&mut render, 11, "0", 11);
    produce(&mut render, "0", 10, false, true);
    assert!(render.flush(|_| panic!("Unchanged committed frame repeated")));
    assert_eq!(render.active.as_ref().unwrap().sources.request_id, 10);
    produce(&mut render, "0", 11, true, true);
    let next = out(&mut render);
    assert_eq!(first["enabled"], true);
    assert_eq!(next["enabled"], true);
    assert_eq!(render.active.as_ref().unwrap().sources.request_id, 11);
    assert_eq!(next["entityLights"][0]["drawX"], 480.0);
}
#[test]
fn reset_retires_old_sources_retry_and_generation() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    produce(&mut render, "0", 10, true, true);
    assert!(!render.flush(|_| false));
    let generation = render.generation();
    render.clear_scene();
    assert!(render.generation() > generation);
    assert!(render.flush(|_| panic!("Old retry survived reset")));
    assert!(!render.admit_sources(sources(10, "0", (10, 20))));
    produce(&mut render, "0", 10, true, true);
    assert_eq!(out(&mut render)["enabled"], false);
}
#[test]
fn map_switch_drops_previous_active_and_waits_for_destination_render_receipt() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    produce(&mut render, "0", 10, true, true);
    out(&mut render);
    stage(&mut render, 11, "other", 11);
    produce(&mut render, "other", 11, false, true);
    let state = out(&mut render);
    assert_eq!(state["enabled"], false);
    assert_eq!(state["mapLights"], json!([]));
    produce(&mut render, "other", 11, true, true);
    assert_eq!(out(&mut render)["enabled"], true);
}
#[test]
fn effect_fractional_tile_and_current_generation_are_shared_not_reinterpolated() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    let effect = NativeEffectLightSnapshot {
        generation: render.generation(),
        key: "fire".into(),
        tile_x: 11.5,
        tile_y: 19.25,
        light: 6,
    };
    let stale = NativeEffectLightSnapshot {
        generation: render.generation() + 1,
        key: "stale".into(),
        ..effect.clone()
    };
    let motion = NativeLightingMotion {
        entity_offsets: HashMap::from([("42".into(), (8.0, -4.0))]),
        ..Default::default()
    };
    render
        .produce(
            &env("0"),
            true,
            Some((10, 20)),
            |_| true,
            &motion,
            &[effect, stale],
        )
        .unwrap();
    let state = out(&mut render);
    assert_eq!(state["entityLights"][0]["drawX"], 488.0);
    assert_eq!(state["entityLights"][0]["drawY"], 348.0);
    assert_eq!(state["entityLights"][2]["key"], "effect:fire");
    assert_eq!(state["entityLights"][2]["drawX"], 552.0);
    assert_eq!(state["entityLights"][2]["drawY"], 328.0);
    assert_eq!(state["entityLights"].as_array().unwrap().len(), 3);
}
#[test]
fn exact_full_frame_retry_and_unchanged_dedup_do_not_fake_render_or_authority_ack() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    produce(&mut render, "0", 10, true, true);
    let mut rejected = None;
    assert!(!render.flush(|raw| {
        rejected = Some(raw);
        false
    }));
    let mut retried = None;
    assert!(!render.flush(|raw| {
        retried = Some(raw);
        false
    }));
    assert_eq!(rejected, retried);
    assert!(render.flush(|raw| {
        assert_eq!(Some(raw), rejected);
        true
    }));
    produce(&mut render, "0", 10, true, true);
    assert!(render.flush(|_| panic!("Already sent identical frame repeated")));
}
#[test]
fn missing_original_texture_fails_closed_without_placeholder_light_sources() {
    let mut render = AndroidLightingRender::default();
    render.stage_world(world("0", 10));
    render.expect_render(10, (10, 20));
    let mut frame = sources(10, "0", (10, 20));
    frame.assets = NativeLightAssets::default();
    assert!(render.admit_sources(frame));
    produce(&mut render, "0", 10, true, true);
    let state = out(&mut render);
    assert_eq!(state["enabled"], false);
    assert_eq!(state["entityLights"], json!([]));
    assert_eq!(state["mapLights"], json!([]));
}
#[test]
fn source_batch_and_keys_are_bounded_transactionally() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    let prior = render.candidate.as_ref().unwrap().sources.clone();
    let mut bad = sources(10, "0", (10, 20));
    bad.cells[0].key = "bad\n".into();
    assert!(!render.admit_sources(bad));
    assert_eq!(render.candidate.as_ref().unwrap().sources, prior);
    let mut bad = sources(10, "0", (10, 20));
    bad.cells = vec![bad.cells[0].clone(); MAX_MAP_LIGHT_CELLS + 1];
    assert!(!render.admit_sources(bad));
}
#[test]
fn foreground_visibility_and_map_metadata_remain_independent_fail_closed_barriers() {
    let mut render = AndroidLightingRender::default();
    stage(&mut render, 10, "0", 10);
    produce(&mut render, "0", 10, true, true);
    assert_eq!(out(&mut render)["enabled"], true);
    produce(&mut render, "0", 10, true, false);
    assert_eq!(out(&mut render)["enabled"], false);
    produce(&mut render, "0", 10, true, true);
    assert_eq!(out(&mut render)["enabled"], true);
    produce(&mut render, "other", 10, true, true);
    assert_eq!(out(&mut render)["enabled"], false);
}

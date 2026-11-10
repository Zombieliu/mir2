use super::*;
use crate::{
    atlas::{resolved_native_sprite, NATIVE_FISHING_POSE},
    gameplay_bridge::{NativeGameplayAdapter, NATIVE_FISHING_OVERLAY},
    native_protocol::PacketEvent,
};
use serde_json::json;

fn shore_payload() -> Value {
    json!({
        "mapFileName":"0", "mapIndex":0, "playerObjectId":1000,
        "sceneView":{"center":{"x":342,"y":244},"width":24,"height":18},
        "entities":[{
            "objectId":1000, "kind":"selfPlayer", "class":"Taoist", "gender":"Male",
            "x":342, "y":244, "direction":"UpLeft", "dead":false,
            "fishing":false, "ridingMount":false, "transformType":-1,
            "sprite":{"bodyLibrary":"CArmour/05","hairLibrary":"CHair/00",
                "weaponLibrary":"CWeapon/49","frameBaseOffset":0,"weaponFrameOffset":0}
        }]
    })
}

fn packet(name: &str, payload: Value) -> PacketEvent {
    PacketEvent::Other {
        packet: name.to_owned(),
        payload,
    }
}

fn fishing_packet(id: u32, active: bool, progress: u32) -> PacketEvent {
    packet(
        "FishingUpdate",
        json!({
            "objectId":id,"fishing":active,"progressPercent":progress,"chancePercent":38,
            "fishingPoint":{"x":339,"y":247},"foundFish":active && progress == 100,
        }),
    )
}

fn sync(
    presentation: &mut NativeEntityPresentation,
    adapter: &NativeGameplayAdapter,
    source: &Value,
    at: u64,
) {
    let mut payload = source.clone();
    adapter.apply_authoritative_overlay(&mut payload);
    presentation.replace_payload(payload);
    presentation.sync_pending_payload(at, at);
}

fn fishing_frame(presentation: &mut NativeEntityPresentation, at: u64, expected: i64, rod: u8) {
    let mut seen = false;
    let rendered = presentation.render_state_if_changed_with(at, true, |payload, frames, _| {
        let entity = &payload["entities"][0];
        assert_eq!(entity["objectId"], 1000);
        assert_eq!(entity["direction"], "downleft");
        assert_eq!(entity[NATIVE_FISHING_POSE], rod);
        assert_eq!(frames["1000"], (expected, AnimationAction::Standing));
        let sprite = resolved_native_sprite(entity, AnimationAction::Standing);
        assert_eq!(sprite.body_library, "/original-ui/CArmour/05");
        assert_eq!(
            sprite.hair_library.as_deref(),
            Some("/original-ui/CHair/00")
        );
        assert_eq!(
            sprite.weapon_library,
            Some(format!("/original-ui/Fishing/{:02}", rod - 49))
        );
        assert_eq!(sprite.weapon_frame_offset, Some(-632));
        assert_eq!(sprite.weapon_library_secondary, None);
        seen = true;
        Some(json!({}))
    });
    assert!(seen && rendered.is_some());
    assert_eq!(presentation.self_equipment_pose("1000"), Some((false, -1)));
}

#[test]
fn native_fishing_visual_uses_typed_edges_and_source_frame_clock() {
    let mut adapter = NativeGameplayAdapter::default();
    let mut presentation = NativeEntityPresentation::default();
    let source = shore_payload();
    adapter.observe_world_snapshot(&source);
    sync(&mut presentation, &adapter, &source, 0);
    assert_eq!(presentation.self_equipment_pose("1000"), Some((true, -1)));
    assert!(!adapter.observe_packet(&fishing_packet(9000, true, 0)));
    assert!(!adapter.observe_packet(&packet(
        "FishingUpdate",
        json!({"objectId":1000,"fishing":true})
    )));
    assert!(adapter.observe_packet(&fishing_packet(1000, true, 0)));
    sync(&mut presentation, &adapter, &source, 100);
    fishing_frame(&mut presentation, 100, 672, 49);
    assert!(!adapter.observe_packet(&fishing_packet(1000, true, 25)));
    assert!(adapter.observe_packet(&fishing_packet(1000, true, 100)));
    sync(&mut presentation, &adapter, &source, 175);
    fishing_frame(&mut presentation, 200, 673, 49);
    fishing_frame(&mut presentation, 800, 679, 49);
    fishing_frame(&mut presentation, 900, 726, 49);

    assert!(adapter.observe_packet(&fishing_packet(1000, false, 100)));
    sync(&mut presentation, &adapter, &source, 1020);
    fishing_frame(&mut presentation, 1020, 727, 49);
    fishing_frame(&mut presentation, 1620, 784, 49);
    fishing_frame(&mut presentation, 2320, 791, 49);
    presentation
        .render_state_if_changed_with(2420, true, |payload, _, _| {
            let entity = &payload["entities"][0];
            assert!(entity.get(NATIVE_FISHING_POSE).is_none());
            assert_eq!(
                resolved_native_sprite(entity, AnimationAction::Standing)
                    .weapon_library
                    .as_deref(),
                Some("/original-ui/CWeapon/49")
            );
            Some(json!({}))
        })
        .expect("idle must replace the reel pose");
    assert_eq!(presentation.self_equipment_pose("1000"), Some((true, -1)));

    assert!(adapter.observe_packet(&fishing_packet(1000, true, 0)));
    sync(&mut presentation, &adapter, &source, 2500);
    fishing_frame(&mut presentation, 2500, 672, 49);
    assert!(adapter.observe_packet(&fishing_packet(1000, false, 100)));
    sync(&mut presentation, &adapter, &source, 2750);
    fishing_frame(&mut presentation, 2750, 674, 49);
    fishing_frame(&mut presentation, 3300, 784, 49);
}

#[test]
fn native_fishing_visual_retires_on_map_death_and_removal() {
    let mut adapter = NativeGameplayAdapter::default();
    let mut presentation = NativeEntityPresentation::default();
    let mut source = shore_payload();
    source["entities"][0]["sprite"]["weaponLibrary"] = json!("CWeapon/50");
    adapter.observe_world_snapshot(&source);
    sync(&mut presentation, &adapter, &source, 0);
    assert!(adapter.observe_packet(&fishing_packet(1000, true, 0)));
    sync(&mut presentation, &adapter, &source, 100);
    fishing_frame(&mut presentation, 100, 672, 50);

    adapter.observe_packet(&packet("MapChanged", json!({"mapIndex":2})));
    source["mapFileName"] = json!("2");
    adapter.observe_world_snapshot(&source);
    sync(&mut presentation, &adapter, &source, 200);
    assert_eq!(presentation.fishing_actors["1000"].animation.phase(), None);
    assert_eq!(
        presentation.latest_payload.as_ref().unwrap()["entities"][0][NATIVE_FISHING_OVERLAY],
        json!({"reset":true})
    );

    assert!(adapter.observe_packet(&fishing_packet(1000, true, 0)));
    sync(&mut presentation, &adapter, &source, 300);
    fishing_frame(&mut presentation, 300, 672, 50);
    assert!(adapter.observe_packet(&packet("Death", json!({}))));
    sync(&mut presentation, &adapter, &source, 400);
    assert!(!presentation.fishing_actors.contains_key("1000"));
    assert!(!adapter.observe_packet(&fishing_packet(1000, true, 25)));

    assert!(adapter.observe_packet(&packet("ObjectRemove", json!({"objectId":1000}))));
    sync(&mut presentation, &adapter, &source, 500);
    assert!(presentation.latest_payload.as_ref().unwrap()["entities"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!adapter.observe_packet(&fishing_packet(1000, true, 0)));
}

#[test]
fn native_fishing_water_and_audio_follow_frame_one_and_release_custody() {
    let mut adapter = NativeGameplayAdapter::default();
    let mut presentation = NativeEntityPresentation::default();
    let mut audio = NativeGameplayAudioQueue::default();
    let source = shore_payload();
    presentation.bind_fishing_audio_generation(7);
    adapter.observe_world_snapshot(&source);
    sync(&mut presentation, &adapter, &source, 0);
    assert!(adapter.observe_packet(&fishing_packet(1000, true, 0)));
    sync(&mut presentation, &adapter, &source, 100);

    let mut last_sequence = 0;
    let mut tick = |presentation: &mut NativeEntityPresentation, at, visible, expected: &[&str]| {
        let result =
            presentation.render_state_if_changed_with(at, visible, |_, _, _| Some(json!({})));
        let events = std::mem::take(&mut presentation.fishing_sound_events);
        assert_eq!(
            events
                .iter()
                .map(|event| event.file_name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        for event in events {
            assert_eq!(event.generation, 7);
            assert!(event.sequence > last_sequence);
            last_sequence = event.sequence;
            assert!(audio.push(event.clone()));
            assert!(!audio.push(event));
        }
        result
    };
    tick(&mut presentation, 199, false, &[]);
    tick(&mut presentation, 200, false, &["fishing_throw.wav"]);
    tick(&mut presentation, 201, false, &[]);
    tick(&mut presentation, 900, true, &[]);
    assert!(presentation.last_fishing_cues.is_empty());
    tick(&mut presentation, 1020, true, &[]);
    assert_eq!(
        presentation.last_fishing_cues["1000"],
        NativeFishingCueFrames {
            point: (339, 247),
            water: [640, 650],
            actor: None,
        }
    );

    assert!(adapter.observe_packet(&fishing_packet(1000, true, 100)));
    sync(&mut presentation, &adapter, &source, 1050);
    assert!(!adapter.observe_packet(&fishing_packet(1000, true, 100)));
    tick(&mut presentation, 1140, true, &[]);
    assert_eq!(presentation.last_fishing_cues["1000"].water, [641, 651]);
    tick(&mut presentation, 1739, true, &[]);
    assert_eq!(presentation.last_fishing_cues["1000"].water, [645, 655]);
    tick(&mut presentation, 1740, true, &["fishing.wav"]);
    assert_eq!(
        presentation.last_fishing_cues["1000"],
        NativeFishingCueFrames {
            point: (339, 247),
            water: [665, 671],
            actor: Some(1350),
        }
    );
    tick(&mut presentation, 2100, true, &[]);
    assert_eq!(presentation.last_fishing_cues["1000"].actor, Some(1351));

    assert!(adapter.observe_packet(&fishing_packet(1000, false, 100)));
    sync(&mut presentation, &adapter, &source, 2160);
    tick(&mut presentation, 2340, true, &[]);
    assert_eq!(presentation.last_fishing_cues["1000"].water, [670, 676]);
    tick(&mut presentation, 2440, true, &["fishing_pull.wav"]);
    assert_eq!(presentation.last_fishing_cues["1000"].actor, Some(1351));
    assert!(tick(&mut presentation, 2460, true, &[]).is_some());
    assert!(presentation.last_fishing_cues.is_empty());
    assert!(tick(&mut presentation, 2461, true, &[]).is_none());
}

#[test]
fn native_fishing_cues_catch_a_skipped_frame_one_without_restarting() {
    let mut adapter = NativeGameplayAdapter::default();
    let mut presentation = NativeEntityPresentation::default();
    let source = shore_payload();
    presentation.bind_fishing_audio_generation(7);
    adapter.observe_world_snapshot(&source);
    sync(&mut presentation, &adapter, &source, 0);
    assert!(adapter.observe_packet(&fishing_packet(1000, true, 0)));
    sync(&mut presentation, &adapter, &source, 100);

    let tick = |presentation: &mut NativeEntityPresentation, now_ms| {
        presentation.render_state_if_changed_with(now_ms, true, |_, _, _| Some(json!({})));
        std::mem::take(&mut presentation.fishing_sound_events)
            .into_iter()
            .map(|event| event.file_name)
            .collect::<Vec<_>>()
    };
    assert!(tick(&mut presentation, 100).is_empty());
    assert_eq!(tick(&mut presentation, 300), ["fishing_throw.wav"]);
    assert!(tick(&mut presentation, 400).is_empty());
    assert!(tick(&mut presentation, 900).is_empty());

    assert!(adapter.observe_packet(&fishing_packet(1000, true, 100)));
    sync(&mut presentation, &adapter, &source, 950);
    assert_eq!(tick(&mut presentation, 1140), ["fishing.wav"]);
    assert_eq!(presentation.last_fishing_cues["1000"].water, [666, 672]);
    assert!(tick(&mut presentation, 1260).is_empty());
    assert_eq!(presentation.last_fishing_cues["1000"].water, [667, 673]);
}

#[test]
#[ignore = "requires the verified Native asset root with exported Crystal fishing cues"]
fn native_fishing_offscreen_water_and_owner_marker_use_original_offsets_and_depth() {
    let payload = shore_payload();
    let mut state = json!({"entities":[{
        "objectId":"1000", "gridX":342, "gridY":244,
        "layers":[{"key":"1000:body"}],
    }]});
    let mut cues = HashMap::from([(
        "1000".to_owned(),
        NativeFishingCueFrames {
            point: (339, 247),
            water: [665, 671],
            actor: Some(1350),
        },
    )]);
    crate::atlas::append_native_fishing_cues(&payload, &mut state, &cues);
    let layers = &state["entities"][1]["layers"];
    let marker = &state["entities"][0]["layers"][1];
    assert_eq!(state["entities"][1]["objectId"], "fishing-water:1000");
    for (layer, path, left, top, w, h, additive) in [
        (
            &layers[0],
            "/original-effects/Effect/665.png",
            359.0,
            448.0,
            4,
            9,
            true,
        ),
        (
            &layers[1],
            "/original-effects/Effect/671.png",
            353.0,
            449.0,
            16,
            14,
            true,
        ),
        (
            marker,
            "/original-effects/Prguse/1350.png",
            490.0,
            263.0,
            28,
            32,
            false,
        ),
    ] {
        assert_eq!(layer["path"], path);
        assert_eq!(layer["left"].as_f64(), Some(left));
        assert_eq!(layer["top"].as_f64(), Some(top));
        assert_eq!(layer["width"], w);
        assert_eq!(layer["height"], h);
        assert_eq!(layer["additive"], additive);
    }
    assert!(layers[0]["z"].as_f64().unwrap() < layers[1]["z"].as_f64().unwrap());
    assert!(layers[1]["z"].as_f64().unwrap() < marker["z"].as_f64().unwrap());

    cues.clear();
    let mut clean = json!({"entities":[]});
    crate::atlas::append_native_fishing_cues(&payload, &mut clean, &cues);
    assert_eq!(clean["entities"], json!([]));
}

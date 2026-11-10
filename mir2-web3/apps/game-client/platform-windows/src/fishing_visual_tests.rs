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
    assert!(!adapter.observe_packet(&fishing_packet(1000, true, 100)));
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

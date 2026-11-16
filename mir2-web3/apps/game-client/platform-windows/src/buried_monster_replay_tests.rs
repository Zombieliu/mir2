//! CPU-only native replay of sanitized ordinary Wizard packets.
//!
//! The legacy/raw negative control deliberately reproduces the old public
//! projection leak. The paired expected projection is synthesized and labeled
//! in the fixture; it is not presented as a capture from a repaired server.
//! Production adapter, presentation and atlas routing are used below, with
//! synthetic atlas geometry and no GPU, input, service or account mutation.

use super::*;
use crate::gameplay_bridge::{NativeGameplayAdapter, NativeGameplaySnapshot};
use crate::native_protocol::{parse_inbound_value, InboundEvent};
use serde_json::json;

const PRIVATE_ZOMBIE: u32 = 248756;
const REVEALED_ZOMBIE: u32 = 248713;
const SELF_ID: &str = "1000";

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../tests/fixtures/vis03-buried-zombie2-wizard-20261001.json"
    ))
    .expect("sanitized buried Zombie2 replay fixture")
}

fn routing_manifest() -> Value {
    // Only routing and same-grid hover are tested. These rectangles do not
    // claim to be real Zombie2 bitmap dimensions or alpha-mask acceptance.
    crate::atlas::routing_atlas_manifest_fixture(&[
        "/original-ui/Monster/069/0.png",
        "/original-ui/Monster/069/4.png",
        "/original-ui/CArmour/01/0.png",
        "/original-ui/CArmour/01/28.png",
        "/original-ui/CHair/00/0.png",
        "/original-ui/CHair/00/28.png",
        "/original-ui/CWeapon/10/0.png",
        "/original-ui/CWeapon/10/28.png",
    ])
}

fn observe_fixture_packet(adapter: &mut NativeGameplayAdapter, entry: &Value) {
    match parse_inbound_value(entry["event"].clone()).expect("production packet parser") {
        InboundEvent::Packet(packet) => {
            // ObjectShow for this family is not itself an adapter read-model
            // change. Its real preceding ObjectMonster admits the actor.
            adapter.observe_packet(&packet);
        }
        other => panic!("fixture must decode as a packet, got {other:?}"),
    }
}

fn native_scene(
    adapter: &mut NativeGameplayAdapter,
    presentation: &mut NativeEntityPresentation,
    mut payload: Value,
    target_tile: (i32, i32),
) -> (Value, NativeGameplaySnapshot) {
    adapter.observe_world_snapshot_dispositions(&payload);
    adapter.observe_world_snapshot(&payload);
    adapter.apply_authoritative_overlay(&mut payload);
    let snapshot = adapter.snapshot(&payload);
    let center = &payload["sceneView"]["center"];
    let cursor = (
        ((target_tile.0 - center["x"].as_i64().unwrap() as i32 + 10) * 48 + 24) as f32,
        ((target_tile.1 - center["y"].as_i64().unwrap() as i32 + 11) * 32 + 16) as f32,
    );
    assert!((0.0..1024.0).contains(&cursor.0) && (0.0..768.0).contains(&cursor.1));
    presentation.replace_payload(payload);
    // Retain Crystal's compiled frame descriptors, independently of packaged
    // asset files. Atlas paths are still resolved by the production renderer.
    presentation.sync_pending_payload_with(0, |kind, _, _| AnimationCatalog::crystal_default(kind));
    presentation.set_hover_presentation(Some(cursor), false);
    let manifest = routing_manifest();
    let render = presentation
        .render_state_if_changed_with(0, true, |payload, poses, effect_visible| {
            crate::atlas::build_entity_render_state_with_manifest_for_test(
                payload,
                poses,
                effect_visible,
                &manifest,
            )
        })
        .expect("production native entity render projection");
    (render, snapshot)
}

fn assert_actor_surface(
    presentation: &NativeEntityPresentation,
    rendered: &Value,
    snapshot: &NativeGameplaySnapshot,
    actor_id: u32,
    target_tile: (i32, i32),
    present: bool,
) {
    let id = actor_id.to_string();
    let actor = rendered["entities"]
        .as_array()
        .expect("rendered actors")
        .iter()
        .find(|entity| entity["objectId"].as_str() == Some(id.as_str()));
    assert_eq!(actor.is_some(), present, "rendered actor membership");
    let has_body = actor.is_some_and(|entity| {
        entity["layers"].as_array().unwrap().iter().any(|layer| {
            layer["key"] == format!("{id}:body")
                && layer["path"]
                    .as_str()
                    .is_some_and(|path| path.starts_with("/original-ui/Monster/069/"))
        })
    });
    assert_eq!(has_body, present, "Zombie2 production body layer");
    assert_eq!(
        presentation.hovered_object_id() == Some(id.as_str()),
        present,
        "production same-grid hover identity"
    );
    assert_eq!(
        snapshot.world_click_state.targets.contains_key(&actor_id),
        present,
        "native world-click actor identity"
    );
    assert_eq!(
        presentation.tile_has_blocking_entity(SELF_ID, target_tile),
        Some(present),
        "live entity tile blocker"
    );
}

fn assert_combat_target(snapshot: &NativeGameplaySnapshot, target: Option<u32>) {
    assert_eq!(
        snapshot
            .combat_target
            .target
            .as_ref()
            .map(|target| target.object_id),
        target,
        "authoritative combat read model"
    );
}

fn assert_projection_pair(raw: &Value, projected: &Value, removed_id: u32) {
    assert_eq!(raw["provenance"]["kind"], "sanitized_raw_snapshot_slice");
    assert_eq!(
        projected["provenance"]["kind"],
        "expected_public_projection_fixture"
    );
    let mut expected = raw["payload"].clone();
    expected["entities"]
        .as_array_mut()
        .unwrap()
        .retain(|entity| entity["objectId"].as_u64() != Some(u64::from(removed_id)));
    assert_eq!(
        expected, projected["payload"],
        "omit only the private actor"
    );
}

#[test]
fn buried_zombie_raw_seq18577_documents_legacy_private_projection_leak() {
    let fixture = fixture();
    assert_eq!(
        fixture["rawSeq18577"]["provenance"]["sourceSequence"],
        18577
    );
    let mut adapter = NativeGameplayAdapter::default();
    let mut presentation = NativeEntityPresentation::default();
    let (rendered, snapshot) = native_scene(
        &mut adapter,
        &mut presentation,
        fixture["rawSeq18577"]["payload"].clone(),
        (40, 135),
    );
    assert_actor_surface(
        &presentation,
        &rendered,
        &snapshot,
        PRIVATE_ZOMBIE,
        (40, 135),
        true,
    );
    assert_combat_target(&snapshot, Some(PRIVATE_ZOMBIE));
}

#[test]
fn buried_zombie_projected_seq18577_removes_every_native_target_surface() {
    let fixture = fixture();
    assert_projection_pair(
        &fixture["rawSeq18577"],
        &fixture["projectedSeq18577"],
        PRIVATE_ZOMBIE,
    );
    let mut adapter = NativeGameplayAdapter::default();
    let mut presentation = NativeEntityPresentation::default();
    let (rendered, snapshot) = native_scene(
        &mut adapter,
        &mut presentation,
        fixture["projectedSeq18577"]["payload"].clone(),
        (40, 135),
    );
    assert_actor_surface(
        &presentation,
        &rendered,
        &snapshot,
        PRIVATE_ZOMBIE,
        (40, 135),
        false,
    );
    assert_combat_target(&snapshot, None);
}

#[test]
fn buried_zombie_real_public_reveal_and_synthetic_remove_or_map_controls() {
    let fixture = fixture();
    assert_projection_pair(
        &fixture["rawBeforeRealReveal"],
        &fixture["projectedBeforeRealReveal"],
        REVEALED_ZOMBIE,
    );
    // Cleanup branches start independently from the same genuine public
    // reveal. Removal cannot mask a stale actor in the map-transition branch.
    for cleanup in ["syntheticRemoveControl", "syntheticMapTransitionControl"] {
        assert_eq!(
            fixture[cleanup]["provenance"]["kind"],
            "synthetic_lifecycle_control"
        );
        let mut adapter = NativeGameplayAdapter::default();
        let mut presentation = NativeEntityPresentation::default();
        let before = fixture["projectedBeforeRealReveal"]["payload"].clone();
        let (rendered, snapshot) = native_scene(&mut adapter, &mut presentation, before, (29, 146));
        assert_actor_surface(
            &presentation,
            &rendered,
            &snapshot,
            REVEALED_ZOMBIE,
            (29, 146),
            false,
        );
        assert_combat_target(&snapshot, None);

        let location = &fixture["realWithinThreeUserLocation"]["event"]["payload"];
        let monster = &fixture["realObjectMonster"]["event"]["payload"];
        let dx =
            (location["x"].as_i64().unwrap() - monster["location"]["x"].as_i64().unwrap()).abs();
        let dy =
            (location["y"].as_i64().unwrap() - monster["location"]["y"].as_i64().unwrap()).abs();
        assert!(
            dx.max(dy) <= 3,
            "real owner receipt is inside Crystal reveal range"
        );
        assert_eq!(monster["objectId"], json!(REVEALED_ZOMBIE));
        assert_eq!(monster["hidden"], json!(false));
        assert_eq!(
            fixture["realObjectShow"]["event"]["payload"]["objectId"],
            json!(REVEALED_ZOMBIE)
        );
        for packet in [
            "realWithinThreeUserLocation",
            "realObjectMonster",
            "realObjectShow",
        ] {
            assert_eq!(fixture[packet]["provenance"]["kind"], "raw_received_packet");
            observe_fixture_packet(&mut adapter, &fixture[packet]);
        }
        for packet in fixture["realPostRevealOwnerLocations"].as_array().unwrap() {
            assert_eq!(packet["provenance"]["kind"], "raw_received_packet");
            observe_fixture_packet(&mut adapter, packet);
        }
        // The genuine post-reveal snapshot supplies HP and hostility; the
        // spawn packet itself does not fabricate those fields in the adapter.
        let public_payload = fixture["rawAfterRealReveal"]["payload"].clone();
        let (rendered, snapshot) = native_scene(
            &mut adapter,
            &mut presentation,
            public_payload.clone(),
            (29, 146),
        );
        assert_actor_surface(
            &presentation,
            &rendered,
            &snapshot,
            REVEALED_ZOMBIE,
            (29, 146),
            true,
        );
        assert_combat_target(&snapshot, Some(REVEALED_ZOMBIE));

        observe_fixture_packet(&mut adapter, &fixture[cleanup]);
        let cleaned_payload = if cleanup == "syntheticMapTransitionControl" {
            fixture[cleanup]["destinationPublicSnapshotFixture"].clone()
        } else {
            public_payload
        };
        let (rendered, snapshot) =
            native_scene(&mut adapter, &mut presentation, cleaned_payload, (29, 146));
        assert_actor_surface(
            &presentation,
            &rendered,
            &snapshot,
            REVEALED_ZOMBIE,
            (29, 146),
            false,
        );
        assert_combat_target(&snapshot, None);
        assert!(!snapshot.zone_entity_tiles.contains_key(&REVEALED_ZOMBIE));
        assert!(presentation
            .world
            .active_state(&REVEALED_ZOMBIE.to_string())
            .is_none());
    }
}

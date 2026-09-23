use super::*;

#[test]
fn self_occlusion_redraw_keeps_source_pose_and_crystal_parts_in_all_directions() {
    let payload = json!({"sceneView":{"center":{"x":19,"y":156}}});
    for direction in [
        "up",
        "upRight",
        "right",
        "downRight",
        "down",
        "downLeft",
        "left",
        "upLeft",
    ] {
        let mut layers = [
            "weapon-primary",
            "body",
            "hair",
            "wings",
            "mount",
            "shadow",
            "effect",
        ]
        .iter()
        .enumerate()
        .map(|(i, role)| {
            json!({
                "key":format!("1001:{role}"), "atlasKey":"actor-page",
                "atlasRectKey":format!("frame-{i}"), "left":42+i,"top":83+i,
                "width":48,"height":96,"z":entity_z_base(19,156)+i as f32,
                "path":format!("/original-ui/Actor/{i}.png")
            })
        })
        .collect::<Vec<_>>();
        let original = layers.clone();
        append_self_occlusion_redraw(&mut layers, original.len(), "1001", direction, &payload);
        assert_eq!(&layers[..original.len()], original.as_slice());
        let expected = if matches!(direction_index(direction), 0 | 1 | 2 | 6 | 7) {
            ["body", "hair", "wings"]
        } else {
            ["body", "wings", "hair"]
        };
        assert_eq!(layers.len(), original.len() + 3);
        for (redraw, role) in layers[original.len()..].iter().zip(expected) {
            let source = original
                .iter()
                .find(|v| v["key"] == format!("1001:{role}"))
                .unwrap();
            assert_eq!(redraw["key"], format!("1001:self-occlusion:{role}"));
            for field in [
                "atlasKey",
                "atlasRectKey",
                "left",
                "top",
                "width",
                "height",
                "path",
            ] {
                assert_eq!(redraw[field], source[field], "{direction} {role} {field}");
            }
            assert_eq!(redraw["opacity"], json!(0.4));
            assert_eq!(redraw["additive"], json!(role == "wings"));
            let z = redraw["z"].as_f64().unwrap() as f32;
            let (min, max) = post_world_depth_bounds(&payload);
            assert!(z > max && z < post_world_hover_z(&payload, min));
        }
    }
}

#[test]
fn self_occlusion_redraw_public_builder_survives_disabled_highlight_and_pose_changes() {
    let manifest = routing_atlas_manifest_fixture(&[
        "/original-ui/CArmour/00/0.png",
        "/original-ui/CArmour/00/1.png",
        "/original-ui/CHair/00/0.png",
        "/original-ui/CHair/00/1.png",
    ]);
    for frame in [0, 1] {
        let payload = json!({"sceneView":{"center":{"x":19,"y":156}},
            "_nativeHighlightTarget":false,"entities":[
            {"objectId":1001,"kind":"selfPlayer","x":19,"y":156,"direction":"up",
             "hidden":true,"sprite":{"bodyLibrary":"CArmour/00","hairLibrary":"CHair/00"}},
            {"objectId":1002,"kind":"player","x":20,"y":156,"direction":"up",
             "sprite":{"bodyLibrary":"CArmour/00"}}
        ]});
        let poses = HashMap::from([("1001".to_owned(), (frame, AnimationAction::Standing))]);
        let state =
            build_entity_render_state_with_manifest_for_test(&payload, &poses, false, &manifest)
                .unwrap();
        let layers = state["entities"][0]["layers"].as_array().unwrap();
        for role in ["body", "hair"] {
            let source = layers
                .iter()
                .find(|v| v["key"] == format!("1001:{role}"))
                .unwrap();
            let redraw = layers
                .iter()
                .find(|v| v["key"] == format!("1001:self-occlusion:{role}"))
                .unwrap();
            assert_eq!(source["atlasRectKey"], redraw["atlasRectKey"]);
            assert_eq!(source["opacity"], json!(0.5));
            assert_eq!(redraw["opacity"], json!(0.4));
            assert!(redraw["path"]
                .as_str()
                .unwrap()
                .ends_with(&format!("/{frame}.png")));
        }
        assert!(!state["entities"][1]["layers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["key"].as_str().unwrap().contains("self-occlusion")));
    }
}

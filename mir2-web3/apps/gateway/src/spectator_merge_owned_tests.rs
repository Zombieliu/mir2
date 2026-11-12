use super::*;

// Frozen pre-change algorithm: equality against this reference guards merge,
// fallback, duplicate-ID and malformed-object behavior independently of moves.
fn reference_merge(previous: Option<Value>, next: Value) -> Value {
    let Some(mut previous) = previous else {
        return next;
    };
    let Some(previous_object) = previous.as_object_mut() else {
        return next;
    };
    let Some(next_object) = next.as_object() else {
        return previous;
    };
    for (key, value) in next_object {
        if key != "entities" && key != "groundDrops" {
            previous_object.insert(key.clone(), value.clone());
        }
    }
    for field in ["entities", "groundDrops"] {
        let mut values = BTreeMap::<u64, Value>::new();
        for item in previous_object
            .get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .chain(
                next_object
                    .get(field)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten(),
            )
        {
            if let Some(id) = item.get("objectId").and_then(Value::as_u64) {
                values.insert(id, item.clone());
            }
        }
        previous_object.insert(
            field.to_string(),
            Value::Array(values.into_values().collect()),
        );
    }
    previous
}

#[test]
fn owned_merge_preserves_surviving_string_buffers() {
    let previous = json!({
        "entities": [{"objectId": 1, "name": "prior-entity-".repeat(512)}],
        "groundDrops": [{"objectId": 10, "name": "prior-drop-".repeat(512)}]
    });
    let next = json!({
        "sceneView": {"large": "scene-geometry-".repeat(512)},
        "entities": [{"objectId": 2, "name": "next-entity-".repeat(512)}],
        "groundDrops": [{"objectId": 11, "name": "next-drop-".repeat(512)}]
    });
    let prior_entity = previous["entities"][0]["name"].as_str().unwrap().as_ptr();
    let prior_drop = previous["groundDrops"][0]["name"]
        .as_str()
        .unwrap()
        .as_ptr();
    let scene = next["sceneView"]["large"].as_str().unwrap().as_ptr();
    let next_entity = next["entities"][0]["name"].as_str().unwrap().as_ptr();
    let next_drop = next["groundDrops"][0]["name"].as_str().unwrap().as_ptr();
    let merged = merge_worlds(Some(previous), next);
    assert_eq!(
        merged["sceneView"]["large"].as_str().unwrap().as_ptr(),
        scene
    );
    assert_eq!(
        merged["entities"][0]["name"].as_str().unwrap().as_ptr(),
        prior_entity
    );
    assert_eq!(
        merged["entities"][1]["name"].as_str().unwrap().as_ptr(),
        next_entity
    );
    assert_eq!(
        merged["groundDrops"][0]["name"].as_str().unwrap().as_ptr(),
        prior_drop
    );
    assert_eq!(
        merged["groundDrops"][1]["name"].as_str().unwrap().as_ptr(),
        next_drop
    );
}

#[test]
fn owned_merge_matches_reference_for_one_hundred_overlapping_viewports_and_events() {
    let (mut previous, mut reference) = (None, None);
    let (mut entity_seen, mut drop_seen) = (BTreeMap::new(), BTreeMap::new());
    for viewport in 0..100u64 {
        let at = viewport * 100;
        let mut entities = (0..8u64)
            .map(|offset| {
                let id = (viewport * 3 + offset) % 43;
                json!({"objectId": id, "name": format!("actor-{id}"), "kind": "player",
                    "x": viewport + offset, "y": offset, "hp": viewport % 17,
                    "dead": viewport % 11 == 0})
            })
            .collect::<Vec<_>>();
        entities.extend([
            json!({"objectId": (viewport * 3) % 43, "name": "last-duplicate-wins"}),
            json!({"objectId": -1}),
            json!({"name": "missing-id"}),
            Value::Null,
        ]);
        let next = json!({"tick": viewport, "mapFileName": "0",
            "sceneView": {"terrain": "public-geometry".repeat(100)}, "entities": entities,
            "groundDrops": [{"objectId": viewport % 29, "name": "drop", "x": viewport, "y": 2, "quantity": 1},
                {"objectId": viewport % 29, "name": "last-drop", "quantity": 3}],
            "retainedWhenMissing": viewport});
        record_seen_ids(&next, "entities", at, &mut entity_seen);
        record_seen_ids(&next, "groundDrops", at, &mut drop_seen);
        let before = previous.clone();
        let reference_before = reference.clone();
        let mut merged = merge_worlds(previous.take(), next.clone());
        let mut expected = reference_merge(reference.take(), next);
        let (mut expected_entity_seen, mut expected_drop_seen) =
            (entity_seen.clone(), drop_seen.clone());
        prune_stale_objects(&mut merged, "entities", at, 500, &mut entity_seen);
        prune_stale_objects(&mut merged, "groundDrops", at, 500, &mut drop_seen);
        prune_stale_objects(
            &mut expected,
            "entities",
            at,
            500,
            &mut expected_entity_seen,
        );
        prune_stale_objects(
            &mut expected,
            "groundDrops",
            at,
            500,
            &mut expected_drop_seen,
        );
        assert_eq!(merged, expected, "viewport {viewport}");
        assert_eq!(
            serde_json::to_vec(&merged).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
        assert_eq!(entity_seen, expected_entity_seen);
        assert_eq!(drop_seen, expected_drop_seen);
        assert_eq!(
            serde_json::to_value(world_events(before.as_ref(), &merged, at)).unwrap(),
            serde_json::to_value(world_events(reference_before.as_ref(), &expected, at)).unwrap()
        );
        previous = Some(merged);
        reference = Some(expected);
    }
}

#[test]
fn owned_merge_preserves_reference_fallbacks_and_invalid_arrays() {
    let values = [
        Value::Null,
        json!(3),
        json!("not-an-object"),
        json!([]),
        json!({}),
        json!({"entities": false, "groundDrops": null, "retained": "previous"}),
        json!({"entities": [{"objectId": 7, "name": "first"}, {"objectId": 7, "name": "last"},
            {"objectId": true}, {"objectId": "8"}, 4], "groundDrops": [{"objectId": 5}]}),
    ];
    for previous in std::iter::once(None).chain(values.iter().cloned().map(Some)) {
        for next in &values {
            assert_eq!(
                merge_worlds(previous.clone(), next.clone()),
                reference_merge(previous.clone(), next.clone())
            );
        }
    }
}

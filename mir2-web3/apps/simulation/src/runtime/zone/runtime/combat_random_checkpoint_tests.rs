use super::*;

fn legacy_fixture(version: u32) -> serde_json::Value {
    let runtime = ZoneRuntime::new(ZoneKey::for_map("combat-random-legacy"));
    let mut checkpoint: serde_json::Value =
        serde_json::from_slice(&runtime.checkpoint_bytes().unwrap()).unwrap();
    checkpoint["version"] = version.into();
    checkpoint["state_root"] = match version {
        1 => runtime.legacy_canonical_state_root(),
        2 => runtime.legacy_v2_canonical_state_root(),
        3 => runtime.legacy_v3_canonical_state_root(),
        4 => runtime.legacy_v4_canonical_state_root(),
        5 => runtime.legacy_v5_canonical_state_root(),
        6 => runtime.legacy_v6_canonical_state_root(),
        7 => runtime.legacy_v7_canonical_state_root(),
        _ => unreachable!(),
    }
    .unwrap()
    .into();
    checkpoint.as_object_mut().unwrap().remove("combat_random");
    checkpoint
}

#[test]
fn v8_checkpoint_and_live_fork_continue_the_exact_random_suffix() {
    let mut runtime = ZoneRuntime::new(ZoneKey::for_map("combat-random-v8"));
    runtime.combat_random = AuthoritativeCombatRandom::from_seed([42; 32]);
    for _ in 0..7 {
        runtime.combat_random.next_u32().unwrap();
    }
    let bytes = runtime.checkpoint_bytes().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["version"], 9);
    assert_eq!(value["combat_random"]["draws"], 7);
    let mut cold = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    let mut live = runtime.transaction_fork();
    assert_eq!(
        cold.canonical_state_root().unwrap(),
        runtime.canonical_state_root().unwrap()
    );
    assert_eq!(live.checkpoint_bytes().unwrap(), bytes);
    for _ in 0..32 {
        let expected = runtime.combat_random.next_u32().unwrap();
        assert_eq!(cold.combat_random.next_u32(), Some(expected));
        assert_eq!(live.combat_random.next_u32(), Some(expected));
        assert_eq!(
            cold.canonical_state_root().unwrap(),
            runtime.canonical_state_root().unwrap()
        );
        assert_eq!(
            live.checkpoint_bytes().unwrap(),
            runtime.checkpoint_bytes().unwrap()
        );
    }
}

#[test]
fn v8_root_authenticates_seed_and_position_and_requires_supported_complete_state() {
    let runtime = ZoneRuntime::new(ZoneKey::for_map("combat-random-integrity"));
    let value: serde_json::Value =
        serde_json::from_slice(&runtime.checkpoint_bytes().unwrap()).unwrap();
    for change in ["seed", "draws", "version", "missing", "null"] {
        let mut changed = value.clone();
        match change {
            "seed" => {
                changed["combat_random"]["seed"][0] =
                    ((changed["combat_random"]["seed"][0].as_u64().unwrap() + 1) % 256).into()
            }
            "draws" => changed["combat_random"]["draws"] = 1.into(),
            "version" => changed["combat_random"]["version"] = 99.into(),
            "missing" => {
                changed.as_object_mut().unwrap().remove("combat_random");
            }
            "null" => changed["combat_random"] = serde_json::Value::Null,
            _ => unreachable!(),
        }
        assert!(
            ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{change}"
        );
    }
}

#[test]
fn every_legacy_version_authenticates_its_original_root_then_mints_a_new_stream() {
    for version in 1..=7 {
        let fixture = legacy_fixture(version);
        for forward in [
            None,
            Some(serde_json::json!({"version":999,"seed":"malformed","draws":1234})),
        ] {
            let mut value = fixture.clone();
            if let Some(forward) = forward {
                value["combat_random"] = forward;
            }
            let restored =
                ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&value).unwrap()).unwrap();
            assert_eq!(
                restored.combat_random.draws(),
                0,
                "v{version} cannot adopt unauthenticated forward cursor"
            );
            let upgraded: serde_json::Value =
                serde_json::from_slice(&restored.checkpoint_bytes().unwrap()).unwrap();
            assert_eq!(upgraded["version"], 9);
            assert_eq!(upgraded["combat_random"]["version"], 1);
            let next =
                ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&upgraded).unwrap()).unwrap();
            assert_eq!(next.combat_random, restored.combat_random);
        }
        let mut tampered = fixture;
        tampered["next_object_id"] = 9_999.into();
        assert!(
            ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&tampered).unwrap()).is_err(),
            "v{version}"
        );
    }
}

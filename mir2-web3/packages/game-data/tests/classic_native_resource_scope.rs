use mir2_game_data::{
    crystal_monster_by_name, crystal_respawn_manifest, platinum_176_profile,
    platinum_176_profile_bundle, validate_content_profile, ContentMapRule, ContentProfile,
    ContentProfileBundleSummary,
};

#[test]
fn classic_runtime_has_208_maps_and_the_209th_room_is_resource_only() {
    let profile = platinum_176_profile();
    assert_eq!(profile.version, 27);
    assert_eq!(profile.map_whitelist.len(), 208);
    assert_eq!(profile.monster_whitelist.len(), 176);
    assert_eq!(profile.native_resource_only_maps, ["D71653"]);
    assert!(!profile
        .map_whitelist
        .iter()
        .any(|map| map.file_name == "D71653"));
    for name in [
        "D717", "D71625", "D71652", "D5069", "D514", "D10013", "D10032",
    ] {
        assert!(
            profile
                .map_whitelist
                .iter()
                .any(|map| map.file_name == name),
            "missing {name}"
        );
    }
    assert_eq!(validate_content_profile(&profile), Ok(()));
    let bundle = platinum_176_profile_bundle();
    assert_eq!(bundle.summary.maps, 208);
    assert_eq!(bundle.summary.native_resource_only_maps, 1);
    assert_eq!(bundle.summary.native_maps, 209);
    for (name, image, count) in [("EvilSnake", 49, 2), ("WhiteBoar0", 48, 28)] {
        assert!(profile
            .monster_whitelist
            .iter()
            .any(|monster| monster == name));
        assert_eq!(crystal_monster_by_name(name).unwrap().image, image);
        let map = crystal_respawn_manifest()
            .maps
            .into_iter()
            .find(|map| map.map_file_name == "D717")
            .unwrap();
        assert_eq!(
            map.respawns
                .iter()
                .find(|spawn| spawn.monster_name == name)
                .unwrap()
                .count,
            count
        );
    }
}

#[test]
fn resource_only_room_does_not_bypass_the_original_runtime_reachability_error() {
    let mut profile = platinum_176_profile();
    profile.map_whitelist.push(ContentMapRule {
        file_name: "D71653".into(),
        tier: "stone_tomb".into(),
        recommended_min_level: 34,
        recommended_max_level: 40,
    });
    let errors =
        validate_content_profile(&profile).expect_err("resource presence is not runtime admission");
    assert!(errors.iter().any(|error| error == "mapWhitelist map D71653 is not reachable from map 0 through whitelisted movements or visible NPC scripts"));
    assert!(errors
        .iter()
        .any(|error| error.contains("must not be runtime-admitted")));
}

#[test]
fn another_unreachable_runtime_branch_still_fails() {
    let mut profile = platinum_176_profile();
    profile
        .npc_script_whitelist
        .retain(|script| script != "BichonProvince/Sailor" && script != "PrajnaIsland/Sailor");
    let errors = validate_content_profile(&profile)
        .expect_err("an orphan asset must not excuse another branch");
    assert!(errors
        .iter()
        .any(|error| error.contains("mapWhitelist map 5 is not reachable")));
}

#[test]
fn only_the_canonical_named_resource_room_is_allowed() {
    for rooms in [
        vec!["D71652"],
        vec!["D71653", "D71653"],
        vec!["d71653"],
        vec!["../D71653"],
    ] {
        let mut profile = platinum_176_profile();
        profile.native_resource_only_maps = rooms.iter().map(|name| name.to_string()).collect();
        assert!(validate_content_profile(&profile)
            .unwrap_err()
            .iter()
            .any(|error| error.contains("nativeResourceOnlyMaps")));
    }
}

#[test]
fn metadata_and_summary_are_defaulted_for_older_json_consumers() {
    let mut value = serde_json::to_value(platinum_176_profile()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("nativeResourceOnlyMaps");
    let profile: ContentProfile = serde_json::from_value(value).unwrap();
    assert!(profile.native_resource_only_maps.is_empty());
    assert_eq!(validate_content_profile(&profile), Ok(()));
    let mut value = serde_json::to_value(platinum_176_profile_bundle().summary).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("nativeResourceOnlyMaps");
    value.as_object_mut().unwrap().remove("nativeMaps");
    let summary: ContentProfileBundleSummary = serde_json::from_value(value).unwrap();
    assert_eq!(summary.maps, 208);
    assert_eq!(summary.native_resource_only_maps, 0);
    assert_eq!(summary.native_maps, 0);
}

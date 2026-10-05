use super::*;
use serde_json::json;

#[test]
fn scene_reset_retains_connection_time_but_session_reset_does_not() {
    let mut environment = NativeLightingEnvironment::default();
    environment.observe_packet("TimeOfDay", &json!({"lights":4}));
    environment.observe_packet(
        "MapInformation",
        &json!({"fileName":"0","lights":3,"mapDarkLight":2}),
    );
    environment.reset_scene();
    assert_eq!(
        environment,
        NativeLightingEnvironment {
            time_of_day_light_setting: Some(4),
            ..Default::default()
        }
    );
    environment.reset_session();
    assert_eq!(environment, NativeLightingEnvironment::default());
}

#[test]
fn all_original_light_settings_and_darkness_values_survive() {
    for setting in 1..=4 {
        for darkness in 0..=4 {
            let mut environment = NativeLightingEnvironment::default();
            environment.observe_packet("TimeOfDay", &json!({"lights":setting}));
            environment.observe_packet(
                "MapInformation",
                &json!({"fileName":"0","lights":setting,"mapDarkLight":darkness}),
            );
            assert_eq!(environment.time_of_day_light_setting, Some(setting));
            assert_eq!(environment.map_light_setting, Some(setting));
            assert_eq!(environment.map_dark_light, darkness);
        }
    }
}

#[test]
fn invalid_time_clears_while_invalid_same_map_override_retains() {
    for invalid in [
        json!(-1),
        json!(0),
        json!(5),
        json!(1.5),
        json!("4"),
        json!(null),
    ] {
        let mut environment = NativeLightingEnvironment::default();
        environment.observe_packet("TimeOfDay", &json!({"lights":4}));
        environment.observe_packet(
            "MapInformation",
            &json!({"fileName":"0","lights":3,"mapDarkLight":2}),
        );
        environment.observe_packet("TimeOfDay", &json!({"lights":invalid}));
        environment.observe_packet(
            "NewMapInfo",
            &json!({"mapFileName":"0","lights":invalid,"mapDarkLight":5}),
        );
        assert_eq!(environment.time_of_day_light_setting, None);
        assert_eq!(environment.map_light_setting, Some(3));
        assert_eq!(environment.map_dark_light, 2);
    }
}

#[test]
fn invalid_same_map_darkness_retains_the_previous_valid_value() {
    for invalid in [json!(-1), json!(5), json!(1.5), json!("4"), json!(null)] {
        let mut environment = NativeLightingEnvironment::default();
        environment.observe_packet("MapInformation", &json!({"fileName":"0","mapDarkLight":2}));
        environment.observe_packet(
            "NewMapInfo",
            &json!({"mapFileName":"0","mapDarkLight":invalid}),
        );
        assert_eq!(environment.map_dark_light, 2);
    }
}

#[test]
fn normalized_map_aliases_do_not_clear_map_overrides() {
    let mut environment = NativeLightingEnvironment::default();
    environment.observe_packet(
        "MapInformation",
        &json!({"fileName":"Data\\\\Maps\\\\BICHON.MAP","lights":4,"mapDarkLight":3}),
    );
    for alias in ["bichon", " Bichon.map ", "Data/Maps/bichon.map"] {
        environment.observe_world_snapshot(&json!({"mapFileName":alias}));
        assert_eq!(environment.map_light_setting, Some(4));
        assert_eq!(environment.map_dark_light, 3);
    }
    assert!(!same_map_file_name("0", "1"));
}

#[test]
fn snapshot_changes_map_and_only_a_valid_snapshot_time_replaces_time() {
    let mut environment = NativeLightingEnvironment::default();
    environment.observe_packet("TimeOfDay", &json!({"lights":4}));
    environment.observe_packet(
        "MapInformation",
        &json!({"fileName":"0","lights":3,"mapDarkLight":2}),
    );
    environment.observe_world_snapshot(&json!({"mapFileName":"1","lightSetting":0}));
    assert_eq!(environment.current_map_file_name.as_deref(), Some("1"));
    assert_eq!(environment.map_light_setting, None);
    assert_eq!(environment.map_dark_light, 0);
    assert_eq!(environment.time_of_day_light_setting, Some(4));
    environment.observe_world_snapshot(&json!({"lightSetting":2}));
    assert_eq!(environment.time_of_day_light_setting, Some(2));
    assert_eq!(environment.current_map_file_name.as_deref(), Some("1"));
}

#[test]
fn three_original_map_packets_have_identical_environment_semantics() {
    for packet in ["MapInformation", "MapChanged", "NewMapInfo"] {
        let mut environment = NativeLightingEnvironment::default();
        environment.observe_packet("TimeOfDay", &json!({"lights":4}));
        environment.observe_packet(
            packet,
            &json!({"mapFileName":"0","lights":1,"mapDarkLight":4}),
        );
        assert_eq!(environment.current_map_file_name.as_deref(), Some("0"));
        assert_eq!(environment.map_light_setting, Some(1));
        assert_eq!(environment.map_dark_light, 4);
        assert_eq!(environment.time_of_day_light_setting, Some(4));
        environment.observe_packet(packet, &json!({"fileName":"1","lights":0,"mapDarkLight":5}));
        assert_eq!(environment.map_light_setting, None);
        assert_eq!(environment.map_dark_light, 0);
        assert_eq!(environment.time_of_day_light_setting, Some(4));
    }
}

#[test]
fn nested_payload_and_file_name_priority_match_the_original_producer() {
    let mut environment = NativeLightingEnvironment::default();
    environment.observe_packet(
        "MapInformation",
        &json!({"payload":{"fileName":"0","mapFileName":"1","lights":3,"mapDarkLight":2}}),
    );
    assert_eq!(environment.current_map_file_name.as_deref(), Some("0"));
    assert_eq!(environment.map_light_setting, Some(3));
    environment.observe_packet("TimeOfDay", &json!({"payload":{"lights":4}}));
    assert_eq!(environment.time_of_day_light_setting, Some(4));
}

#[test]
fn missing_or_empty_map_and_unknown_packets_do_not_invent_state() {
    let mut environment = NativeLightingEnvironment::default();
    environment.observe_packet(
        "MapInformation",
        &json!({"fileName":"0","lights":3,"mapDarkLight":2}),
    );
    let baseline = environment.clone();
    environment.observe_world_snapshot(&json!({"mapFileName":"","lightSetting":5}));
    environment.observe_packet(
        "Unknown",
        &json!({"fileName":"1","lights":4,"mapDarkLight":4}),
    );
    assert_eq!(environment, baseline);
}

#[test]
fn logout_removes_all_environment_without_a_new_protocol() {
    let mut environment = NativeLightingEnvironment::default();
    environment.observe_packet("TimeOfDay", &json!({"lights":4}));
    environment.observe_packet(
        "NewMapInfo",
        &json!({"mapFileName":"0","lights":3,"mapDarkLight":2}),
    );
    environment.observe_packet("LogOutSuccess", &json!({}));
    assert_eq!(environment, NativeLightingEnvironment::default());
}

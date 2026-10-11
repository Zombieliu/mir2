use mir2_game_data::CrystalItemTemplate;
use mir2_simulation::WorldItemTooltipSource;
use serde_json::json;

// Data-only wire fixture: no catalogue assets, session or runtime is needed.
fn template(item_index: i32) -> CrystalItemTemplate {
    CrystalItemTemplate {
        item_index,
        name: format!("Wire item {item_index}"),
        item_type: 12,
        grade: 0,
        required_type: 0,
        required_class: 31,
        required_gender: 3,
        item_set: 0,
        shape: 0,
        weight: 1,
        light: 38,
        required_amount: 0,
        image: 130,
        durability: 8000,
        stack_size: 1,
        price: 10,
        start_item: true,
        effect: 0,
        need_identify: false,
        show_group_pickup: false,
        class_based: false,
        level_based: false,
        can_mine: false,
        global_drop_notify: false,
        bind: 0,
        unique: 0,
        random_stats_id: 0,
        can_fast_run: false,
        can_awakening: false,
        slots: 0,
        stats: Vec::new(),
        tooltip: None,
    }
}

fn source() -> WorldItemTooltipSource {
    WorldItemTooltipSource {
        info: template(722),
        real_info: None,
        user_item: None,
        socket_infos: Vec::new(),
        real_socket_infos: Vec::new(),
    }
}

#[test]
fn tooltip_wire_emits_explicit_empty_socket_arrays() {
    let source = source();
    let wire = serde_json::to_value(&source).expect("serialize tooltip source");
    assert_eq!(wire.get("socketInfos"), Some(&json!([])));
    assert_eq!(wire.get("realSocketInfos"), Some(&json!([])));
    assert_eq!(wire["info"], serde_json::to_value(&source.info).unwrap());
    assert!(wire.get("realInfo").is_none());
    assert!(wire.get("userItem").is_none());
    assert_eq!(serde_json::from_value::<WorldItemTooltipSource>(wire).unwrap(), source);
}

#[test]
fn tooltip_wire_preserves_nonempty_socket_positions_and_template_rows() {
    let mut source = source();
    source.socket_infos = vec![None, Some(template(658)), None];
    source.real_socket_infos = vec![None, Some(template(659)), None];
    let wire = serde_json::to_value(&source).expect("serialize positional sockets");
    assert_eq!(wire["socketInfos"], json!([null, template(658), null]));
    assert_eq!(wire["realSocketInfos"], json!([null, template(659), null]));
    assert_eq!(serde_json::from_value::<WorldItemTooltipSource>(wire).unwrap(), source);
}

#[test]
fn tooltip_wire_legacy_omitted_socket_arrays_still_deserialize_empty() {
    let source = source();
    let mut legacy = serde_json::to_value(&source).unwrap();
    let object = legacy.as_object_mut().unwrap();
    object.remove("socketInfos");
    object.remove("realSocketInfos");
    let decoded: WorldItemTooltipSource = serde_json::from_value(legacy).expect("legacy tooltip source");
    assert_eq!(decoded, source);
    let canonical = serde_json::to_value(decoded).unwrap();
    assert_eq!(canonical.get("socketInfos"), Some(&json!([])));
    assert_eq!(canonical.get("realSocketInfos"), Some(&json!([])));
}

use mir2_protocol::{ClientPacket, MirDirection, ServerPacket};
use mir2_simulation::VisibleNpcRecord;
use mir2_simulation::{SimulationConfig, SimulationSession};
use serde_json::json;
use serde_json::Value;

const NPC: u32 = 49_990;
fn config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: NPC,
        name: "Refine test smith".into(),
        image: 1,
        colour_argb: -1,
        position: mir2_protocol::Point {
            x: config.spawn.x + 1,
            y: config.spawn.y,
        },
        direction: MirDirection::Down,
        quest_ids: vec![],
        script_key: Some("MongchonProvince/SabukWall/Blacksmith1".into()),
    });
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&0)
        .unwrap()
        .gold = 1_000_000;
    config
}
fn login(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut save = session.active_character_checkpoint().unwrap();
    save.position = config.spawn.clone();
    session.restore_active_character_checkpoint(&save).unwrap();
    session
}
fn bag(session: &SimulationSession) -> Vec<Value> {
    session
        .active_character_checkpoint()
        .unwrap()
        .inventory_items_json
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
fn call(session: &mut SimulationSession, label: &str) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::CallNpc {
        object_id: NPC,
        key: label.into(),
    })
}
fn target(session: &SimulationSession) -> Value {
    bag(session)
        .into_iter()
        .find(|v| v["key"] == "dagger")
        .unwrap()
}
fn start(session: &mut SimulationSession) -> u64 {
    let uid = target(session)["unique_id"].as_u64().unwrap();
    assert!(call(session, "@Refine")
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCRefine { rate, .. } if *rate == 125.0)));
    let packets = session.handle_packet(ClientPacket::RefineItem { unique_id: uid });
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::RefineItem { unique_id } if *unique_id == uid)),
        "{packets:?}"
    );
    uid
}
fn edit_refine(session: &mut SimulationSession, edit: impl FnOnce(&mut Value)) {
    let mut save = session.active_character_checkpoint().unwrap();
    let mut systems: Value =
        serde_json::from_str(save.stage5_systems_json.as_ref().unwrap()).unwrap();
    edit(&mut systems["refine"]);
    save.stage5_systems_json = Some(systems.to_string());
    session.restore_active_character_checkpoint(&save).unwrap();
}
fn make_due(session: &mut SimulationSession) {
    edit_refine(session, |state| {
        state["remainingMs"] = json!(0);
        state["clockEpoch"] = json!("previous-server-test-fixture");
        state["collectDeadlineMs"] = json!(1);
    });
}
fn identity(mut item: Value) -> Value {
    item.as_object_mut().unwrap().remove("container");
    item.as_object_mut().unwrap().remove("slot");
    item
}

#[test]
fn oven_roundtrip_reload_preserves_target_and_debits_fee_once() {
    let config = config();
    let mut s = login(&config);
    let mut save = s.active_character_checkpoint().unwrap();
    for encoded in &mut save.inventory_items_json {
        let mut item: Value = serde_json::from_str(encoded).unwrap();
        if item["key"] == "dagger" {
            item["unique_id"] = json!(910_001);
            item["durability_current"] = json!(7);
            item["durability_max"] = json!(31);
            item["added_attack"] = json!(4);
            item["gem_count"] = json!(3);
            item["description"] = json!("refine oven instance fixture");
            *encoded = item.to_string();
        }
    }
    s.restore_active_character_checkpoint(&save).unwrap();
    let before_gold = s.world_snapshot().gold;
    let uid = start(&mut s);
    let after_gold = s.world_snapshot().gold;
    assert!(after_gold < before_gold);
    let held = s
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json
        .unwrap();
    let held_item: Value = serde_json::from_str(&held).unwrap();
    assert_eq!(held_item["durability_current"], 7);
    assert_eq!(held_item["durability_max"], 31);
    assert_eq!(held_item["added_attack"], 4);
    s.handle_packet(ClientPacket::RefineItem { unique_id: uid });
    s.handle_packet(ClientPacket::CheckRefine { unique_id: uid });
    assert_eq!(s.world_snapshot().gold, after_gold);
    assert!(bag(&s).iter().all(|item| item["unique_id"] != uid));
    s.save_active_character().unwrap();
    let mut s = login(&config);
    assert_eq!(s.world_snapshot().gold, after_gold);
    assert_eq!(
        s.world_snapshot()
            .stage5_systems
            .refine
            .oven_item_state_json
            .as_deref(),
        Some(held.as_str())
    );
    assert!(call(&mut s, "@Refine")
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCRefine { refining: true, .. })));
    make_due(&mut s);
    let packets = call(&mut s, "@RefineCollect");
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCCollectRefine { success: true })));
    let returned = target(&s);
    assert_eq!(identity(returned), identity(held_item));
    assert!(s
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json
        .is_none());
    let count = bag(&s).len();
    call(&mut s, "@RefineCollect");
    assert_eq!(bag(&s).len(), count);
}

#[test]
fn full_bag_collection_keeps_the_oven_target_until_retry() {
    let config = config();
    let mut s = login(&config);
    let mut sample = target(&s);
    let uid = start(&mut s);
    make_due(&mut s);
    let held = s
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json;
    let mut save = s.active_character_checkpoint().unwrap();
    save.inventory_items_json = (0..save.inventory_capacity - 6)
        .map(|slot| {
            sample["unique_id"] = json!(920_000 + u64::from(slot));
            sample["slot"] = json!(slot % 40);
            sample["container"] = json!(if slot < 40 { "bag1" } else { "bag2" });
            sample.to_string()
        })
        .collect();
    s.restore_active_character_checkpoint(&save).unwrap();
    assert!(call(&mut s, "@RefineCollect")
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCCollectRefine { success: false })));
    assert_eq!(
        s.world_snapshot()
            .stage5_systems
            .refine
            .oven_item_state_json,
        held
    );
    s.save_active_character().unwrap();
    let mut s = login(&config);
    let mut save = s.active_character_checkpoint().unwrap();
    save.inventory_items_json.pop();
    s.restore_active_character_checkpoint(&save).unwrap();
    assert!(call(&mut s, "@RefineCollect")
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCCollectRefine { success: true })));
    assert_eq!(bag(&s).iter().filter(|v| v["unique_id"] == uid).count(), 1);
}

#[test]
fn zero_duration_auto_collects_but_check_requires_the_real_check_service() {
    let mut config = config();
    config.refine_duration_ms = 0;
    let mut s = login(&config);
    let uid = start(&mut s);
    assert!(s
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json
        .is_none());
    assert_eq!(target(&s)["user_item_metadata"]["refine_added"], 1);
    s.handle_packet(ClientPacket::CheckRefine { unique_id: uid });
    assert_eq!(target(&s)["user_item_metadata"]["refine_added"], 1);
    call(&mut s, "@RefineCheck");
    let packets = s.handle_packet(ClientPacket::CheckRefine { unique_id: uid });
    // No ingredients is a valid attempt with guaranteed failure.
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::RefineItem { unique_id } if *unique_id == uid)));
    assert!(bag(&s).iter().all(|v| v["unique_id"] != uid));
    assert!(!s
        .handle_packet(ClientPacket::CheckRefine { unique_id: uid })
        .iter()
        .any(|p| matches!(p, ServerPacket::RefineItem { .. })));
}

#[test]
fn forged_start_wrong_service_and_insufficient_gold_preserve_every_asset() {
    let config = config();
    let mut s = login(&config);
    let uid = target(&s)["unique_id"].as_u64().unwrap();
    let before = bag(&s);
    let gold = s.world_snapshot().gold;
    s.handle_packet(ClientPacket::RefineItem { unique_id: uid });
    assert_eq!(bag(&s), before);
    assert_eq!(s.world_snapshot().gold, gold);
    call(&mut s, "@Refine");
    let mut save = s.active_character_checkpoint().unwrap();
    save.gold = 0;
    s.restore_active_character_checkpoint(&save).unwrap();
    call(&mut s, "@Refine");
    s.handle_packet(ClientPacket::RefineItem { unique_id: uid });
    assert_eq!(bag(&s), before);
    assert_eq!(s.world_snapshot().gold, 0);
    assert!(s
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json
        .is_none());
}

#[test]
fn malformed_or_duplicate_oven_targets_reject_restore_without_mutating_live_state() {
    let config = config();
    let mut s = login(&config);
    start(&mut s);
    let original = s.active_character_checkpoint().unwrap();
    let held = s
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json
        .clone()
        .unwrap();
    for malformed in [false, true] {
        let mut save = original.clone();
        if malformed {
            let mut state: Value =
                serde_json::from_str(save.stage5_systems_json.as_ref().unwrap()).unwrap();
            state["refine"]["ovenItemStateJson"] = json!("{invalid");
            save.stage5_systems_json = Some(state.to_string());
        } else {
            save.inventory_items_json.push(held.clone());
        }
        assert!(s.restore_active_character_checkpoint(&save).is_err());
        assert_eq!(
            s.world_snapshot()
                .stage5_systems
                .refine
                .oven_item_state_json
                .as_deref(),
            Some(held.as_str())
        );
    }
}

#[test]
fn ordinary_refine_removes_weapon_from_bag_until_npc_collection() {
    let config = config();
    let mut session = login(&config);
    let target = bag(&session)
        .into_iter()
        .find(|v| v["key"] == "dagger")
        .unwrap();
    let uid = target["unique_id"].as_u64().unwrap();
    let opened = call(&mut session, "@Refine");
    assert!(
        opened
            .iter()
            .any(|p| matches!(p, ServerPacket::NPCRefine { .. })),
        "packets={opened:?}; npc={:?}",
        session
            .world_snapshot()
            .entities
            .iter()
            .find(|e| e.object_id == NPC)
    );
    let packets = session.handle_packet(ClientPacket::RefineItem { unique_id: uid });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::RefineItem { unique_id } if *unique_id == uid)));
    assert!(
        bag(&session).iter().all(|v| v["unique_id"] != uid),
        "the oven must exclusively own the target"
    );
    assert!(call(&mut session, "@RefineCollect")
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCCollectRefine { success: false })));
}

#[test]
fn collected_refinement_survives_reload_and_success_is_applied_only_once() {
    let mut config = config();
    config.refine_duration_ms = 0;
    let mut s = login(&config);
    let uid = start(&mut s);
    let mut save = s.active_character_checkpoint().unwrap();
    for encoded in &mut save.inventory_items_json {
        let mut item: Value = serde_json::from_str(encoded).unwrap();
        if item["unique_id"] == uid {
            item["user_item_metadata"]["refined_value"] = json!(1);
            item["user_item_metadata"]["refine_success_chance"] = json!(100);
            *encoded = item.to_string();
        }
    }
    s.restore_active_character_checkpoint(&save).unwrap();
    s.save_active_character().unwrap();
    let mut s = login(&config);
    let before = target(&s)["added_attack"].as_i64().unwrap();
    call(&mut s, "@Refine");
    let gold = s.world_snapshot().gold;
    s.handle_packet(ClientPacket::RefineItem { unique_id: uid });
    assert_eq!(s.world_snapshot().gold, gold);
    assert!(s
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json
        .is_none());
    call(&mut s, "@RefineCheck");
    let packets = s.handle_packet(ClientPacket::CheckRefine { unique_id: uid });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::ItemUpgraded { .. })));
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::RefineItem { .. })));
    let after = target(&s);
    assert!(after["added_attack"].as_i64().unwrap() > before);
    assert_eq!(after["user_item_metadata"]["refine_added"], 0);
    assert_eq!(after["user_item_metadata"]["refine_success_chance"], 0);
    s.handle_packet(ClientPacket::CheckRefine { unique_id: uid });
    assert_eq!(target(&s), after);
}

#[test]
fn legacy_pending_migrates_existing_target_and_missing_target_fails_closed() {
    let config = config();
    let mut s = login(&config);
    let uid = target(&s)["unique_id"].as_u64().unwrap();
    let original = s.active_character_checkpoint().unwrap();
    let mut save = original.clone();
    let mut state: Value =
        serde_json::from_str(save.stage5_systems_json.as_ref().unwrap()).unwrap();
    state["refine"]["currentItem"] = json!("dagger");
    state["refine"]["refining"] = json!(true);
    state["refine"]["ready"] = json!(true);
    state["refine"]["pendingUniqueId"] = json!(uid);
    state["refine"]["pendingChance"] = json!(75);
    state["refine"]["pendingStat"] = json!(0);
    save.stage5_systems_json = Some(state.to_string());
    let mut missing = save.clone();
    missing
        .inventory_items_json
        .retain(|v| serde_json::from_str::<Value>(v).unwrap()["unique_id"] != uid);
    assert!(s.restore_active_character_checkpoint(&missing).is_err());
    assert_eq!(
        s.active_character_checkpoint()
            .unwrap()
            .inventory_items_json,
        original.inventory_items_json
    );
    s.restore_active_character_checkpoint(&save).unwrap();
    assert_eq!(target(&s)["user_item_metadata"]["refine_added"], 1);
    assert_eq!(
        target(&s)["user_item_metadata"]["refine_success_chance"],
        75
    );
    assert_eq!(
        s.world_snapshot().stage5_systems.refine.pending_unique_id,
        0
    );
    assert!(!s.world_snapshot().stage5_systems.refine.refining);
}

#[test]
fn malformed_clock_is_rejected_and_ready_flag_cannot_bypass_deadline() {
    let config = config();
    let mut s = login(&config);
    start(&mut s);
    let original = s.active_character_checkpoint().unwrap();
    let mut invalid = original.clone();
    let mut state: Value =
        serde_json::from_str(invalid.stage5_systems_json.as_ref().unwrap()).unwrap();
    state["refine"]["clockEpoch"] = Value::Null;
    invalid.stage5_systems_json = Some(state.to_string());
    assert!(s.restore_active_character_checkpoint(&invalid).is_err());
    edit_refine(&mut s, |state| state["ready"] = json!(true));
    assert!(call(&mut s, "@RefineCollect")
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCCollectRefine { success: false })));
    assert!(s
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json
        .is_some());
}

#[test]
fn collecting_one_pending_weapon_does_not_block_a_second_oven_job() {
    let config = config();
    let mut s = login(&config);
    let first = start(&mut s);
    make_due(&mut s);
    call(&mut s, "@RefineCollect");
    let first_item = target(&s);
    let mut save = s.active_character_checkpoint().unwrap();
    let mut second = first_item.clone();
    second["unique_id"] = json!(940_002);
    second["slot"] = json!(30);
    second["user_item_metadata"]["refine_added"] = json!(0);
    second["user_item_metadata"]["refined_value"] = json!(0);
    second["user_item_metadata"]["refine_success_chance"] = json!(0);
    save.inventory_items_json.push(second.to_string());
    s.restore_active_character_checkpoint(&save).unwrap();
    call(&mut s, "@Refine");
    let packets = s.handle_packet(ClientPacket::RefineItem { unique_id: 940_002 });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::RefineItem { unique_id: 940_002 })));
    assert_eq!(
        bag(&s).iter().find(|v| v["unique_id"] == first).unwrap(),
        &first_item
    );
    assert!(bag(&s).iter().all(|v| v["unique_id"] != 940_002));
    make_due(&mut s);
    call(&mut s, "@RefineCollect");
    assert_eq!(
        bag(&s)
            .iter()
            .filter(|v| v["user_item_metadata"]["refine_added"] == 1)
            .count(),
        2
    );
}

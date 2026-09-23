//! Ordinary player packet custody must preserve complete item instances.
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession};
use serde_json::{json, Value};

fn open_refine(s: &mut SimulationSession) {
    let packets = s.handle_packet(ClientPacket::CallNpc {
        object_id: 49_990,
        key: "@Refine".into(),
    });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCRefine { .. })));
}
fn started(config: &SimulationConfig) -> SimulationSession {
    let mut config = config.clone();
    config.visible_npcs.push(mir2_simulation::VisibleNpcRecord {
        object_id: 49_990,
        name: "Custody test smith".into(),
        image: 1,
        colour_argb: -1,
        position: mir2_protocol::Point {
            x: config.spawn.x + 1,
            y: config.spawn.y,
        },
        direction: mir2_protocol::MirDirection::Down,
        quest_ids: vec![],
        script_key: Some("MongchonProvince/SabukWall/Blacksmith1".into()),
    });
    let spawn = config.spawn.clone();
    let mut s = SimulationSession::new(config);
    assert!(s
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    s.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut save = s.active_character_checkpoint().unwrap();
    save.position = spawn;
    s.restore_active_character_checkpoint(&save).unwrap();
    open_refine(&mut s);
    s
}
fn items(s: &SimulationSession) -> Vec<Value> {
    s.active_character_checkpoint()
        .unwrap()
        .inventory_items_json
        .iter()
        .map(|v| serde_json::from_str(v).unwrap())
        .collect()
}
fn identity(mut v: Value) -> Value {
    v.as_object_mut().unwrap().remove("slot");
    v.as_object_mut().unwrap().remove("container");
    v
}
fn fixture(s: &mut SimulationSession, key: &str, uid: u64) -> Value {
    let mut checkpoint = s.active_character_checkpoint().unwrap();
    for encoded in &mut checkpoint.inventory_items_json {
        let mut item: Value = serde_json::from_str(encoded).unwrap();
        if item["key"] != key {
            continue;
        }
        item["unique_id"] = json!(uid);
        item["description"] = json!("custody roundtrip fixture");
        item["added_stats"] = json!([{"stat": 11, "value": 3}]);
        item["identified"] = json!(false);
        item["gem_count"] = json!(4);
        if key == "dagger" {
            item["durability_current"] = json!(7);
            item["durability_max"] = json!(23);
            item["added_attack"] = json!(5);
        } else {
            item["quantity"] = json!(17);
        }
        *encoded = item.to_string();
    }
    s.restore_active_character_checkpoint(&checkpoint).unwrap();
    open_refine(s);
    items(s)
        .into_iter()
        .find(|v| v["unique_id"] == uid)
        .unwrap()
}
fn consign(s: &mut SimulationSession, uid: u64) -> u64 {
    assert!(s
        .handle_packet(ClientPacket::ConsignItem {
            unique_id: uid,
            price: 77,
            market_type: 0
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::ConsignItem { success: true, .. })));
    s.world_snapshot()
        .stage5_systems
        .auction
        .iter()
        .map(|l| u64::from(l.id))
        .max()
        .unwrap()
}
fn fill_bag(s: &mut SimulationSession, sample: &Value) {
    let mut save = s.active_character_checkpoint().unwrap();
    save.inventory_items_json = (0..save.inventory_capacity - 6)
        .map(|slot| {
            let mut item = sample.clone();
            item["unique_id"] = json!(800_000 + u64::from(slot));
            item["slot"] = json!(slot % 40);
            item["container"] = json!(if slot < 40 { "bag1" } else { "bag2" });
            item.to_string()
        })
        .collect();
    s.restore_active_character_checkpoint(&save).unwrap();
}
#[test]
fn full_bag_keeps_market_and_refine_custody_until_space_is_available() {
    for market in [false, true] {
        let config = SimulationConfig::default();
        let mut s = started(&config);
        let before = fixture(&mut s, "dagger", 700_003);
        let id = if market {
            consign(&mut s, 700_003)
        } else {
            s.handle_packet(ClientPacket::DepositRefineItem {
                from: before["slot"].as_i64().unwrap() as i32,
                to: 2,
            });
            0
        };
        fill_bag(&mut s, &before);
        let held = s.world_snapshot().stage5_systems;
        let bag = items(&s);
        if market {
            s.handle_packet(ClientPacket::MarketGetBack {
                mode: 0,
                auction_id: id,
            });
        } else {
            s.handle_packet(ClientPacket::RefineCancel);
        }
        assert_eq!(items(&s), bag);
        assert_eq!(s.world_snapshot().stage5_systems, held);
        s.save_active_character().unwrap();
        let mut s = started(&config);
        let mut save = s.active_character_checkpoint().unwrap();
        save.inventory_items_json.pop();
        s.restore_active_character_checkpoint(&save).unwrap();
        if market {
            s.handle_packet(ClientPacket::MarketGetBack {
                mode: 0,
                auction_id: id,
            });
        } else {
            s.handle_packet(ClientPacket::RefineCancel);
        }
        assert_eq!(
            identity(
                items(&s)
                    .into_iter()
                    .find(|v| v["unique_id"] == 700_003)
                    .unwrap()
            ),
            identity(before)
        );
    }
}
#[test]
fn legacy_key_only_custody_never_creates_replacement_items() {
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let before = fixture(&mut s, "dagger", 700_004);
    let id = consign(&mut s, 700_004);
    let mut save = s.active_character_checkpoint().unwrap();
    let mut state: Value =
        serde_json::from_str(save.stage5_systems_json.as_ref().unwrap()).unwrap();
    // Use the typed carrier so its serialized field naming remains explicit.
    let mut systems = s.world_snapshot().stage5_systems;
    systems
        .auction
        .iter_mut()
        .find(|l| u64::from(l.id) == id)
        .unwrap()
        .item_state_json = None;
    systems
        .refine
        .slots
        .insert(2, before["key"].as_str().unwrap().into());
    state.clone_from(&serde_json::to_value(systems).unwrap());
    save.stage5_systems_json = Some(state.to_string());
    s.restore_active_character_checkpoint(&save).unwrap();
    let bag = items(&s);
    let held = s.world_snapshot().stage5_systems;
    s.handle_packet(ClientPacket::MarketGetBack {
        mode: 0,
        auction_id: id,
    });
    s.handle_packet(ClientPacket::RefineCancel);
    s.handle_packet(ClientPacket::RetrieveRefineItem { from: 2, to: 0 });
    assert_eq!(items(&s), bag);
    assert_eq!(s.world_snapshot().stage5_systems, held);
}
#[test]
fn conflicting_custody_uid_rejects_restore_without_mutation() {
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let before = fixture(&mut s, "dagger", 700_005);
    consign(&mut s, 700_005);
    let mut save = s.active_character_checkpoint().unwrap();
    save.inventory_items_json.push(before.to_string());
    let bag = items(&s);
    let held = s.world_snapshot().stage5_systems;
    assert!(s.restore_active_character_checkpoint(&save).is_err());
    assert_eq!(items(&s), bag);
    assert_eq!(s.world_snapshot().stage5_systems, held);
}
#[test]
fn buying_exact_listing_is_atomic_and_charges_only_once() {
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let before = fixture(&mut s, "dagger", 700_006);
    let id = consign(&mut s, 700_006);
    // An isolated foreign listing fixture; this does not simulate a shared market.
    let mut save = s.active_character_checkpoint().unwrap();
    let mut systems = s.world_snapshot().stage5_systems;
    systems
        .auction
        .iter_mut()
        .find(|l| u64::from(l.id) == id)
        .unwrap()
        .seller = "OtherSeller".into();
    save.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
    s.restore_active_character_checkpoint(&save).unwrap();
    fill_bag(&mut s, &before);
    let gold = s.world_snapshot().gold;
    s.handle_packet(ClientPacket::MarketBuy {
        auction_id: id,
        bid_price: 77,
    });
    assert_eq!(s.world_snapshot().gold, gold);
    assert!(
        !s.world_snapshot()
            .stage5_systems
            .auction
            .iter()
            .find(|l| u64::from(l.id) == id)
            .unwrap()
            .sold
    );
    let mut save = s.active_character_checkpoint().unwrap();
    save.inventory_items_json.pop();
    s.restore_active_character_checkpoint(&save).unwrap();
    let packets = s.handle_packet(ClientPacket::MarketBuy {
        auction_id: id,
        bid_price: 77,
    });
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::LoseGold { gold: 77 })));
    assert_eq!(s.world_snapshot().gold, gold - 77);
    assert_eq!(
        identity(
            items(&s)
                .into_iter()
                .find(|v| v["unique_id"] == 700_006)
                .unwrap()
        ),
        identity(before)
    );
    s.handle_packet(ClientPacket::MarketBuy {
        auction_id: id,
        bid_price: 77,
    });
    assert_eq!(s.world_snapshot().gold, gold - 77);
}
#[test]
fn demo_with_every_bag_item_in_custody_is_not_reseeded_on_login() {
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let mut fixture_save = s.active_character_checkpoint().unwrap();
    fixture_save
        .inventory_items_json
        .retain(|encoded| serde_json::from_str::<Value>(encoded).unwrap()["key"] == "dagger");
    s.restore_active_character_checkpoint(&fixture_save)
        .unwrap();
    open_refine(&mut s);
    let item = items(&s).into_iter().next().unwrap();
    assert!(s
        .handle_packet(ClientPacket::DepositRefineItem {
            from: item["slot"].as_i64().unwrap() as i32,
            to: 0,
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::DepositRefineItem { success: true, .. })));
    assert!(items(&s).is_empty());
    let mut save = s.active_character_checkpoint().unwrap();
    save.gold = 0;
    s.restore_active_character_checkpoint(&save).unwrap();
    s.save_active_character().unwrap();
    let s = started(&config);
    assert!(
        items(&s).is_empty(),
        "custodied starter assets must not be cloned by demo seed migration"
    );
    assert_eq!(s.world_snapshot().gold, 0);
    assert!(!s
        .world_snapshot()
        .stage5_systems
        .refine
        .item_states
        .is_empty());
}
#[test]
fn market_merge_refreshes_existing_stack_without_double_increment() {
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let before = fixture(&mut s, "red-potion", 700_007);
    let id = consign(&mut s, 700_007);
    let mut save = s.active_character_checkpoint().unwrap();
    let mut destination = before.clone();
    destination["unique_id"] = json!(700_008);
    destination["quantity"] = json!(1);
    save.inventory_items_json.push(destination.to_string());
    s.restore_active_character_checkpoint(&save).unwrap();
    let packets = s.handle_packet(ClientPacket::MarketGetBack {
        mode: 0,
        auction_id: id,
    });
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::RefreshItem { item } if item.unique_id == 700_008 && item.count == 18)));
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedItem { item } if item.unique_id == 700_008)));
    let returned = items(&s)
        .into_iter()
        .filter(|v| v["key"] == "red-potion")
        .collect::<Vec<_>>();
    assert_eq!(returned.len(), 1);
    destination["quantity"] = json!(18);
    assert_eq!(identity(returned[0].clone()), identity(destination));
}

#[test]
fn refine_cancel_returns_only_what_fits_and_keeps_the_remaining_instance() {
    let config = SimulationConfig::default();
    let mut s = started(&config);
    let dagger = fixture(&mut s, "dagger", 700_030);
    let potion = fixture(&mut s, "red-potion", 700_031);
    for (to, item) in [(0, &dagger), (1, &potion)] {
        assert!(s
            .handle_packet(ClientPacket::DepositRefineItem {
                from: item["slot"].as_i64().unwrap() as i32,
                to,
            })
            .iter()
            .any(|p| matches!(p, ServerPacket::DepositRefineItem { success: true, .. })));
    }
    fill_bag(&mut s, &dagger);
    let mut save = s.active_character_checkpoint().unwrap();
    save.inventory_items_json.pop();
    s.restore_active_character_checkpoint(&save).unwrap();
    s.handle_packet(ClientPacket::RefineCancel);
    assert_eq!(
        s.world_snapshot().stage5_systems.refine.item_states.len(),
        1
    );
    assert!(items(&s).iter().any(|item| item["unique_id"] == 700_030));
    assert!(!items(&s).iter().any(|item| item["unique_id"] == 700_031));
    s.save_active_character().unwrap();
    let mut s = started(&config);
    let held = s.world_snapshot().stage5_systems.refine;
    let remaining: Value = serde_json::from_str(&held.item_states[&1]).unwrap();
    assert_eq!(identity(remaining), identity(potion));
    let bag = items(&s);
    s.handle_packet(ClientPacket::RetrieveRefineItem { from: 1, to: 0 });
    assert_eq!(items(&s), bag);
    assert_eq!(s.world_snapshot().stage5_systems.refine, held);
}
#[test]
fn market_return_preserves_whole_stack_and_equipment_after_save_reload() {
    for key in ["dagger", "red-potion"] {
        let config = SimulationConfig::default();
        let mut s = started(&config);
        let before = fixture(&mut s, key, 700_001);
        let id = consign(&mut s, 700_001);
        s.save_active_character().unwrap();
        let mut s = started(&config);
        assert!(s
            .handle_packet(ClientPacket::MarketGetBack {
                mode: 0,
                auction_id: id
            })
            .iter()
            .any(|p| matches!(p, ServerPacket::MarketSuccess { .. })));
        let after = items(&s)
            .into_iter()
            .find(|v| v["key"] == before["key"])
            .unwrap();
        assert_eq!(
            identity(after),
            identity(before),
            "{key} lost instance fields"
        );
        assert!(s
            .handle_packet(ClientPacket::MarketGetBack {
                mode: 0,
                auction_id: id
            })
            .iter()
            .all(|p| !matches!(p, ServerPacket::MarketSuccess { .. })));
    }
}
#[test]
fn refine_retrieve_and_cancel_preserve_equipment_after_save_reload() {
    for cancel in [false, true] {
        let config = SimulationConfig::default();
        let mut s = started(&config);
        let before = fixture(&mut s, "dagger", 700_002);
        let slot = before["slot"].as_i64().unwrap() as i32;
        assert!(s
            .handle_packet(ClientPacket::DepositRefineItem { from: slot, to: 2 })
            .iter()
            .any(|p| matches!(p, ServerPacket::DepositRefineItem { success: true, .. })));
        s.save_active_character().unwrap();
        let mut s = started(&config);
        let packet = if cancel {
            ClientPacket::RefineCancel
        } else {
            ClientPacket::RetrieveRefineItem { from: 2, to: slot }
        };
        s.handle_packet(packet);
        let after = items(&s)
            .into_iter()
            .find(|v| v["key"] == before["key"])
            .unwrap();
        assert_eq!(
            identity(after),
            identity(before),
            "refine custody lost instance"
        );
    }
}

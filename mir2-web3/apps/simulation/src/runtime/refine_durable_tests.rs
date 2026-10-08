//! Isolated, in-memory Carlos fixture. These are ordinary packet/durable
//! boundary tests, not receipts for a natural player route or native frontend.
use crate::{AccountStoreTransactionFault, SimulationConfig, SimulationSession, VisibleNpcRecord};
use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};
use serde_json::{json, Value};

const NPC: u32 = 49_990;
const TARGET: u64 = 900_000;
const MATERIAL_BASE: u64 = 900_100;

fn config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: NPC,
        name: "Isolated refine transaction fixture".into(),
        image: 1,
        colour_argb: -1,
        position: Point {
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
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
    session
}
fn fixture(config: &SimulationConfig) -> SimulationSession {
    let mut session = login(config);
    let mut save = session.active_character_checkpoint().unwrap();
    save.position = config.spawn.clone();
    let source: Value = save
        .inventory_items_json
        .iter()
        .map(|encoded| serde_json::from_str::<Value>(encoded).unwrap())
        .find(|item| item["key"] == "dagger")
        .unwrap();
    save.inventory_items_json = (0u32..17)
        .map(|index| {
            let mut item = source.clone();
            item["unique_id"] = json!(if index == 0 {
                TARGET
            } else {
                MATERIAL_BASE + u64::from(index - 1)
            });
            item["slot"] = json!(index);
            item["container"] = json!("bag1");
            item["quantity"] = json!(1);
            item["durability_current"] = json!(7);
            item["durability_max"] = json!(23);
            item["description"] = json!(format!("test-only refine carrier {index}"));
            item.to_string()
        })
        .collect();
    session.restore_active_character_checkpoint(&save).unwrap();
    // Only fixture setup is explicitly saved; operations below must checkpoint
    // before returning ACK without any manual post-operation save call.
    session.save_active_character().unwrap();
    session
}
fn call(session: &mut SimulationSession, key: &str) -> Vec<ServerPacket> {
    session
        .try_handle_packet(ClientPacket::CallNpc {
            object_id: NPC,
            key: key.into(),
        })
        .unwrap()
}
fn open(session: &mut SimulationSession) {
    assert!(call(session, "@Refine")
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NPCRefine { .. })));
}
fn carried(session: &SimulationSession) -> Vec<Value> {
    let save = session.active_character_checkpoint().unwrap();
    save.inventory_items_json
        .iter()
        .chain(&save.belt_items_json)
        .map(|encoded| serde_json::from_str(encoded).unwrap())
        .collect()
}
fn stored_carried(config: &SimulationConfig) -> Vec<Value> {
    let store = config.account_store.lock().unwrap();
    let save = &store.accounts["demo"].saves[&0];
    save.inventory_items_json
        .iter()
        .chain(&save.belt_items_json)
        .map(|encoded| serde_json::from_str(encoded).unwrap())
        .collect()
}
fn stored_refine(config: &SimulationConfig) -> Value {
    let store = config.account_store.lock().unwrap();
    let systems: Value = serde_json::from_str(
        store.accounts["demo"].saves[&0]
            .stage5_systems_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    systems["refine"].clone()
}
fn deposit(session: &mut SimulationSession, slot: i32) -> Vec<ServerPacket> {
    session
        .try_handle_packet(ClientPacket::DepositRefineItem {
            from: 7 + slot,
            to: slot,
        })
        .unwrap()
}
fn due_fixture(session: &mut SimulationSession) {
    let mut save = session.active_character_checkpoint().unwrap();
    let mut systems: Value =
        serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
    systems["refine"]["remainingMs"] = json!(0);
    systems["refine"]["clockEpoch"] = json!("test-only previous server");
    systems["refine"]["collectDeadlineMs"] = json!(1);
    save.stage5_systems_json = Some(systems.to_string());
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
}

#[test]
fn refine_deposit_persist_failure_returns_no_ack_and_rolls_back_live_and_durable_custody() {
    let config = config();
    let mut session = fixture(&config);
    open(&mut session);
    let before = carried(&session);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .try_handle_packet(ClientPacket::DepositRefineItem { from: 7, to: 0 })
        .is_err());
    assert_eq!(carried(&session), before);
    assert_eq!(stored_carried(&config), before);
    assert!(session
        .world_snapshot()
        .stage5_systems
        .refine
        .item_states
        .is_empty());
    assert!(deposit(&mut session, 0).iter().any(|p| matches!(
        p,
        ServerPacket::DepositRefineItem {
            from: 7,
            to: 0,
            success: true
        }
    )));
    assert!(stored_carried(&config)
        .iter()
        .all(|item| item["unique_id"] != MATERIAL_BASE));
    assert!(stored_refine(&config)["itemStates"]["0"]
        .as_str()
        .unwrap()
        .contains("test-only refine carrier 1"));
}

#[test]
fn all_sixteen_materials_are_durable_before_ack_and_reload_with_original_uid_and_fields() {
    let config = config();
    let mut session = fixture(&config);
    open(&mut session);
    for slot in 0..16 {
        assert!(deposit(&mut session, slot)
            .iter()
            .any(|p| matches!(p, ServerPacket::DepositRefineItem { success: true, .. })));
        assert_eq!(
            stored_refine(&config)["itemStates"]
                .as_object()
                .unwrap()
                .len(),
            slot as usize + 1
        );
    }
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .refine
            .item_states
            .len(),
        16
    );
    let reloaded = login(&config);
    for (slot, encoded) in reloaded.world_snapshot().stage5_systems.refine.item_states {
        let item: Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(item["unique_id"], MATERIAL_BASE + u64::from(slot));
        assert_eq!(item["durability_current"], 7);
        assert_eq!(item["durability_max"], 23);
    }
    assert!(carried(&reloaded).iter().all(|item| !item["unique_id"]
        .as_u64()
        .is_some_and(|id| (MATERIAL_BASE..MATERIAL_BASE + 16).contains(&id))));
}

#[test]
fn oven_start_failure_preserves_gold_weapon_and_material_then_retry_charges_once() {
    let config = config();
    let mut session = fixture(&config);
    open(&mut session);
    deposit(&mut session, 0);
    let before = carried(&session);
    let held = session.world_snapshot().stage5_systems.refine.item_states;
    let gold = session.world_snapshot().gold;
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .try_handle_packet(ClientPacket::RefineItem { unique_id: TARGET })
        .is_err());
    assert_eq!(carried(&session), before);
    assert_eq!(session.world_snapshot().gold, gold);
    assert_eq!(
        session.world_snapshot().stage5_systems.refine.item_states,
        held
    );
    assert!(session
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json
        .is_none());
    let packets = session
        .try_handle_packet(ClientPacket::RefineItem { unique_id: TARGET })
        .unwrap();
    let fee = packets
        .iter()
        .find_map(|packet| {
            if let ServerPacket::LoseGold { gold } = packet {
                Some(*gold)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(session.world_snapshot().gold, gold - fee);
    assert!(stored_carried(&config)
        .iter()
        .all(|item| item["unique_id"] != TARGET));
    assert!(stored_refine(&config)["ovenItemStateJson"]
        .as_str()
        .is_some());
    let again = session
        .try_handle_packet(ClientPacket::RefineItem { unique_id: TARGET })
        .unwrap();
    assert!(!again
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { .. })));
    assert_eq!(session.world_snapshot().gold, gold - fee);
}

#[test]
fn ordinary_collect_checkpoint_failure_keeps_result_in_oven_and_retry_returns_exact_uid_once() {
    let config = config();
    let mut session = fixture(&config);
    open(&mut session);
    deposit(&mut session, 0);
    session
        .try_handle_packet(ClientPacket::RefineItem { unique_id: TARGET })
        .unwrap();
    due_fixture(&mut session);
    let oven = session
        .world_snapshot()
        .stage5_systems
        .refine
        .oven_item_state_json;
    let before = carried(&session);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .try_handle_packet(ClientPacket::CallNpc {
            object_id: NPC,
            key: "@RefineCollect".into()
        })
        .is_err());
    assert_eq!(carried(&session), before);
    assert_eq!(
        session
            .world_snapshot()
            .stage5_systems
            .refine
            .oven_item_state_json,
        oven
    );
    assert!(call(&mut session, "@RefineCollect")
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCCollectRefine { success: true })));
    assert_eq!(
        stored_carried(&config)
            .iter()
            .filter(|item| item["unique_id"] == TARGET)
            .count(),
        1
    );
    assert!(stored_refine(&config)["ovenItemStateJson"].is_null());
    assert!(!call(&mut session, "@RefineCollect")
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCCollectRefine { success: true })));
}

#[test]
fn checking_pending_weapon_failure_rolls_back_instance_and_commits_destruction_only_once() {
    let config = config();
    let mut session = fixture(&config);
    open(&mut session);
    session
        .try_handle_packet(ClientPacket::RefineItem { unique_id: TARGET })
        .unwrap();
    due_fixture(&mut session);
    call(&mut session, "@RefineCollect");
    // No materials has source RefinedValue.None, a guaranteed smash. No random
    // seed, privileged live operation or claimed natural acquisition is used.
    call(&mut session, "@RefineCheck");
    let before = carried(&session);
    let gold = session.world_snapshot().gold;
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .try_handle_packet(ClientPacket::CheckRefine { unique_id: TARGET })
        .is_err());
    assert_eq!(carried(&session), before);
    assert_eq!(stored_carried(&config), before);
    let packets = session
        .try_handle_packet(ClientPacket::CheckRefine { unique_id: TARGET })
        .unwrap();
    assert!(packets
        .iter()
        .any(|p| matches!(p,ServerPacket::RefineItem { unique_id } if *unique_id==TARGET)));
    assert!(stored_carried(&config)
        .iter()
        .all(|item| item["unique_id"] != TARGET));
    assert!(!session
        .try_handle_packet(ClientPacket::CheckRefine { unique_id: TARGET })
        .unwrap()
        .iter()
        .any(|p| matches!(p,ServerPacket::RefineItem { unique_id } if *unique_id==TARGET)));
    assert_eq!(session.world_snapshot().gold, gold);
}

#[test]
fn retrieve_and_cancel_failure_keep_held_uid_and_both_retry_without_duplicate_items() {
    for cancel in [false, true] {
        let config = config();
        let mut session = fixture(&config);
        open(&mut session);
        deposit(&mut session, 0);
        let held = session.world_snapshot().stage5_systems.refine.item_states;
        let before = carried(&session);
        let packet = if cancel {
            ClientPacket::RefineCancel
        } else {
            ClientPacket::RetrieveRefineItem { from: 0, to: 7 }
        };
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        assert!(session.try_handle_packet(packet.clone()).is_err());
        assert_eq!(
            session.world_snapshot().stage5_systems.refine.item_states,
            held
        );
        assert_eq!(carried(&session), before);
        let packets = session.try_handle_packet(packet).unwrap();
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::RefineCancel | ServerPacket::RetrieveRefineItem { success: true, .. }
        )));
        assert_eq!(
            stored_carried(&config)
                .iter()
                .filter(|item| item["unique_id"] == MATERIAL_BASE)
                .count(),
            1
        );
        assert!(stored_refine(&config)["itemStates"]
            .as_object()
            .unwrap()
            .is_empty());
    }
}

#[test]
fn full_bag_cancel_has_negative_ack_and_retains_durable_custody_for_later_space() {
    let config = config();
    let mut session = fixture(&config);
    open(&mut session);
    deposit(&mut session, 0);
    let mut save = session.active_character_checkpoint().unwrap();
    save.inventory_capacity = 46;
    let sample: Value = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
    save.inventory_items_json = (0..40)
        .map(|slot| {
            let mut item = sample.clone();
            item["unique_id"] = json!(800_000 + slot);
            item["slot"] = json!(slot);
            item.to_string()
        })
        .collect();
    save.belt_items_json = (0..6)
        .map(|slot| {
            let mut item = sample.clone();
            item["unique_id"] = json!(810_000 + slot);
            item["container"] = json!("belt");
            item["slot"] = json!(slot);
            item.to_string()
        })
        .collect();
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
    let packets = session
        .try_handle_packet(ClientPacket::RefineCancel)
        .unwrap();
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::NPCCollectRefine { success: false })));
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::RefineCancel)));
    assert_eq!(
        stored_refine(&config)["itemStates"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    let mut save = session.active_character_checkpoint().unwrap();
    save.inventory_items_json.pop();
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
    assert!(session
        .try_handle_packet(ClientPacket::RefineCancel)
        .unwrap()
        .iter()
        .any(|p| matches!(p, ServerPacket::RefineCancel)));
    assert_eq!(
        stored_carried(&config)
            .iter()
            .filter(|item| item["unique_id"] == MATERIAL_BASE)
            .count(),
        1
    );
}

#[test]
fn overweight_refine_cancel_returns_to_empty_source_cell_and_never_manufactures_mail() {
    let config = config();
    let mut session = fixture(&config);
    let mut save = session.active_character_checkpoint().unwrap();
    // The default starter sample carries a legacy display-weight cache; use
    // the actual source Dagger weight for this prepared overweight fixture.
    let source_weight = mir2_game_data::crystal_item_by_name("Dagger").unwrap().weight;
    for encoded in &mut save.inventory_items_json {
        let mut item: Value = serde_json::from_str(encoded).unwrap();
        item["weight"] = json!(source_weight);
        *encoded = item.to_string();
    }
    let sample: Value = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
    for slot in 17..40 {
        let mut item = sample.clone();
        item["slot"] = json!(slot);
        item["unique_id"] = json!(920_000 + slot);
        save.inventory_items_json.push(item.to_string());
    }
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
    let snapshot = session.world_snapshot();
    assert!(snapshot.current_weight > snapshot.max_weight,
        "prepared original weight {} exceeds capacity {}", snapshot.current_weight, snapshot.max_weight);
    open(&mut session);
    deposit(&mut session, 0);
    let held = session.world_snapshot().stage5_systems.refine.item_states;
    let before = carried(&session);
    let mail_before = session.world_snapshot().stage5_systems.mail;

    // HumanObject.CanGainItem first returns true for any free Inventory cell.
    // RefineCancel already found that cell; weight does not select its mail
    // branch in the pinned Crystal source. Failed persistence still retains it.
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session.try_handle_packet(ClientPacket::RefineCancel).is_err());
    assert_eq!(session.world_snapshot().stage5_systems.refine.item_states, held);
    assert_eq!(carried(&session), before);
    assert_eq!(session.world_snapshot().stage5_systems.mail, mail_before);

    let packets = session.try_handle_packet(ClientPacket::RefineCancel).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::RetrieveRefineItem { from: 0, success: true, .. })));
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::RefineCancel)));
    assert!(packets.iter().all(|packet| !matches!(packet, ServerPacket::ReceiveMail { .. })));
    assert_eq!(session.world_snapshot().stage5_systems.mail, mail_before);
    let returned = stored_carried(&config);
    let item = returned.iter().find(|item| item["unique_id"] == MATERIAL_BASE).unwrap();
    let held: Value = serde_json::from_str(&held[&0]).unwrap();
    assert_eq!(item["durability_current"], held["durability_current"]);
    assert_eq!(item["durability_max"], held["durability_max"]);
    assert_eq!(item["description"], held["description"]);
    assert_eq!(returned.iter().filter(|item| item["unique_id"] == MATERIAL_BASE).count(), 1);
    assert!(stored_refine(&config)["itemStates"].as_object().unwrap().is_empty());
    let reloaded = login(&config);
    assert_eq!(reloaded.world_snapshot().stage5_systems.mail, mail_before);
    assert_eq!(carried(&reloaded).iter().filter(|item| item["unique_id"] == MATERIAL_BASE).count(), 1);
}

#[test]
fn original_belt_and_second_bag_cells_preserve_concrete_item_across_durable_transfers() {
    let config = config();
    let mut session = fixture(&config);
    let mut save = session.active_character_checkpoint().unwrap();
    save.inventory_capacity = 86;
    let mut source: Value = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
    let uid = 990_005u64;
    source["unique_id"] = json!(uid);
    source["container"] = json!("belt");
    source["slot"] = json!(5);
    save.belt_items_json
        .retain(|encoded| serde_json::from_str::<Value>(encoded).unwrap()["slot"] != 5);
    save.belt_items_json.push(source.to_string());
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
    open(&mut session);
    assert!(session
        .try_handle_packet(ClientPacket::DepositRefineItem { from: 5, to: 15 })
        .unwrap()
        .iter()
        .any(|p| matches!(
            p,
            ServerPacket::DepositRefineItem {
                from: 5,
                to: 15,
                success: true
            }
        )));
    assert!(stored_carried(&config)
        .iter()
        .all(|item| item["unique_id"] != uid));
    assert!(session
        .try_handle_packet(ClientPacket::RetrieveRefineItem { from: 15, to: 85 })
        .unwrap()
        .iter()
        .any(|p| matches!(
            p,
            ServerPacket::RetrieveRefineItem {
                from: 15,
                to: 85,
                success: true
            }
        )));
    let stored_items = stored_carried(&config);
    let returned = stored_items
        .iter()
        .find(|item| item["unique_id"] == uid)
        .unwrap();
    assert_eq!(returned["container"], "bag2");
    assert_eq!(returned["slot"], 39);
    assert_eq!(returned["durability_current"], 7);
    assert_eq!(returned["durability_max"], 23);
    assert_eq!(
        stored_items
            .iter()
            .filter(|item| item["unique_id"] == uid)
            .count(),
        1
    );
    assert!(session
        .try_handle_packet(ClientPacket::DepositRefineItem { from: 85, to: 15 })
        .unwrap()
        .iter()
        .any(|p| matches!(
            p,
            ServerPacket::DepositRefineItem {
                from: 85,
                to: 15,
                success: true
            }
        )));
    assert!(session
        .try_handle_packet(ClientPacket::RetrieveRefineItem { from: 15, to: 5 })
        .unwrap()
        .iter()
        .any(|p| matches!(
            p,
            ServerPacket::RetrieveRefineItem {
                from: 15,
                to: 5,
                success: true
            }
        )));
    let reloaded = login(&config);
    let carried = carried(&reloaded);
    let returned = carried
        .iter()
        .find(|item| item["unique_id"] == uid)
        .unwrap();
    assert_eq!(returned["container"], "belt");
    assert_eq!(returned["slot"], 5);
    assert_eq!(returned["durability_current"], 7);
    assert_eq!(
        carried
            .iter()
            .filter(|item| item["unique_id"] == uid)
            .count(),
        1
    );
}

#[test]
fn belt_weapon_can_start_return_to_full_bag_empty_belt_and_check_with_durable_rollback() {
    let config = config();
    let mut session = fixture(&config);
    let mut save = session.active_character_checkpoint().unwrap();
    save.inventory_capacity = 46;
    let mut weapon: Value = serde_json::from_str(&save.inventory_items_json.remove(0)).unwrap();
    weapon["container"] = json!("belt");
    weapon["slot"] = json!(5);
    save.belt_items_json
        .retain(|encoded| serde_json::from_str::<Value>(encoded).unwrap()["slot"] != 5);
    save.belt_items_json.push(weapon.to_string());
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
    open(&mut session);
    assert!(session
        .try_handle_packet(ClientPacket::RefineItem { unique_id: TARGET })
        .unwrap()
        .iter()
        .any(
            |packet| matches!(packet,ServerPacket::RefineItem { unique_id } if *unique_id==TARGET)
        ));
    assert!(stored_carried(&config)
        .iter()
        .all(|item| item["unique_id"] != TARGET));
    due_fixture(&mut session);
    let mut save = session.active_character_checkpoint().unwrap();
    let sample: Value = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
    save.inventory_items_json = (0..40)
        .map(|slot| {
            let mut item = sample.clone();
            item["unique_id"] = json!(820_000 + slot);
            item["container"] = json!("bag1");
            item["slot"] = json!(slot);
            item.to_string()
        })
        .collect();
    save.belt_items_json = (0..5)
        .map(|slot| {
            let mut item = sample.clone();
            item["unique_id"] = json!(830_000 + slot);
            item["container"] = json!("belt");
            item["slot"] = json!(slot);
            item.to_string()
        })
        .collect();
    session.restore_active_character_checkpoint(&save).unwrap();
    session.save_active_character().unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .try_handle_packet(ClientPacket::CallNpc {
            object_id: NPC,
            key: "@RefineCollect".into()
        })
        .is_err());
    assert!(stored_refine(&config)["ovenItemStateJson"]
        .as_str()
        .is_some());
    assert!(stored_carried(&config)
        .iter()
        .all(|item| item["unique_id"] != TARGET));
    assert!(call(&mut session, "@RefineCollect")
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NPCCollectRefine { success: true })));
    let stored = stored_carried(&config);
    let returned = stored
        .iter()
        .find(|item| item["unique_id"] == TARGET)
        .unwrap();
    assert_eq!(returned["container"], "belt");
    assert_eq!(returned["slot"], 5);
    assert_eq!(returned["durability_current"], 7);
    assert!(stored_refine(&config)["ovenItemStateJson"].is_null());
    call(&mut session, "@RefineCheck");
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session
        .try_handle_packet(ClientPacket::CheckRefine { unique_id: TARGET })
        .is_err());
    assert_eq!(
        carried(&session)
            .iter()
            .filter(|item| item["unique_id"] == TARGET)
            .count(),
        1
    );
    assert!(session
        .try_handle_packet(ClientPacket::CheckRefine { unique_id: TARGET })
        .unwrap()
        .iter()
        .any(
            |packet| matches!(packet,ServerPacket::RefineItem { unique_id } if *unique_id==TARGET)
        ));
    assert!(stored_carried(&config)
        .iter()
        .all(|item| item["unique_id"] != TARGET));
}

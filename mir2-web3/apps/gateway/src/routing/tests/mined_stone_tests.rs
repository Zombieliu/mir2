//! Real original D401 wall, ordinary account/shop/equipment, trusted Zone
//! preparation and exact personal/Zone commit. Only progression/position and
//! candidate timestamps are controlled; no raw carrier, serial or rate is set.
use super::*;

struct Workshops(Option<std::ffi::OsString>);
impl Workshops {
    fn enter() -> Self {
        let prior = std::env::var_os("MIR2_STONE_WORKSHOPS");
        std::env::set_var("MIR2_STONE_WORKSHOPS", "1");
        Self(prior)
    }
}
impl Drop for Workshops {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => std::env::set_var("MIR2_STONE_WORKSHOPS", value),
            None => std::env::remove_var("MIR2_STONE_WORKSHOPS"),
        }
    }
}

fn raw_swing(f: &mut Fixture) -> ZoneMiningSwing {
    let sid = f.runtime.current_zone_session_id().unwrap();
    let tool = f.runtime.inner.shared_mining_tool().unwrap();
    for _ in 0..8192 {
        let swing = f
            .runtime
            .zone_state
            .lock()
            .unwrap()
            .zone_manager
            .prepare_mining_swing(&sid, MirDirection::Up, &tool, f.next_time)
            .unwrap();
        if swing.raw_stone_template_id() == Some(50001) {
            assert!(swing.ore().is_none());
            assert!(swing.tool_damage() > 0);
            return swing;
        }
        f.next_time = f.next_time.saturating_add(tool.attack_delay_ms.max(550));
    }
    panic!("normal D401 rolls did not produce a rough raw stone");
}

fn registry(f: &Fixture) -> Value {
    let store: Value = serde_json::from_slice(&fs::read(&f.path).unwrap()).unwrap();
    store["sealedStones"].clone()
}

fn raw_carrier(save: &mir2_simulation::CharacterSaveRecord) -> Value {
    let raw: Vec<_> = save
        .inventory_items_json
        .iter()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .filter(|item| item["stone_serial"].as_u64().is_some())
        .collect();
    assert_eq!(raw.len(), 1);
    assert_eq!(raw[0]["key"], "crystal-item-50001");
    assert_eq!(raw[0]["quantity"], 1);
    raw[0].clone()
}

fn choose(runtime: &mut SharedInProcessZoneSessionRuntime, prefix: &str) -> Vec<ServerPacket> {
    let dialog = runtime
        .inner
        .world_snapshot()
        .active_npc_dialog
        .unwrap_or_else(|| panic!("missing NPC dialog while choosing {prefix}"));
    assert!(!dialog
        .links
        .iter()
        .any(|link| link.target.starts_with("@stone:v1:seal:")));
    let targets: Vec<_> = dialog
        .links
        .iter()
        .map(|link| link.target.as_str())
        .collect();
    let target = dialog
        .links
        .iter()
        .find(|link| {
            // Imported merchant scripts may spell @exit in lowercase. Send the
            // exact displayed bytes; economic stone targets remain case-sensitive.
            if prefix == "@Exit" {
                link.target.eq_ignore_ascii_case(prefix)
            } else {
                link.target.starts_with(prefix)
            }
        })
        .unwrap_or_else(|| {
            panic!(
                "missing NPC link {prefix}; title={:?}, npc={}, actual targets={targets:?}",
                dialog.title, dialog.npc_object_id
            )
        })
        .target
        .clone();
    runtime
        .execute(WorldCommand::SelectNpcDialog { target })
        .unwrap()
}

// Faults below target the raw carrier/registry commit, not the independent
// first-use actor bootstrap. Initialize that actor through Bill's ordinary
// displayed workshop link, then return to the original D401 fixture position.
fn purchased_stone_fixture(label: &str) -> Fixture {
    let mut f = purchased_fixture(MirClass::Warrior, label);
    let mining = f.runtime.inner.active_character_checkpoint().unwrap();
    prepare_transform(
        &mut f.runtime,
        "0",
        "BichonProvince",
        Point { x: 302, y: 222 },
        12,
        None,
    );
    let bill =
        mir2_game_data::crystal_npc_info_by_script_key("BichonProvince/BichonWall/Blacksmith")
            .unwrap();
    f.runtime
        .execute(WorldCommand::Interact {
            object_id: bill.loaded_object_id.unwrap(),
        })
        .unwrap();
    choose(&mut f.runtime, "@stone:v1:home");
    let initialized = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(initialized.npc_purchase_journal.is_some());
    let store: mir2_simulation::AccountStore =
        serde_json::from_slice(&fs::read(&f.path).unwrap()).unwrap();
    let identity = f.runtime.inner.active_identity().unwrap();
    assert_eq!(
        store.accounts[&identity.account_id].saves[&identity.character_index].npc_purchase_journal,
        initialized.npc_purchase_journal
    );
    choose(&mut f.runtime, "@Exit");
    prepare_transform(
        &mut f.runtime,
        "D401",
        "DeadMineEntrance",
        mining.position,
        12,
        None,
    );
    f.next_time = SharedInProcessZoneSessionRuntime::zone_now_ms().saturating_add(10_000);
    f
}

#[test]
fn mined_stone_original_d401_award_survives_cold_login_and_ordinary_appraise_cut() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let _workshops = Workshops::enter();
    let mut f = purchased_fixture(MirClass::Warrior, "MinedStone");
    refill(&mut f);
    let swing = raw_swing(&mut f);
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(
        before.npc_purchase_journal.is_none(),
        "first mining initializes its own actor"
    );
    let stones = spot(&f)["stones_left"].as_u64().unwrap();
    let dura = f.runtime.inner.shared_mining_tool().unwrap().durability;
    let packets = commit(&mut f, &swing);
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p,
        ServerPacket::GainedItem { item } if item.item_index == 50001))
            .count(),
        1
    );
    assert_eq!(spot(&f)["stones_left"], stones - 1);
    let mined = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(mined.npc_purchase_journal.is_some());
    assert_eq!(mined.gold, before.gold);
    let raw = raw_carrier(&mined);
    let uid = raw["unique_id"].as_u64().unwrap();
    assert_ne!(uid, f.tool_uid);
    assert_eq!(
        f.runtime.inner.shared_mining_tool().unwrap().durability,
        dura - swing.tool_damage()
    );
    let records = registry(&f);
    let record = &records["stones"].as_array().unwrap()[0];
    assert_eq!(records["stones"].as_array().unwrap().len(), 1);
    assert_eq!(record["mint"]["source"]["kind"], "mining");
    assert_eq!(record["mint"]["sourceTemplateId"], 836);
    assert_eq!(record["mint"]["carrierUid"], uid.to_string());
    assert_eq!(record["lifecycle"], "sealed");
    let identity = f.runtime.inner.active_identity().unwrap();
    packet(&mut f.runtime, ClientPacket::LogOut);
    // New config reloads File Source; no in-memory account cache is reused.
    let config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile()
        .with_account_store_path(f.path.clone())
        .with_save_recovery_dir(f.path.with_extension("recovery"))
        .with_save_recovery_mac_key(std::array::from_fn::<_, 32, _>(|i| {
            (i as u8).wrapping_mul(7)
        }))
        .unwrap();
    let mut cold = shared_session_runtime(Arc::new(Mutex::new(SharedInProcessZoneState::new())));
    cold.inner = InProcessWorldRuntime::new(config);
    packet(
        &mut cold,
        ClientPacket::Login {
            account_id: identity.account_id.clone(),
            password: identity.account_id.clone(),
        },
    );
    assert!(packet(
        &mut cold,
        ClientPacket::StartGame {
            character_index: identity.character_index
        }
    )
    .iter()
    .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(
        raw_carrier(&cold.inner.active_character_checkpoint().unwrap()),
        raw
    );
    assert_eq!(registry(&f), records);
    prepare_transform(
        &mut cold,
        "0",
        "BichonProvince",
        Point { x: 302, y: 222 },
        12,
        None,
    );
    let bill =
        mir2_game_data::crystal_npc_info_by_script_key("BichonProvince/BichonWall/Blacksmith")
            .unwrap();
    cold.execute(WorldCommand::Interact {
        object_id: bill.loaded_object_id.unwrap(),
    })
    .unwrap();
    for prefix in [
        "@stone:v1:home",
        "@stone:v1:list:stone:",
        "@stone:v1:stone:",
    ] {
        choose(&mut cold, prefix);
    }
    let gold = cold.inner.world_snapshot().gold;
    for (prefix, fee) in [("@stone:v1:appraise:", 100), ("@stone:v1:cut:", 1500)] {
        let packets = choose(&mut cold, prefix);
        assert_eq!(
            packets
                .iter()
                .filter(|p| matches!(p,
            ServerPacket::LoseGold { gold } if *gold == fee))
                .count(),
            1
        );
    }
    assert_eq!(cold.inner.world_snapshot().gold, gold - 1600);
    let final_save = cold.inner.active_character_checkpoint().unwrap();
    let cut = final_save
        .inventory_items_json
        .iter()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .find(|item| item["unique_id"].as_u64() == Some(uid))
        .unwrap();
    assert_eq!(cut["slot"], raw["slot"]);
    assert!(cut["stone_serial"].is_null());
    assert_ne!(cut["key"], raw["key"]);
    let after_cut = registry(&f);
    assert_eq!(after_cut["stones"].as_array().unwrap().len(), 1);
    assert_eq!(after_cut["stones"][0]["lifecycle"], "cut");
    packet(&mut cold, ClientPacket::LogOut);
}

#[test]
fn mined_stone_original_d401_failed_save_rolls_back_registry_inventory_tool_and_shared_wall() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let _workshops = Workshops::enter();
    let mut f = purchased_stone_fixture("MinedFail");
    refill(&mut f);
    let swing = raw_swing(&mut f);
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(before.npc_purchase_journal.is_some());
    let bytes = fs::read(&f.path).unwrap();
    let shared = zone_state(&f);
    f.config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforeFileRename,
    );
    assert!(f.runtime.inner.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    assert_eq!(
        serde_json::to_value(f.runtime.inner.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(zone_state(&f), shared);
    let packets = commit(&mut f, &swing);
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainedItem { .. }))
            .count(),
        1
    );
    raw_carrier(&f.runtime.inner.active_character_checkpoint().unwrap());
    assert_eq!(registry(&f)["stones"].as_array().unwrap().len(), 1);
    let committed = fs::read(&f.path).unwrap();
    assert!(f.runtime.inner.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), committed);
}

#[test]
fn mined_stone_original_d401_unknown_publication_retains_one_raw_and_fences_retry() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let _workshops = Workshops::enter();
    let mut f = purchased_stone_fixture("MinedUnknown");
    refill(&mut f);
    let swing = raw_swing(&mut f);
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(before.npc_purchase_journal.is_some());
    let shared = zone_state(&f);
    f.config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(f.runtime.inner.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(
        serde_json::to_value(f.runtime.inner.active_character_checkpoint().unwrap()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(zone_state(&f), shared);
    let disk = fs::read(&f.path).unwrap();
    let store: mir2_simulation::AccountStore = serde_json::from_slice(&disk).unwrap();
    let identity = f.runtime.inner.active_identity().unwrap();
    let saved = &store.accounts[&identity.account_id].saves[&identity.character_index];
    raw_carrier(saved);
    assert_eq!(saved.revision, before.revision + 1);
    assert_ne!(saved.equipment_items_json, before.equipment_items_json);
    assert_eq!(registry(&f)["stones"].as_array().unwrap().len(), 1);
    assert!(f.config.save_account_store().is_err());
    assert!(f.runtime.inner.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    assert_eq!(zone_state(&f), shared);
}

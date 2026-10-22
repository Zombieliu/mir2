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

fn dual_drop_swing(f: &mut Fixture) -> ZoneMiningSwing {
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
        if swing.raw_stone_template_id() == Some(50001) && swing.ore().is_some() {
            assert!(swing.tool_damage() > 0);
            return swing;
        }
        f.next_time = f.next_time.saturating_add(tool.attack_delay_ms.max(550));
    }
    panic!("normal D401 rolls did not produce one swing with ore and a rough raw stone");
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

// Read exact ordinary ore from the saved physical inventory, including its
// original purity and UID. The independent raw roll must not consume this ore.
fn ordinary_ore(save: &mir2_simulation::CharacterSaveRecord, swing: &ZoneMiningSwing) -> Value {
    let expected = swing
        .ore()
        .expect("dual-drop candidate must include normal ore");
    let ores: Vec<_> = save
        .inventory_items_json
        .iter()
        .chain(&save.belt_items_json)
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .filter(|item| item["name"] == expected.item_name)
        .collect();
    assert_eq!(
        ores.len(),
        1,
        "same swing persists precisely one ordinary ore"
    );
    assert!(ores[0]["stone_serial"].is_null());
    assert_eq!(ores[0]["quantity"], 1);
    assert_eq!(ores[0]["durability_current"], expected.current_dura);
    ores[0].clone()
}

fn assert_dual_packets(
    packets: &[ServerPacket],
    swing: &ZoneMiningSwing,
    tool_uid: u64,
    remaining_dura: u16,
) {
    let ore_index = mir2_game_data::crystal_item_by_name(&swing.ore().unwrap().item_name)
        .unwrap()
    .item_index;
    let gained: Vec<_> = packets
        .iter()
        .filter_map(|p| match p {
            ServerPacket::GainedItem { item } => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(
        gained.len(),
        2,
        "one normal ore and one independently rolled raw"
    );
    assert_eq!(
        gained
            .iter()
            .filter(|item| item.item_index == ore_index)
            .count(),
        1
    );
    assert_eq!(
        gained
            .iter()
            .filter(|item| item.item_index == 50001)
            .count(),
        1
    );
    assert_ne!(gained[0].unique_id, gained[1].unique_id);
    assert!(gained.iter().all(|item| item.unique_id != tool_uid));
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p,
        ServerPacket::DuraChanged { unique_id, .. } if *unique_id == tool_uid))
            .count(),
        1,
        "the dual drop must publish only one tool-wear update"
    );
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p,
        ServerPacket::DuraChanged { unique_id, current_dura }
            if *unique_id == tool_uid && *current_dura == remaining_dura))
            .count(),
        1
    );
}

fn cold_login(
    f: &Fixture,
    account: &str,
    character_index: i32,
) -> SharedInProcessZoneSessionRuntime {
    // A new config reloads File Source, never the previous account cache.
    let config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_platinum_176_profile()
        .with_account_store_path(f.path.clone())
        .with_save_recovery_dir(f.path.with_extension("recovery"))
        .with_save_recovery_mac_key(std::array::from_fn::<_, 32, _>(|i| {
            (i as u8).wrapping_mul(7)
        }))
        .unwrap();
    let mut runtime = shared_session_runtime(Arc::new(Mutex::new(SharedInProcessZoneState::new())));
    runtime.inner = InProcessWorldRuntime::new(config);
    assert!(packet(
        &mut runtime,
        ClientPacket::Login {
            account_id: account.into(),
            password: account.into()
        }
    )
    .iter()
    .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    assert!(
        packet(&mut runtime, ClientPacket::StartGame { character_index })
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. }))
    );
    runtime
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
fn mined_stone_original_d401_dual_drop_survives_cold_login_and_ordinary_appraise_cut() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let _workshops = Workshops::enter();
    let mut f = purchased_fixture(MirClass::Warrior, "MinedStone");
    refill(&mut f);
    let swing = dual_drop_swing(&mut f);
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(
        before.npc_purchase_journal.is_none(),
        "first mining initializes its own actor"
    );
    let stones = spot(&f)["stones_left"].as_u64().unwrap();
    let dura = f.runtime.inner.shared_mining_tool().unwrap().durability;
    let packets = commit(&mut f, &swing);
    assert_dual_packets(&packets, &swing, f.tool_uid, dura - swing.tool_damage());
    assert_eq!(spot(&f)["stones_left"], stones - 1);
    let mined = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(mined.npc_purchase_journal.is_some());
    assert_eq!(mined.gold, before.gold);
    let raw = raw_carrier(&mined);
    let ore = ordinary_ore(&mined, &swing);
    assert_ne!(raw["unique_id"], ore["unique_id"]);
    assert_ne!(
        (raw["container"].clone(), raw["slot"].clone()),
        (ore["container"].clone(), ore["slot"].clone())
    );
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
    let mut cold = cold_login(&f, &identity.account_id, identity.character_index);
    assert_eq!(
        ordinary_ore(&cold.inner.active_character_checkpoint().unwrap(), &swing),
        ore
    );
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
    assert_eq!(
        ordinary_ore(&final_save, &swing),
        ore,
        "workshop must leave crafting ore intact"
    );
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
    drop(cold);
    let mut reopened = cold_login(&f, &identity.account_id, identity.character_index);
    let settled = reopened.inner.active_character_checkpoint().unwrap();
    assert_eq!(settled.gold, final_save.gold);
    assert_eq!(
        settled.inventory_items_json,
        final_save.inventory_items_json
    );
    assert_eq!(settled.belt_items_json, final_save.belt_items_json);
    assert_eq!(ordinary_ore(&settled, &swing), ore);
    assert_eq!(registry(&f), after_cut);
    packet(&mut reopened, ClientPacket::LogOut);
}

#[test]
fn mined_stone_original_d401_dual_drop_failed_save_rolls_back_registry_inventory_tool_and_shared_wall(
) {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let _workshops = Workshops::enter();
    let mut f = purchased_stone_fixture("MinedFail");
    refill(&mut f);
    let swing = dual_drop_swing(&mut f);
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(before.npc_purchase_journal.is_some());
    let dura = f.runtime.inner.shared_mining_tool().unwrap().durability;
    let stones = spot(&f)["stones_left"].as_u64().unwrap();
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
    assert_dual_packets(&packets, &swing, f.tool_uid, dura - swing.tool_damage());
    assert_eq!(spot(&f)["stones_left"], stones - 1);
    let saved = f.runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(saved.gold, before.gold);
    let raw = raw_carrier(&saved);
    let ore = ordinary_ore(&saved, &swing);
    assert_ne!(raw["unique_id"], ore["unique_id"]);
    assert_eq!(registry(&f)["stones"].as_array().unwrap().len(), 1);
    let committed = fs::read(&f.path).unwrap();
    assert!(f.runtime.inner.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), committed);
}

#[test]
fn mined_stone_original_d401_dual_drop_unknown_publication_retains_both_items_and_fences_retry() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let _workshops = Workshops::enter();
    let mut f = purchased_stone_fixture("MinedUnknown");
    refill(&mut f);
    let swing = dual_drop_swing(&mut f);
    let before = f.runtime.inner.active_character_checkpoint().unwrap();
    assert!(before.npc_purchase_journal.is_some());
    let dura = f.runtime.inner.shared_mining_tool().unwrap().durability;
    let stones = spot(&f)["stones_left"].as_u64().unwrap();
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
    let raw = raw_carrier(saved);
    let ore = ordinary_ore(saved, &swing);
    assert_ne!(raw["unique_id"], ore["unique_id"]);
    assert_eq!(saved.revision, before.revision + 1);
    assert_eq!(saved.gold, before.gold);
    let weapon = saved
        .equipment_items_json
        .iter()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .find(|item| item["user_item_unique_id"].as_u64() == Some(f.tool_uid))
        .unwrap();
    assert_eq!(weapon["durability_current"], dura - swing.tool_damage());
    assert_eq!(
        spot(&f)["stones_left"],
        stones,
        "uncertain personal commit never advances shared wall"
    );
    assert_eq!(registry(&f)["stones"].as_array().unwrap().len(), 1);
    assert!(f.config.save_account_store().is_err());
    assert!(f.runtime.inner.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    assert_eq!(zone_state(&f), shared);
}

// Function-level Gateway regression with a controlled server clock. Tickets
// still come from the original D401 Zone; the ordinary Attack packet ingress
// and cooldown rejection are covered by the parent mining tests.
#[test]
fn mined_stone_raw_only_gateway_breaks_pick_and_syncs_shared_stats_but_full_bag_does_not() {
    let _lock = ENVIRONMENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _environment = MiningEnvironment::enter();
    let _workshops = Workshops::enter();
    for full in [false, true] {
        let mut f = purchased_fixture(
            MirClass::Warrior,
            if full { "RawFullPick" } else { "RawBreakPick" },
        );
        let mut save = f.runtime.inner.active_character_checkpoint().unwrap();
        let index = save
            .equipment_items_json
            .iter()
            .position(|row| serde_json::from_str::<Value>(row).unwrap()["slot"] == "weapon")
            .unwrap();
        let mut pick: Value = serde_json::from_str(&save.equipment_items_json[index]).unwrap();
        pick["durability_current"] = json!(1);
        // Trusted gear precondition: negative Accuracy makes ordinary hits
        // impossible. AC/HP expose the missing post-commit shared refresh.
        pick["added_stats"] = json!([
            {"stat":10,"value":-10},{"stat":1,"value":100},{"stat":12,"value":100}
        ]);
        save.equipment_items_json[index] = pick.to_string();
        f.runtime
            .inner
            .restore_active_character_checkpoint(&save)
            .unwrap();
        if full {
            fill_inventory(&mut f, 40, 6, 46, 1);
        }
        let sid = f.runtime.current_zone_session_id().unwrap();
        f.runtime
            .sync_authoritative_zone_combat_state(&sid)
            .unwrap();
        let before_stats = f.runtime.inner.zone_player_combat_stats();
        let before_max = f.runtime.inner.world_snapshot().player_max_hp.unwrap();
        let mp = f.runtime.inner.world_snapshot().player_mp.unwrap();
        f.runtime
            .inner
            .force_authoritative_player_vitals_with_max_hp(
                Some(before_max),
                Some(before_max),
                Some(mp),
            );
        f.runtime
            .zone_state
            .lock()
            .unwrap()
            .zone_manager
            .commit_owner_vitals(&sid, before_max, before_max, mp, false);
        assert!(before_stats.max_ac >= 100);
        let tool = f.runtime.inner.shared_mining_tool().unwrap();
        assert!(tool.accuracy <= -10);
        refill(&mut f);
        let mut candidate = None;
        for _ in 0..8192 {
            let swing = f
                .runtime
                .zone_state
                .lock()
                .unwrap()
                .zone_manager
                .prepare_mining_swing(&sid, MirDirection::Up, &tool, f.next_time)
                .unwrap();
            assert_eq!(swing.tool_damage(), 0);
            assert!(swing.ore().is_none());
            if swing.raw_stone_template_id().is_some() {
                candidate = Some(swing);
                break;
            }
            f.next_time = f.next_time.saturating_add(tool.attack_delay_ms.max(550));
        }
        let swing = candidate.expect("real independent raw roll must produce a bounded candidate");
        assert!(swing.raw_tool_damage() >= tool.durability);
        let before = f.runtime.inner.active_character_checkpoint().unwrap();
        let before_stones = spot(&f)["stones_left"].as_u64().unwrap();
        let candidate_now = f.next_time;
        let out = f
            .runtime
            .execute_zone_native_mining_with_clock(MirDirection::Up, || candidate_now);
        assert_eq!(spot(&f)["stones_left"], before_stones - 1);
        let snapshot = f.runtime.inner.world_snapshot();
        let after_stats = f.runtime.inner.zone_player_combat_stats();
        if full {
            assert!(!out.iter().any(|p| matches!(
                p,
                ServerPacket::GainedItem { .. } | ServerPacket::DuraChanged { .. }
            )));
            assert_eq!(f.runtime.inner.shared_mining_tool().unwrap().durability, 1);
            assert_eq!(after_stats, before_stats);
            assert_eq!(snapshot.player_max_hp, Some(before_max));
            assert_eq!(snapshot.player_hp, Some(before_max));
            let after = f.runtime.inner.active_character_checkpoint().unwrap();
            assert_eq!(after.inventory_items_json, before.inventory_items_json);
            assert_eq!(after.equipment_items_json, before.equipment_items_json);
            assert!(registry(&f)["stones"].as_array().unwrap().is_empty());
        } else {
            assert!(f.runtime.inner.shared_mining_tool().is_none());
            assert_eq!(
                out.iter()
                    .filter(|p| matches!(p, ServerPacket::GainedItem { .. }))
                    .count(),
                1
            );
            assert!(out.iter().any(|p| matches!(p, ServerPacket::DuraChanged { unique_id, current_dura:0 } if *unique_id == f.tool_uid)));
            assert!(after_stats.max_ac < before_stats.max_ac);
            assert!(snapshot.player_max_hp.unwrap() < before_max);
            assert_eq!(snapshot.player_hp, snapshot.player_max_hp);
        }
        let state = zone_state(&f);
        assert_eq!(
            state["players"][sid.as_str()]["combat_stats"],
            serde_json::to_value(after_stats).unwrap()
        );
        assert_eq!(
            f.runtime
                .zone_state
                .lock()
                .unwrap()
                .zone_manager
                .player_vitals(&sid)
                .unwrap(),
            (
                snapshot.player_hp.unwrap(),
                snapshot.player_max_hp.unwrap(),
                snapshot.player_mp.unwrap()
            )
        );
    }
}

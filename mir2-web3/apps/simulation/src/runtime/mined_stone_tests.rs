//! Controlled File fixtures with real shared-Zone preparation and personal
//! commit. Synthetic collision/progression is explicit, not natural GUI proof.
use super::*;
use crate::runtime::equipment::equipment_state_from_item_state;
use crate::runtime::item_uid_issuance::ItemUidIssuance;
use crate::runtime::zone::{
    ZoneBounds, ZoneCollision, ZoneCommand, ZoneKey, ZoneMiningSwing, ZoneRuntime,
};

pub(super) fn mined_fixture(label: &str) -> Fixture {
    let mut f = Fixture::new(label, 824);
    let template = mir2_game_data::crystal_item_by_name("PickAxe").unwrap();
    let mut pick = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    pick.unique_id = 120_000_002;
    {
        let mut inventory = f
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        inventory.inventory_items.clear();
        inventory.equipment_items = vec![equipment_state_from_item_state(
            &pick,
            crate::EquipmentSlot::Weapon,
        )];
    }
    let mut save = f.live();
    save.map_file_name = "D401".into();
    save.position = Point { x: 38, y: 178 };
    save.direction = MirDirection::Up;
    f.session
        .restore_active_character_checkpoint(&save)
        .unwrap();
    f.session.save_active_character().unwrap();
    assert!(f.session.shared_mining_tool().is_some());
    f
}

pub(super) fn raw_swing(f: &Fixture, index: i32) -> (ZoneRuntime, ZoneMiningSwing) {
    raw_swing_matching(f, index, |s| s.ore().is_none())
}

fn raw_swing_matching(
    f: &Fixture,
    index: i32,
    accept: impl Fn(&ZoneMiningSwing) -> bool,
) -> (ZoneRuntime, ZoneMiningSwing) {
    let position = f.live().position;
    let wall = Point {
        x: position.x,
        y: position.y - 1,
    };
    let collision = ZoneCollision::unbounded()
        .with_bounds(ZoneBounds::new(0, 99, 0, 199))
        .with_blocked_cells([wall]);
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("D401"), collision);
    let join = f
        .session
        .active_zone_join_snapshot("mined-stone-fixture")
        .unwrap();
    assert_eq!(join.map_file_name, "D401");
    assert_eq!(join.position, position);
    let sid = join.session_id.clone();
    zone.handle(ZoneCommand::Join(join));
    assert_eq!(
        zone.player_count(),
        1,
        "fixture must actually join its shared Zone"
    );
    assert_eq!(zone.player_position(&sid), Some(position.clone()));
    assert!(zone.player_identity(&sid).is_some());
    // Joining intentionally leaves combat admission unset. Mirror the real
    // Gateway's trusted personal-session synchronization before preparation.
    let snapshot = f.session.world_snapshot();
    let player = snapshot
        .entities
        .iter()
        .find(|e| e.kind == crate::WorldEntityKind::SelfPlayer)
        .unwrap();
    let attack_blocked = snapshot.active_buffs.iter().any(|buff| {
        matches!(
            buff.key.as_str(),
            "crystal-paralysis" | "crystal-dazed" | "crystal-frozen" | "crystal-blindness"
        )
    });
    zone.handle(ZoneCommand::sync_player_combat_state(
        sid.clone(),
        player.class.unwrap(),
        player.has_class_weapon.unwrap(),
        player.riding_mount.unwrap(),
        player.can_mount_attack.unwrap(),
        player.dead || snapshot.player_hp.unwrap() <= 0,
        attack_blocked,
        player.fishing.unwrap(),
    ));
    zone.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: sid.clone(),
        stats: f.session.zone_player_combat_stats(),
    });
    let tool = f.session.shared_mining_tool().unwrap();
    assert!(
        zone.player_harvest_admitted(&sid, 2_000),
        "fixture admission failed: position={:?}, vitals={:?}, player={player:?}, blocked={attack_blocked}",
        zone.player_position(&sid),
        zone.player_vitals(&sid)
    );
    for attempt in 1..100_000u64 {
        let now = attempt * 2_000;
        let swing = zone.prepare_mining_swing(&sid, MirDirection::Up, &tool, now).unwrap_or_else(||
            panic!("fixture swing rejected: map=D401 now={now} position={:?} vitals={:?} admitted={} tool={tool:?}",
                zone.player_position(&sid), zone.player_vitals(&sid), zone.player_harvest_admitted(&sid, now)));
        if swing.raw_stone_template_id() == Some(index) && accept(&swing) {
            return (zone, swing);
        }
        if swing.tool_damage() == 0
            && swing.ore().is_none()
            && swing.raw_stone_template_id().is_none()
        {
            zone.commit_mining_swing(&swing).unwrap();
        }
    }
    panic!("normal mining rolls did not yield the requested controlled fixture");
}

pub(super) fn return_to_workshop(f: &mut Fixture) {
    let mut save = f.live();
    save.map_file_name = "0".into();
    save.position = Point { x: 302, y: 222 };
    save.direction = MirDirection::Up;
    f.session
        .restore_active_character_checkpoint(&save)
        .unwrap();
    f.session.save_active_character().unwrap();
    f.session.interact(f.npc);
    assert!(workshop_allowed(f.session.app.world()));
}

#[test]
fn mined_stone_three_grades_atomically_save_carrier_frozen_result_and_tool_wear() {
    for index in [RAW_STONE_ROUGH, RAW_STONE_FINE, RAW_STONE_PRECIOUS] {
        let mut f = mined_fixture("grades");
        let (mut zone, swing) = raw_swing(&f, index);
        let before = f.live();
        let floor = mined_uid_floor(&f.disk(), &before).unwrap();
        let tool = f.session.shared_mining_tool().unwrap();
        let packets = f.session.apply_shared_mining_swing(&swing).unwrap();
        zone.commit_mining_swing(&swing).unwrap();
        assert_eq!(f.live().revision, before.revision + 1);
        assert_eq!(f.live().gold, before.gold);
        let inventory = f.session.app.world().resource::<InventoryResource>();
        let item = &inventory.inventory_items[0];
        assert!(item.unique_id > floor);
        assert_eq!(
            try_user_item_from_item_state(item).unwrap().item_index,
            index
        );
        let record = f
            .disk()
            .sealed_stones
            .record(item.stone_serial.unwrap())
            .unwrap()
            .clone();
        assert_eq!(record.carrier_uid(), item.unique_id);
        assert_eq!(record.mint_request().source.kind, StoneSourceKind::Mining);
        assert_eq!(
            record.mint_request().source.event_id,
            swing.raw_stone_event_id()
        );
        assert_eq!(
            record.source_template_id(),
            mir2_game_data::crystal_item_by_name("PickAxe")
                .unwrap()
                .item_index
        );
        assert_eq!(
            inventory.equipment_items[0].durability_current,
            tool.durability - swing.raw_tool_damage()
        );
        assert_eq!(
            packets
                .iter()
                .filter(|p| matches!(p, ServerPacket::GainedItem { .. }))
                .count(),
            1
        );
        f.assert_checkpoint();
        let committed = fs::read(&f.path).unwrap();
        assert!(f.session.apply_shared_mining_swing(&swing).is_err());
        assert_eq!(fs::read(&f.path).unwrap(), committed);
    }
}

#[test]
fn mined_stone_platinum_profile_requires_workshop_opt_in_for_extension_award() {
    for enabled in [false, true] {
        let mut f = mined_fixture("platinum-opt-in");
        f.config = f.config.clone().with_platinum_176_profile();
        f.session
            .app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config = f.config.clone();
        if !enabled {
            f.session
                .app
                .world_mut()
                .remove_resource::<WorkshopEnabled>();
        }
        assert_eq!(
            workshop_enabled(f.session.app.world()),
            enabled,
            "disabled fixture requires no process-wide MIR2_STONE_WORKSHOPS opt-in"
        );
        let raw = crystal_item_by_index(RAW_STONE_ROUGH).unwrap();
        assert!(
            !f.config.item_is_allowed(&raw.name),
            "imported whitelist must remain unchanged"
        );
        let (_, swing) = raw_swing_matching(&f, RAW_STONE_ROUGH, |s| s.tool_damage() == 0);
        let before = f.live();
        let packets = f.session.apply_shared_mining_swing(&swing).unwrap();
        assert_eq!(
            packets
                .iter()
                .filter(|p| matches!(p,
            ServerPacket::GainedItem { item } if item.item_index == RAW_STONE_ROUGH))
                .count(),
            usize::from(enabled)
        );
        assert_eq!(
            f.disk().sealed_stones.records().count(),
            usize::from(enabled)
        );
        assert_eq!(f.live().gold, before.gold);
        assert_eq!(
            f.live().equipment_items_json != before.equipment_items_json,
            enabled
        );
        f.assert_checkpoint();
    }
}

#[test]
fn mined_stone_independent_award_survives_guaranteed_ordinary_miss_and_wears_once() {
    let mut f = mined_fixture("independent");
    f.session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .equipment_items[0]
        .added_stats
        .push(mir2_protocol::UserItemStat {
            stat: crate::runtime::crystal_compat::CRYSTAL_STAT_ACCURACY,
            value: -10,
        });
    f.session.save_active_character().unwrap();
    assert!(f.session.shared_mining_tool().unwrap().accuracy <= -10);
    let (mut zone, swing) = raw_swing(&f, RAW_STONE_ROUGH);
    assert_eq!(swing.tool_damage(), 0);
    assert!(swing.ore().is_none());
    assert!(swing.raw_tool_damage() > 0);
    let before = f.live();
    let mut invalid = swing.clone();
    invalid.raw_durability_damage = 0;
    assert!(f.session.apply_shared_mining_swing(&invalid).is_err());
    assert_eq!(f.live().equipment_items_json, before.equipment_items_json);
    let packets = f.session.apply_shared_mining_swing(&swing).unwrap();
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainedItem { .. }))
            .count(),
        1
    );
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::DuraChanged { .. }))
            .count(),
        1
    );
    assert_eq!(f.live().revision, before.revision + 1);
    assert_eq!(
        f.session.shared_mining_tool().unwrap().durability,
        swing.tool.durability - swing.raw_tool_damage()
    );
    zone.commit_mining_swing(&swing).unwrap();
    let bytes = fs::read(&f.path).unwrap();
    assert!(f.session.apply_shared_mining_swing(&swing).is_err());
    assert!(zone.commit_mining_swing(&swing).is_none());
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
}

#[test]
fn mined_stone_dual_payout_keeps_ordinary_metadata_distinct_uids_and_one_commit() {
    let mut f = mined_fixture("dual");
    let (mut zone, swing) = raw_swing_matching(&f, RAW_STONE_ROUGH, |s| s.ore().is_some());
    let (_, staged) =
        crate::runtime::mining_player::stage_shared_mining_ore(f.session.app.world(), &swing)
            .unwrap();
    let mut expected_ore = staged.unwrap();
    let before = f.live();
    let packets = f.session.apply_shared_mining_swing(&swing).unwrap();
    let inv = f.session.app.world().resource::<InventoryResource>();
    assert_eq!(inv.inventory_items.len(), 2);
    let ore = inv
        .inventory_items
        .iter()
        .find(|i| i.stone_serial.is_none())
        .unwrap();
    let raw = inv
        .inventory_items
        .iter()
        .find(|i| i.stone_serial.is_some())
        .unwrap();
    expected_ore.unique_id = ore.unique_id;
    assert_eq!(
        serde_json::to_value(ore).unwrap(),
        serde_json::to_value(expected_ore).unwrap()
    );
    assert_eq!(
        (ore.container, ore.slot, raw.container, raw.slot),
        (ItemContainer::Bag1, 0, ItemContainer::Bag1, 1)
    );
    assert!(raw.unique_id > ore.unique_id);
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainedItem { .. }))
            .count(),
        2
    );
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::DuraChanged { .. }))
            .count(),
        1
    );
    assert_eq!(f.live().revision, before.revision + 1);
    assert_eq!(f.live().gold, before.gold);
    zone.commit_mining_swing(&swing).unwrap();
    assert!(zone.commit_mining_swing(&swing).is_none());
    f.assert_checkpoint();
}

#[test]
fn mined_stone_ordinary_ore_takes_last_bag_slot_but_raw_can_use_second_bag() {
    for capacity in [46, 86] {
        let mut f = mined_fixture("bag-priority");
        let template = crystal_item_by_index(824).unwrap();
        {
            let mut inv = f
                .session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>();
            inv.inventory_capacity = capacity;
            for slot in 0..39 {
                let mut item =
                    embedded_item_state_from_template(&template, ItemContainer::Bag1, slot);
                item.unique_id = 140_000_000 + u64::from(slot);
                inv.inventory_items.push(item);
            }
        }
        f.session.save_active_character().unwrap();
        let (_, swing) = raw_swing_matching(&f, RAW_STONE_ROUGH, |s| s.ore().is_some());
        let packets = f.session.apply_shared_mining_swing(&swing).unwrap();
        let inv = f.session.app.world().resource::<InventoryResource>();
        assert!(inv.belt_items.is_empty());
        let last = inv
            .inventory_items
            .iter()
            .find(|i| i.container == ItemContainer::Bag1 && i.slot == 39)
            .unwrap();
        assert!(last.stone_serial.is_none());
        assert_eq!(
            last.durability_current,
            Some(swing.ore().unwrap().current_dura)
        );
        assert_eq!(
            packets
                .iter()
                .filter(|p| matches!(p, ServerPacket::GainedItem { .. }))
                .count(),
            if capacity == 46 { 1 } else { 2 }
        );
        assert_eq!(
            f.disk().sealed_stones.len(),
            if capacity == 46 { 0 } else { 1 }
        );
        if capacity == 86 {
            let raw = inv
                .inventory_items
                .iter()
                .find(|i| i.stone_serial.is_some())
                .unwrap();
            assert_eq!((raw.container, raw.slot), (ItemContainer::Bag2, 0));
        }
        f.assert_checkpoint();
    }
}

#[test]
fn mined_stone_full_registry_skips_only_raw_and_preserves_ordinary_mining() {
    let mut f = mined_fixture("ledger-full");
    let (o, _) = owner(f.session.app.world()).unwrap();
    let mut registry = StoneRegistry::default();
    // Controlled historical authority fixture, assembled through validated
    // production mint plans. No live operator or production bypass is added.
    for n in 1..=mir2_production::STONE_MAX_RECORDS {
        let mint = StoneMintRequest {
            owner: o.clone(),
            request_id: format!("historical-{n}"),
            serial: n as u64,
            carrier_uid: 150_000_000 + n as u64,
            source_template_id: 836,
            raw_template_id: RAW_STONE_ROUGH,
            raw_weight: 4,
            grade: StoneGrade::Rough,
            binding: StoneBinding {
                flags: 0,
                soul_bound_id: -1,
            },
            source: StoneSource {
                kind: StoneSourceKind::Mining,
                source_version: "v1".into(),
                event_id: format!("historical-{n}"),
            },
        };
        let StoneMintPreparation::Change(plan) = registry
            .prepare_mint(&StoneCatalog::v1(), &mint, 1234)
            .unwrap()
        else {
            panic!("historical fixture must be fresh")
        };
        registry = plan.next_registry;
    }
    registry.validate().unwrap();
    f.config.account_store.lock().unwrap().sealed_stones = registry.clone();
    f.session.save_active_character().unwrap();
    let (mut zone, swing) = raw_swing_matching(&f, RAW_STONE_ROUGH, |s| s.ore().is_some());
    let before = f.live();
    let packets = f.session.apply_shared_mining_swing(&swing).unwrap();
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainedItem { .. }))
            .count(),
        1
    );
    assert_eq!(f.disk().sealed_stones, registry);
    assert_eq!(f.live().revision, before.revision + 1);
    assert_eq!(f.live().inventory_items_json.len(), 1);
    assert_ne!(f.live().equipment_items_json, before.equipment_items_json);
    zone.commit_mining_swing(&swing).unwrap();
    f.assert_checkpoint();
}

#[test]
fn mined_stone_known_failure_reuses_exact_uid_and_frozen_draw_on_retry() {
    let mut f = mined_fixture("known");
    let (mut zone, swing) = raw_swing_matching(&f, RAW_STONE_ROUGH, |s| s.ore().is_some());
    let before = f.live();
    let bytes = fs::read(&f.path).unwrap();
    f.config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(f.session.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), bytes);
    assert_eq!(
        serde_json::to_value(f.live()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    let event = swing.raw_stone_event_id();
    let plan = f.session.app.world().resource::<PendingMinedStones>().0[&event].clone();
    assert!(plan.ordinary.is_some());
    assert_ne!(
        plan.ordinary.as_ref().unwrap().unique_id,
        plan.item.unique_id
    );
    f.config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    assert!(f.session.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(
        f.session.app.world().resource::<PendingMinedStones>().0[&event].mint,
        plan.mint
    );
    assert_eq!(
        f.session.app.world().resource::<PendingMinedStones>().0[&event].next_registry,
        plan.next_registry
    );
    f.session.apply_shared_mining_swing(&swing).unwrap();
    zone.commit_mining_swing(&swing).unwrap();
    assert_eq!(f.disk().sealed_stones, plan.next_registry);
    assert_eq!(f.live().revision, before.revision + 1);
    assert_eq!(f.live().inventory_items_json.len(), 2);
    f.assert_checkpoint();
}

#[test]
fn mined_stone_unknown_publication_retains_one_durable_result_and_fences_retry() {
    let mut f = mined_fixture("unknown");
    let (_, swing) = raw_swing_matching(&f, RAW_STONE_PRECIOUS, |s| s.ore().is_some());
    let before = f.live();
    f.config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(f.session.apply_shared_mining_swing(&swing).is_err());
    assert!(f.config.ensure_account_store_writable().is_err());
    assert_eq!(
        serde_json::to_value(f.live()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    let committed = fs::read(&f.path).unwrap();
    let durable = f.disk();
    assert_eq!(durable.sealed_stones.records().count(), 1);
    assert_eq!(
        durable.accounts["demo"].saves[&0].revision,
        before.revision + 1
    );
    assert_eq!(
        durable.accounts["demo"].saves[&0]
            .inventory_items_json
            .len(),
        2
    );
    assert_ne!(
        durable.accounts["demo"].saves[&0].equipment_items_json,
        before.equipment_items_json
    );
    assert!(f.session.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), committed);
}

#[test]
fn mined_stone_raw_only_unknown_publication_blocks_zero_wear_retries_before_zone_commit() {
    let mut f = mined_fixture("raw-only-unknown");
    let (mut zone, swing) = raw_swing_matching(&f, RAW_STONE_ROUGH, |s| s.tool_damage() == 0);
    assert!(swing.ore().is_none());
    assert!(swing.raw_tool_damage() > 0);
    let before = f.live();
    let shared = zone.checkpoint_bytes().unwrap();
    f.config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let result = f
        .session
        .apply_shared_mining_swing(&swing)
        .map(|_| zone.commit_mining_swing(&swing).unwrap());
    assert!(result.is_err());
    assert!(f.config.ensure_account_store_writable().is_err());
    assert_eq!(zone.checkpoint_bytes().unwrap(), shared);
    assert_eq!(
        serde_json::to_value(f.live()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    let committed = fs::read(&f.path).unwrap();
    let durable = f.disk();
    assert_eq!(durable.sealed_stones.len(), 1);
    let saved = &durable.accounts["demo"].saves[&0];
    assert_eq!(saved.revision, before.revision + 1);
    assert_eq!(saved.inventory_items_json.len(), 1);
    assert_ne!(saved.equipment_items_json, before.equipment_items_json);

    let sid = f
        .session
        .active_zone_join_snapshot("mined-stone-fixture")
        .unwrap()
        .session_id;
    let next = (1..10_000u64)
        .find_map(|offset| {
            let ticket = zone.prepare_mining_swing(
                &sid,
                MirDirection::Up,
                &swing.tool,
                swing.now_ms() + offset * 2_000,
            )?;
            (ticket.raw_stone_template_id().is_some() && ticket.tool_damage() == 0)
                .then_some(ticket)
        })
        .expect("a later raw-only ticket exists while the failed Zone remains unchanged");
    assert_ne!(next.raw_stone_event_id(), swing.raw_stone_event_id());
    // Mirror the Gateway's personal-success gate: neither the exact replay nor
    // a new zero-normal-wear ticket may become an Ok(empty) Zone commit.
    for ticket in [&swing, &next] {
        let retry = f
            .session
            .apply_shared_mining_swing(ticket)
            .map(|_| zone.commit_mining_swing(ticket).unwrap());
        assert!(retry.is_err());
        assert_eq!(zone.checkpoint_bytes().unwrap(), shared);
        assert_eq!(fs::read(&f.path).unwrap(), committed);
        assert_eq!(
            serde_json::to_value(f.live()).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
    }
}

#[test]
fn mined_stone_full_bag_skips_registry_belt_and_extra_raw_only_wear() {
    let mut f = mined_fixture("full");
    let filler = crystal_item_by_index(824).unwrap();
    {
        let mut inv = f
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        inv.inventory_capacity = 46;
        for slot in 0..40 {
            let mut item = embedded_item_state_from_template(&filler, ItemContainer::Bag1, slot);
            item.unique_id = 140_000_000 + u64::from(slot);
            inv.inventory_items.push(item);
        }
        assert_eq!(free_bag_slots(&inv), 0);
        assert!(inv.belt_items.is_empty());
    }
    f.session.save_active_character().unwrap();
    let (_, swing) = raw_swing_matching(&f, RAW_STONE_ROUGH, |s| s.tool_damage() == 0);
    let before = f.live();
    let packets = f.session.apply_shared_mining_swing(&swing).unwrap();
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::GainedItem { .. })));
    assert_eq!(f.disk().sealed_stones.records().count(), 0);
    assert_eq!(f.live().inventory_items_json, before.inventory_items_json);
    assert_eq!(f.live().belt_items_json, before.belt_items_json);
    assert_eq!(f.live().equipment_items_json, before.equipment_items_json);
}

#[test]
fn mined_stone_fenced_uid_authority_never_falls_back_to_legacy() {
    for policy in [ItemUidIssuance::Unavailable, ItemUidIssuance::Fenced] {
        let mut f = mined_fixture("fenced");
        let (_, swing) = raw_swing(&f, RAW_STONE_ROUGH);
        let before = fs::read(&f.path).unwrap();
        f.session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .item_uid_issuance = policy;
        assert!(f.session.apply_shared_mining_swing(&swing).is_err());
        assert_eq!(fs::read(&f.path).unwrap(), before);
        assert_eq!(f.disk().sealed_stones.records().count(), 0);
    }
}

#[test]
fn mined_stone_legacy_and_durable_uid_skip_monthly_card_replay_range() {
    let reserved = crate::monthly_card::monthly_card_item_identity("fixture", 0, "history", 0).1;
    for durable in [false, true] {
        let mut f = mined_fixture("uid-reserved");
        // Retained history can already contain a legitimate monthly-card UID.
        // This test isolates the floor calculation with a controlled carrier.
        let mut historical = embedded_item_state_from_template(
            &crystal_item_by_index(824).unwrap(),
            ItemContainer::Bag1,
            0,
        );
        historical.unique_id = reserved;
        f.session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(historical);
        f.session.save_active_character().unwrap();
        let allocator = if durable {
            let allocator = crate::UserItemUidAllocator::initialize_file(
                f.path.with_file_name("uids.json"),
                reserved + 1,
            )
            .unwrap();
            f.config = f
                .config
                .clone()
                .with_item_uid_allocator(allocator.clone())
                .unwrap();
            f.session
                .app
                .world_mut()
                .resource_mut::<RuntimeConfigResource>()
                .config = f.config.clone();
            f.session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .item_uid_issuance = ItemUidIssuance::Durable(allocator.clone());
            Some(allocator)
        } else {
            None
        };
        let (_, swing) = raw_swing(&f, RAW_STONE_ROUGH);
        f.session.apply_shared_mining_swing(&swing).unwrap();
        let uid = f
            .disk()
            .sealed_stones
            .records()
            .next()
            .unwrap()
            .carrier_uid();
        assert!(uid > reserved);
        assert!(!crate::monthly_card::monthly_card_item_uid_is_reserved(uid));
        if let Some(allocator) = allocator {
            assert_eq!(allocator.issued_through().unwrap(), uid);
        }
        f.assert_checkpoint();
    }
}

#[test]
fn mined_stone_broken_pick_saves_clamped_vitals_with_the_award() {
    let mut f = mined_fixture("break-vitals");
    {
        let mut inv = f
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        let pick = &mut inv.equipment_items[0];
        pick.durability_current = 1;
        pick.added_stats.push(mir2_protocol::UserItemStat {
            stat: crate::runtime::crystal_compat::CRYSTAL_STAT_HP,
            value: 100,
        });
    }
    crate::runtime::stats::refresh_player_stats(f.session.app.world_mut());
    let mut boosted = f.live();
    boosted.hp = boosted.max_hp;
    f.session
        .restore_active_character_checkpoint(&boosted)
        .unwrap();
    f.session.save_active_character().unwrap();
    let (_, swing) = raw_swing(&f, RAW_STONE_ROUGH);
    let before = f.live();
    let packets = f.session.apply_shared_mining_swing(&swing).unwrap();
    let live = f.live();
    let durable = &f.disk().accounts["demo"].saves[&0];
    assert!(live.max_hp < before.max_hp);
    assert_eq!(
        (live.hp, live.max_hp, live.mp, live.max_mp),
        (durable.hp, durable.max_hp, durable.mp, durable.max_mp)
    );
    assert_eq!(live.hp, live.max_hp);
    assert!(packets
        .iter()
        .any(|p| matches!(p, ServerPacket::HealthChanged { hp, .. } if *hp == live.hp)));
    assert_eq!(f.disk().sealed_stones.records().count(), 1);
    f.assert_checkpoint();
}

#[test]
fn mined_stone_new_seal_is_rejected_but_historical_mint_receipt_is_replayable() {
    let mut f = Fixture::new("old-seal", 824);
    let command = f.command("new-seal", Op::Seal { uid: 120_000_001 });
    let before = fs::read(&f.path).unwrap();
    assert_eq!(
        f.session.execute_stone_workshop(command).unwrap_err(),
        Error::InvalidRequest
    );
    assert_eq!(fs::read(&f.path).unwrap(), before);
    let serial = f.legacy_sealed_fixture();
    let frozen = f.disk().sealed_stones.record(serial).unwrap().clone();
    let replay = f.command("seal", Op::Seal { uid: 120_000_001 });
    let before = fs::read(&f.path).unwrap();
    assert!(f.session.execute_stone_workshop(replay).unwrap().replayed);
    assert_eq!(fs::read(&f.path).unwrap(), before);
    assert_eq!(f.disk().sealed_stones.record(serial).unwrap(), &frozen);
}

#[test]
fn mined_stone_file_cold_login_keeps_hidden_result_and_can_cut_original_carrier() {
    let mut f = mined_fixture("cold");
    let (_, swing) = raw_swing(&f, RAW_STONE_ROUGH);
    f.session.apply_shared_mining_swing(&swing).unwrap();
    return_to_workshop(&mut f);
    let record = f.disk().sealed_stones.records().next().unwrap().clone();
    let path = f.path.clone();
    let npc = f.npc;
    f.session.handle_packet(ClientPacket::LogOut);
    drop(f);
    let config = SimulationConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_path(&path);
    let mut session = login(config.clone());
    session.begin_stone_workshops().unwrap();
    session.interact(npc);
    let view = session.stone_workshop_view().unwrap();
    assert_eq!(view.stones.len(), 1);
    let loaded: crate::AccountStore = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        loaded
            .sealed_stones
            .record(record.mint_request().serial)
            .unwrap(),
        &record
    );
    let cut = StoneWorkshopCommand {
        request_id: "mined-cold-cut".into(),
        expected_inventory_revision: view.inventory_revision,
        operation: Op::Cut {
            serial: record.mint_request().serial,
            expected_stone_revision: 0,
        },
    };
    session.execute_stone_workshop(cut).unwrap();
    let inventory = session.app.world().resource::<InventoryResource>();
    let result = inventory
        .inventory_items
        .iter()
        .find(|i| i.unique_id == record.carrier_uid())
        .unwrap();
    assert_eq!(result.stone_serial, None);
    assert_eq!(
        try_user_item_from_item_state(result).unwrap().item_index,
        record.private_output().template_id
    );
}

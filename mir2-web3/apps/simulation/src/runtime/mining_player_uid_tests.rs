//! Shared mining uses a real prepared Zone swing and the original mining table.
//! The bounded wall is synthetic; UID faults use an observable mock/File source.
use super::super::items::craft_uid_transaction_tests::{inventory_image, Fixture};
use super::*;
use crate::{
    AccountStore, AccountStoreTransactionFault, SessionId, ZoneBounds, ZoneCollision, ZoneCommand,
    ZoneJoin, ZoneKey, ZoneRuntime,
};
use mir2_protocol::{MirClass, MirDirection, MirGender, Point};
use std::{fs, sync::atomic::Ordering};

fn prepared(label: &str) -> (Fixture, ZoneRuntime, ZoneMiningSwing) {
    let mut fixture = Fixture::new(label);
    let template = crystal_item_by_name("PickAxe").unwrap();
    let mut carrier =
        super::super::items::embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    carrier.unique_id = 800_001;
    let pickaxe =
        super::super::equipment::equipment_state_from_item_state(&carrier, EquipmentSlot::Weapon);
    {
        let mut inventory = fixture
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        let weapon = inventory
            .equipment_items
            .iter_mut()
            .find(|item| item.slot == EquipmentSlot::Weapon)
            .unwrap();
        // Use the actual equip conversion so an exact UID also retains its
        // template index and canonical metadata, as production equipment does.
        *weapon = pickaxe;
    }
    fixture.session.save_active_character().unwrap();
    let checkpoint = fixture.session.active_character_checkpoint().unwrap();
    let identity = fixture.session.active_identity().unwrap();
    let position = checkpoint.position.clone();
    let wall = Point {
        x: position.x,
        y: position.y - 1,
    };
    let collision = ZoneCollision::unbounded()
        .with_bounds(ZoneBounds::new(-1000, 1000, -1000, 1000))
        .with_blocked_cells([wall]);
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("D401"), collision);
    let session_id = SessionId::new("owned-shared-mining-uid");
    let object_id =
        super::super::components::current_player_object_id(fixture.session.app.world()).unwrap();
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: session_id.clone(),
        account_id: identity.account_id,
        character_index: identity.character_index,
        object_id,
        name: identity.character_name,
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 30,
        hp: checkpoint.hp,
        max_hp: checkpoint.max_hp,
        mp: checkpoint.mp,
        map_file_name: "D401".into(),
        position,
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    zone.handle(ZoneCommand::sync_player_combat_state(
        session_id.clone(),
        MirClass::Warrior,
        false,
        false,
        false,
        false,
        false,
        false,
    ));
    let tool = fixture.session.shared_mining_tool().unwrap();
    // Pick a timestamp that refills the original table with at least one stone.
    // No rates or prepared capability fields are modified.
    let (refill_at, refill) = (1_000_000..1_004_096)
        .find_map(|at| {
            let swing = zone.prepare_mining_swing(&session_id, MirDirection::Up, &tool, at)?;
            let image = serde_json::to_value(&swing).unwrap();
            (image["after"]["stones_left"].as_u64().unwrap() > 0).then_some((at, swing))
        })
        .expect("bounded source-table refill branch");
    zone.commit_mining_swing(&refill).unwrap();
    let ready = refill_at + tool.attack_delay_ms.max(550);
    let swing = (ready..ready + 16_384)
        .find_map(|at| {
            let swing = zone.prepare_mining_swing(&session_id, MirDirection::Up, &tool, at)?;
            (swing.ore().is_some() && swing.tool_damage() > 0).then_some(swing)
        })
        .expect("bounded original-table ore branch");
    (fixture, zone, swing)
}

#[test]
fn shared_mining_uid_failure_preserves_tool_inventory_file_and_uncommitted_wall() {
    let (mut fixture, zone, swing) = prepared("shared-mine-reject");
    let before = inventory_image(&fixture.session);
    let file = fs::read(&fixture.path).unwrap();
    let wall = zone.checkpoint_bytes().unwrap();
    let high = fixture.store.high.load(Ordering::SeqCst);
    fixture.store.arm(1, false);
    assert!(fixture.session.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(inventory_image(&fixture.session), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), file);
    assert_eq!(fixture.store.high.load(Ordering::SeqCst), high);
    assert_eq!(
        zone.checkpoint_bytes().unwrap(),
        wall,
        "personal failure does not authorize the caller's wall commit"
    );
}

#[test]
fn shared_mining_uid_success_persists_ore_and_exact_wear_before_wall_commit() {
    let (mut fixture, mut zone, swing) = prepared("shared-mine-success");
    let high = fixture.store.high.load(Ordering::SeqCst);
    let wall = zone.checkpoint_bytes().unwrap();
    let expected_dura = swing.tool.durability - swing.tool_damage();
    let packets = fixture.session.apply_shared_mining_swing(&swing).unwrap();
    let items: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::GainedItem { item } => Some(item),
            _ => None,
        })
        .collect();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].unique_id, high + 1);
    assert_eq!(items[0].current_dura, swing.ore().unwrap().current_dura);
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::DuraChanged { unique_id, current_dura } if *unique_id == swing.tool.unique_id && *current_dura == expected_dura)));
    let durable: AccountStore = serde_json::from_slice(&fs::read(&fixture.path).unwrap()).unwrap();
    let identity = fixture.session.active_identity().unwrap();
    let saved = &durable.accounts[&identity.account_id].saves[&identity.character_index];
    let active = fixture.session.active_character_checkpoint().unwrap();
    assert_eq!(saved.inventory_items_json, active.inventory_items_json);
    assert_eq!(saved.belt_items_json, active.belt_items_json);
    assert_eq!(saved.equipment_items_json, active.equipment_items_json);
    assert_eq!(zone.checkpoint_bytes().unwrap(), wall);
    zone.commit_mining_swing(&swing).unwrap();
    assert_ne!(zone.checkpoint_bytes().unwrap(), wall);
}

#[test]
fn shared_mining_uid_save_failure_burns_ore_identity_but_restores_tool_and_assets() {
    let (mut fixture, zone, swing) = prepared("shared-mine-save-failure");
    let before = inventory_image(&fixture.session);
    let file = fs::read(&fixture.path).unwrap();
    let wall = zone.checkpoint_bytes().unwrap();
    let high = fixture.store.high.load(Ordering::SeqCst);
    fixture
        .config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(fixture.session.apply_shared_mining_swing(&swing).is_err());
    assert_eq!(inventory_image(&fixture.session), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), file);
    assert_eq!(fixture.store.high.load(Ordering::SeqCst), high + 1);
    assert_eq!(zone.checkpoint_bytes().unwrap(), wall);
}

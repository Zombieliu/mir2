//! Controlled File authority and real inventory planners; no player/UI rollout.
use super::super::inventory::{plan_npc_gold_trade_gain, split_item_impl};
use super::super::items::embedded_item_state_from_template;
use super::super::resources::{NpcStateResource, RuntimeConfigResource};
use super::*;
use crate::{ItemContainer, SimulationSession};
use mir2_protocol::{ClientPacket, MirGridType, Point, ServerPacket};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn path(label: &str) -> PathBuf {
    let root = std::env::temp_dir().canonicalize().unwrap();
    let directory = root.join(format!(
        "mir2-uid-p02b-{label}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(directory.starts_with(root));
    fs::create_dir(&directory).unwrap();
    directory.join("uids.json")
}
fn authority(label: &str, high: u64) -> (UserItemUidAllocator, PathBuf) {
    let path = path(label);
    (
        UserItemUidAllocator::initialize_file(&path, high).unwrap(),
        path,
    )
}
fn inventory(allocator: &UserItemUidAllocator) -> InventoryResource {
    let mut inventory = InventoryResource::new(80);
    inventory.inventory_capacity = 46;
    inventory.item_uid_issuance = ItemUidIssuance::Durable(allocator.clone());
    inventory
}
fn split_session(allocator: &UserItemUidAllocator, storage: bool) -> SimulationSession {
    let config = SimulationConfig::default()
        .with_item_uid_allocator(allocator.clone())
        .unwrap();
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let template = mir2_game_data::crystal_item_by_index(658).unwrap();
    let container = if storage {
        ItemContainer::Storage
    } else {
        ItemContainer::Bag1
    };
    let mut item = embedded_item_state_from_template(&template, container, 0);
    item.unique_id = 800_001;
    item.quantity = 5;
    let mut inv = inventory(allocator);
    if storage {
        inv.storage_items.push(item);
    } else {
        inv.inventory_items.push(item);
    }
    *session.app.world_mut().resource_mut::<InventoryResource>() = inv;
    if storage {
        use super::super::components::{Npc, ObjectId, Position};
        let world = session.app.world();
        let pos = super::super::components::entity_position(
            world,
            super::super::components::player_entity(world).unwrap(),
        )
        .unwrap();
        session.app.world_mut().spawn((
            Npc,
            ObjectId(4999),
            Position(Point { x: pos.x, y: pos.y }),
        ));
        session
            .app
            .world_mut()
            .resource_mut::<NpcStateResource>()
            .active_npc_service = Some(super::super::npc::ActiveNpcServiceState {
            script_key: "uid-test".into(),
            label_key: "STORAGE".into(),
            npc_object_id: 4999,
        });
    }
    session
}

#[test]
fn history_refresh_keeps_legacy_worlds_and_rejects_missing_or_mismatched_durable_config() {
    let mut world = bevy_ecs::prelude::World::new();
    assert!(refresh_world_history_floor(&world).is_err());
    world.insert_resource(InventoryResource::new(80));
    assert!(refresh_world_history_floor(&world).is_ok());

    let (allocator, _) = authority("config-resource-boundary", 30_000);
    world.insert_resource(inventory(&allocator));
    let before = allocator.issued_through().unwrap();
    assert!(refresh_world_history_floor(&world).is_err());
    assert_eq!(allocator.issued_through().unwrap(), before);

    world.insert_resource(RuntimeConfigResource {
        config: SimulationConfig::default(),
    });
    assert!(refresh_world_history_floor(&world).is_err());
    assert_eq!(allocator.issued_through().unwrap(), before);
}

#[test]
fn uid_history_floor_includes_equipment_custody_sockets_and_receipt_only_merged_id() {
    let image = serde_json::json!({
        "equipment": " \n [{\"user_item_unique_id\":8000}]",
        "custody": " {\"unique_id\":9000,\"slots\":[{\"uniqueId\":10000}]}",
        "receipt": {"incomingUniqueId": "18446744073709551614"},
        "retired": {"incoming_unique_id": 20000}
    });
    assert_eq!(historical_uid_floor(&image, 0).unwrap(), u64::MAX - 1);
    assert!(historical_uid_floor(&serde_json::json!({"uniqueId":"0002"}), 0).is_err());
    assert!(historical_uid_floor(&serde_json::json!({"uniqueId":-1}), 0).is_err());
    assert!(historical_uid_floor(&image, 65).is_err());
}

#[test]
fn staged_purchase_clones_share_file_authority_and_aborted_plan_cannot_reissue_uid() {
    let (allocator, path) = authority("shared-clones", 10_000);
    let config = SimulationConfig::default()
        .with_item_uid_allocator(allocator.clone())
        .unwrap();
    let first = SimulationSession::new(config.clone());
    let second = SimulationSession::new(config);
    let template = mir2_game_data::crystal_item_by_index(658).unwrap();
    let a = plan_npc_gold_trade_gain(
        first.app.world().resource::<InventoryResource>(),
        &template,
        1,
        90,
        0,
    )
    .unwrap()
    .1;
    let b = plan_npc_gold_trade_gain(
        second.app.world().resource::<InventoryResource>(),
        &template,
        1,
        90,
        0,
    )
    .unwrap()
    .1;
    assert!(a.unique_id > 10_000);
    assert!(b.unique_id > a.unique_id);
    drop(first);
    drop(second);
    drop(allocator);
    let reopened = UserItemUidAllocator::open_file(path).unwrap();
    let c = issue_for_staged_inventory(
        &inventory(&reopened),
        UserItemUidReason::NpcTradePurchase,
        ItemContainer::Bag1,
        0,
    )
    .unwrap();
    assert!(c > b.unique_id);
}

#[test]
fn uid_issuance_raises_reserved_custody_floor_and_catalog_selector_is_not_history() {
    let (allocator, _) = authority("reserved", 0);
    let mut inv = inventory(&allocator);
    inv.reserved_item_unique_ids.insert(900_000);
    let template = mir2_game_data::crystal_item_by_index(658).unwrap();
    let incoming = plan_npc_gold_trade_gain(&inv, &template, 1, u64::MAX, 0)
        .unwrap()
        .1;
    assert_eq!(incoming.unique_id, 900_001);
    assert_eq!(allocator.issued_through().unwrap(), 900_001);
    // A selector equal to the next allocation is skipped, without becoming a
    // floor for the actual inventory. The excluded ID is permanently retired.
    let next = plan_npc_gold_trade_gain(&inv, &template, 1, 900_002, 0)
        .unwrap()
        .1;
    assert_eq!(next.unique_id, 900_003);
    assert_eq!(inv.reserved_item_unique_ids, [900_000].into());
}

#[test]
fn corrupt_or_missing_uid_sidecar_rejects_staged_purchase_without_local_fallback() {
    for corrupt in [false, true] {
        let (allocator, path) = authority(if corrupt { "corrupt" } else { "missing" }, 1_000);
        let inv = inventory(&allocator);
        if corrupt {
            fs::write(&path, b"{}").unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        let template = mir2_game_data::crystal_item_by_index(658).unwrap();
        assert!(plan_npc_gold_trade_gain(&inv, &template, 1, 90, 0).is_none());
        assert!(inv.inventory_items.is_empty());
        assert!(inv.belt_items.is_empty());
    }
}

#[test]
fn inventory_and_storage_split_issue_fresh_global_uid_before_consuming_source() {
    for storage in [false, true] {
        let (allocator, _) = authority(
            if storage {
                "storage-split"
            } else {
                "bag-split"
            },
            1_000,
        );
        let mut session = split_session(&allocator, storage);
        let grid = if storage {
            MirGridType::Storage
        } else {
            MirGridType::Inventory
        };
        let packets = split_item_impl(session.app.world_mut(), grid, 800_001, 2);
        assert!(packets
            .iter()
            .any(|p| matches!(p, ServerPacket::SplitItem1 { success: true, .. })));
        let inv = session.app.world().resource::<InventoryResource>();
        let items: Vec<_> = if storage {
            inv.storage_items.iter().collect()
        } else {
            inv.inventory_items
                .iter()
                .chain(inv.belt_items.iter())
                .collect()
        };
        assert_eq!(items.len(), 2);
        assert_eq!((items[0].unique_id, items[0].quantity), (800_001, 3));
        assert_eq!(items[1].quantity, 2);
        assert!(items[1].unique_id > 800_001);
        assert_ne!(
            items[1].unique_id,
            super::super::items::default_item_unique_id(items[1].container, items[1].slot)
        );
        assert_eq!(allocator.issued_through().unwrap(), items[1].unique_id);
    }
}

#[test]
fn exhausted_uid_authority_cannot_consume_source_or_publish_successful_split() {
    for storage in [false, true] {
        let (allocator, _) = authority(
            if storage {
                "storage-exhausted"
            } else {
                "bag-exhausted"
            },
            0,
        );
        let mut session = split_session(&allocator, storage);
        allocator.ensure_issued_through_at_least(u64::MAX).unwrap();
        let grid = if storage {
            MirGridType::Storage
        } else {
            MirGridType::Inventory
        };
        let before = session.world_snapshot();
        let packets = split_item_impl(session.app.world_mut(), grid, 800_001, 2);
        assert!(matches!(
            packets.as_slice(),
            [ServerPacket::SplitItem1 { success: false, .. }]
        ));
        assert_eq!(session.world_snapshot(), before);
        assert_eq!(allocator.issued_through().unwrap(), u64::MAX);
    }
}

#[test]
fn isolated_replay_cannot_mint_or_reacquire_live_authority() {
    let (allocator, _) = authority("replay", 1000);
    let config = SimulationConfig::default()
        .with_item_uid_allocator(allocator.clone())
        .unwrap();
    for fork in [
        config.fork_with_isolated_account_store().unwrap(),
        config.fork_for_replica_apply().unwrap(),
    ] {
        assert!(matches!(
            fork.item_uid_issuance,
            ItemUidIssuance::Unavailable
        ));
        assert!(fork
            .clone()
            .with_item_uid_allocator(allocator.clone())
            .is_err());
        let session = SimulationSession::new(fork);
        assert!(issue_for_staged_inventory(
            session.app.world().resource::<InventoryResource>(),
            UserItemUidReason::NpcTradePurchase,
            ItemContainer::Bag1,
            0
        )
        .is_err());
    }
    assert_eq!(allocator.issued_through().unwrap(), 1000);
}

#[test]
fn rebind_and_save_restore_preserve_server_binding_and_cannot_downgrade_to_slots() {
    let (allocator, _) = authority("restore", 1000);
    let mut session = split_session(&allocator, false);
    let save = super::super::save::snapshot_active_character_save(session.app.world()).unwrap();
    session.restore_active_character_checkpoint(&save).unwrap();
    session
        .app
        .world()
        .resource::<InventoryResource>()
        .item_uid_issuance
        .require_same(&allocator)
        .unwrap();
    assert!(allocator.issued_through().unwrap() >= 800_001);
    session.rebind_account_store(&SimulationConfig::default());
    assert!(matches!(
        session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .item_uid_issuance,
        ItemUidIssuance::Fenced
    ));
    let packets = split_item_impl(session.app.world_mut(), MirGridType::Inventory, 800_001, 2);
    assert!(matches!(
        packets.as_slice(),
        [ServerPacket::SplitItem1 { success: false, .. }]
    ));
    assert_eq!(
        session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items[0]
            .quantity,
        5
    );
}

#[test]
fn server_binding_rejects_replacement_authority_and_postgres_bypass() {
    let (allocator, _) = authority("primary", 0);
    let (other, _) = authority("replacement", 0);
    let config = SimulationConfig::default()
        .with_item_uid_allocator(allocator.clone())
        .unwrap();
    assert!(config.clone().with_item_uid_allocator(other).is_err());
    assert!(config
        .clone()
        .with_item_uid_allocator(allocator.clone())
        .is_ok());
    let mut postgres = SimulationConfig::default();
    postgres.account_store_database_url =
        Some("postgresql://never-contact-fixture.invalid/test".into());
    assert!(postgres.with_item_uid_allocator(allocator).is_err());
}

#[test]
fn invalid_live_rebind_stays_fenced_but_trusted_shadow_promotion_can_bind() {
    let (allocator, _) = authority("rebind-first", 1000);
    let (other, _) = authority("rebind-other", 1000);
    let primary = SimulationConfig::default()
        .with_item_uid_allocator(allocator.clone())
        .unwrap();
    let replacement = SimulationConfig::default()
        .with_item_uid_allocator(other)
        .unwrap();
    let mut session = SimulationSession::new(primary.clone());
    session.rebind_account_store(&replacement);
    session.rebind_account_store(&replacement);
    session.rebind_account_store(&primary);
    assert!(matches!(
        session
            .app
            .world()
            .resource::<InventoryResource>()
            .item_uid_issuance,
        ItemUidIssuance::Fenced
    ));
    assert!(issue_for_staged_inventory(
        session.app.world().resource::<InventoryResource>(),
        UserItemUidReason::NpcTradePurchase,
        ItemContainer::Bag1,
        0
    )
    .is_err());
    let fork = session
        .app
        .world()
        .resource::<RuntimeConfigResource>()
        .config
        .fork_for_replica_apply()
        .unwrap();
    assert!(matches!(fork.item_uid_issuance, ItemUidIssuance::Fenced));
    let mut shadow = SimulationSession::new(primary.fork_for_replica_apply().unwrap());
    shadow.rebind_account_store(&primary);
    shadow
        .app
        .world()
        .resource::<InventoryResource>()
        .item_uid_issuance
        .require_same(&allocator)
        .unwrap();
}

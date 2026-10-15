//! A shared receipt service must not replay a previous map's transaction.
use super::*;

fn context(zone: &str, generation: u64) -> SharedAccountInventoryExecutionContext {
    SharedAccountInventoryExecutionContext {
        zone_id: ZoneId::new(zone),
        fencing_generation: generation,
        source_sequence: 7,
        created_at_ms: 100,
        external_commit_authorized: true,
    }
}
fn drop_location(receipt: &SharedAccountInventoryTransactionReceipt) -> Point {
    assert!(receipt.committed);
    receipt
        .packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectItem { info } => Some(info.location.clone()),
            _ => None,
        })
        .expect("a committed item debit produces its actual map ObjectItem")
}
fn exercise_item_scope(fenced: bool) {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(state);
    start_demo_runtime(&mut runtime);
    let service = InProcessAccountInventoryService::new();
    runtime
        .inner
        .execute(WorldCommand::TransferMap {
            key: "crystal:0:288:612".into(),
        })
        .unwrap();
    let before = runtime.inner.world_snapshot();
    let item = before
        .inventory_items
        .iter()
        .find(|item| item.key == "red-potion" && item.quantity >= 2)
        .expect("demo fixture contains a real stack for two distinct item debits");
    let uid = item.unique_id;
    let count = item.quantity;
    let envelope = SharedAccountInventoryCommandEnvelope {
        identity: runtime.inner.active_identity().unwrap(),
        command: SharedAccountInventoryCommand::InventoryItemDrop {
            drop: runtime
                .inner
                .shared_inventory_item_drop(uid, 1, false)
                .unwrap(),
            request_id: 7,
        },
    };
    let quantity = |runtime: &InProcessWorldRuntime| {
        runtime
            .world_snapshot()
            .inventory_items
            .into_iter()
            .find(|item| item.unique_id == uid)
            .map_or(0, |item| item.quantity)
    };
    let first_context = context("map:0", 1);
    let first = service
        .commit_in_zone(
            &mut runtime.inner,
            &first_context.zone_id,
            fenced.then_some(&first_context),
            envelope.clone(),
        )
        .into_receipt();
    let first_position = drop_location(&first);
    assert_eq!(quantity(&runtime.inner), count - 1);
    let next_epoch = context("map:0", 2);
    let replay = service
        .commit_in_zone(
            &mut runtime.inner,
            &next_epoch.zone_id,
            fenced.then_some(&next_epoch),
            envelope.clone(),
        )
        .into_receipt();
    assert_eq!(
        replay, first,
        "a new fencing generation must not repeat an already committed same-zone debit"
    );
    assert_eq!(quantity(&runtime.inner), count - 1);
    runtime
        .inner
        .execute(WorldCommand::TransferMap {
            key: "crystal:1:163:164".into(),
        })
        .unwrap();
    let actual = runtime
        .inner
        .world_snapshot()
        .entities
        .into_iter()
        .find(|e| e.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    let second_context = context("map:1", 1);
    let second = service
        .commit_in_zone(
            &mut runtime.inner,
            &second_context.zone_id,
            fenced.then_some(&second_context),
            envelope.clone(),
        )
        .into_receipt();
    let second_position = drop_location(&second);
    // Crystal searches neighboring cells for a legal drop location; it does
    // not require the item to occupy the player's own tile.
    assert!(
        (second_position.x - actual.x)
            .abs()
            .max((second_position.y - actual.y).abs())
            <= 1,
        "the returned drop belongs to a legal neighboring cell on the current map"
    );
    let object_id = second
        .packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectItem { info } => Some(info.object_id),
            _ => None,
        })
        .unwrap();
    let live_drop = runtime
        .inner
        .world_snapshot()
        .ground_drops
        .into_iter()
        .find(|drop| drop.object_id == object_id)
        .expect("the receipt's ObjectItem must exist in the actual current-map drop state");
    assert_eq!(
        Point {
            x: live_drop.x,
            y: live_drop.y
        },
        second_position
    );
    assert_eq!(live_drop.quantity, 1);
    assert_ne!(
        second_position, first_position,
        "another map must not return old-map cached coordinates"
    );
    assert_eq!(
        quantity(&runtime.inner),
        count - 2,
        "the second zone performs its own actual mutation"
    );
    let replay = service
        .commit_in_zone(
            &mut runtime.inner,
            &second_context.zone_id,
            fenced.then_some(&second_context),
            envelope,
        )
        .into_receipt();
    assert_eq!(replay, second);
    assert_eq!(quantity(&runtime.inner), count - 2);
    assert_eq!(
        runtime.inner.world_snapshot().gold,
        before.gold,
        "item receipt replay cannot affect gold"
    );
}
#[test]
fn fenced_item_receipts_are_scoped_by_zone_but_not_fencing_generation() {
    exercise_item_scope(true);
}
#[test]
fn direct_item_receipts_use_actual_map_without_fabricating_fencing_context() {
    exercise_item_scope(false);
}
#[test]
fn fenced_gold_receipts_debit_each_zone_once() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(state);
    start_demo_runtime(&mut runtime);
    let service = InProcessAccountInventoryService::new();
    let before = runtime.inner.world_snapshot().gold;
    assert!(before >= 20);
    let envelope = SharedAccountInventoryCommandEnvelope {
        identity: runtime.inner.active_identity().unwrap(),
        command: SharedAccountInventoryCommand::GoldDrop {
            amount: 10,
            request_id: 7,
        },
    };
    for (index, zone) in ["map:0", "map:1"].into_iter().enumerate() {
        let first = service
            .commit_in_zone(
                &mut runtime.inner,
                &ZoneId::new(zone),
                Some(&context(zone, 1)),
                envelope.clone(),
            )
            .into_receipt();
        assert!(first.committed);
        let replay = service
            .commit_in_zone(
                &mut runtime.inner,
                &ZoneId::new(zone),
                Some(&context(zone, 2)),
                envelope.clone(),
            )
            .into_receipt();
        assert_eq!(first, replay);
        assert_eq!(
            runtime.inner.world_snapshot().gold,
            before - 10 * (index as u32 + 1)
        );
    }
}

#[test]
fn direct_and_journaled_sequences_have_distinct_receipt_domains_in_one_zone() {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(state);
    start_demo_runtime(&mut runtime);
    let service = InProcessAccountInventoryService::new();
    let before = runtime.inner.world_snapshot().gold;
    let zone = ZoneId::new("map:0");
    let envelope = SharedAccountInventoryCommandEnvelope {
        identity: runtime.inner.active_identity().unwrap(),
        command: SharedAccountInventoryCommand::GoldDrop {
            amount: 10,
            request_id: 7,
        },
    };
    let direct = service
        .commit_in_zone(&mut runtime.inner, &zone, None, envelope.clone())
        .into_receipt();
    assert!(direct.committed);
    let journaled = service
        .commit_in_zone(
            &mut runtime.inner,
            &zone,
            Some(&context("map:0", 1)),
            envelope.clone(),
        )
        .into_receipt();
    assert!(journaled.committed);
    assert_eq!(runtime.inner.world_snapshot().gold, before - 20);
    assert_eq!(
        service
            .commit_in_zone(&mut runtime.inner, &zone, None, envelope.clone())
            .into_receipt(),
        direct
    );
    assert_eq!(
        service
            .commit_in_zone(
                &mut runtime.inner,
                &zone,
                Some(&context("map:0", 2)),
                envelope
            )
            .into_receipt(),
        journaled
    );
    assert_eq!(runtime.inner.world_snapshot().gold, before - 20);
}

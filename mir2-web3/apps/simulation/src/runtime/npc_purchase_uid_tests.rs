//! Same actual File-source purchase fixtures, with one server UID binding.
use super::*;
use crate::UserItemUidAllocator;

fn bind(fixture: &mut Fixture) -> (UserItemUidAllocator, PathBuf) {
    let path = fixture.path.parent().unwrap().join("uids.json");
    let allocator = UserItemUidAllocator::initialize_file(&path, 1_000_000).unwrap();
    fixture.config = fixture
        .config
        .clone()
        .with_item_uid_allocator(allocator.clone())
        .unwrap();
    fixture.session.rebind_account_store(&fixture.config);
    (allocator, path)
}

#[test]
fn global_uid_five_durable_purchase_branches_share_binding_and_keep_metadata() {
    for (label, currency, source) in [
        (
            "BUYSELL",
            NpcPurchaseCurrency::Gold,
            NpcPurchaseSource::Trade,
        ),
        (
            "PEARLBUY",
            NpcPurchaseCurrency::Pearls,
            NpcPurchaseSource::Trade,
        ),
        (
            "BUYBACK",
            NpcPurchaseCurrency::Gold,
            NpcPurchaseSource::BuyBack,
        ),
        (
            "BUYUSED",
            NpcPurchaseCurrency::Gold,
            NpcPurchaseSource::Used,
        ),
        (
            "PEARLBUY",
            NpcPurchaseCurrency::Pearls,
            NpcPurchaseSource::Used,
        ),
    ] {
        let mut fixture = Fixture::new("global-five", label, source);
        let (allocator, _) = bind(&mut fixture);
        let operation = fixture.operation(1);
        let before = allocator.issued_through().unwrap();
        let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
        assert_commit(&execution, operation, currency, source);
        let incoming = execution
            .packets
            .iter()
            .find_map(|p| match p {
                ServerPacket::GainedItem { item } => Some(item),
                _ => None,
            })
            .unwrap();
        assert!(incoming.unique_id > before);
        assert_eq!(incoming.item_index, 658);
        assert_eq!(incoming.soul_bound_id, -1);
        assert_eq!(incoming.count, 2);
        assert_eq!(incoming.current_dura, source_item(0, 2).current_dura);
        assert_eq!(allocator.issued_through().unwrap(), incoming.unique_id);
        fixture.assert_checkpoint();
    }
}

#[test]
fn global_uid_authority_failure_cannot_charge_currency_remove_resale_or_gain_item() {
    for (label, source) in [
        ("BUYSELL", NpcPurchaseSource::Trade),
        ("PEARLBUY", NpcPurchaseSource::Trade),
        ("BUYBACK", NpcPurchaseSource::BuyBack),
        ("BUYUSED", NpcPurchaseSource::Used),
        ("PEARLBUY", NpcPurchaseSource::Used),
    ] {
        let mut fixture = Fixture::new("global-failure", label, source);
        let (_, path) = bind(&mut fixture);
        let operation = fixture.operation(1);
        let before = economic_checkpoint(&fixture.save_from_file());
        fs::write(&path, b"{}").unwrap();
        let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
        assert!(matches!(
            execution.receipt.entry.outcome,
            NpcPurchaseProcessingOutcome::Rejected {
                reason: NpcPurchaseRejection::InvalidDelivery,
                ..
            }
        ));
        assert!(execution.packets.iter().all(|p| !matches!(
            p,
            ServerPacket::LoseGold { .. } | ServerPacket::GainedItem { .. }
        )));
        assert_eq!(economic_checkpoint(&fixture.save_from_file()), before);
        assert_eq!(
            economic_checkpoint(
                &snapshot_active_character_save(fixture.session.app.world()).unwrap()
            ),
            before
        );
        fixture.assert_checkpoint();
    }
}

#[test]
fn global_uid_exact_purchase_replay_uses_original_receipt_even_when_mint_is_unavailable() {
    let mut fixture = Fixture::new("global-replay", "BUYSELL", NpcPurchaseSource::Trade);
    let (allocator, path) = bind(&mut fixture);
    let operation = fixture.operation(1);
    let committed = fixture.session.try_durable_npc_purchase(operation).unwrap();
    let before = fixture.live();
    let high = allocator.issued_through().unwrap();
    fs::write(&path, b"{}").unwrap();
    assert_eq!(
        fixture.session.query_npc_purchase(operation).unwrap(),
        Some(committed.receipt.clone())
    );
    let replay = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert!(replay.replayed);
    assert!(replay.packets.is_empty());
    assert_eq!(replay.receipt, committed.receipt);
    assert_eq!(fixture.live(), before);
    assert!(high > 1_000_000);
    assert_eq!(fs::read(path).unwrap(), b"{}");
}

#[test]
fn global_uid_failed_account_publication_burns_planned_id_and_retry_issues_another() {
    let mut fixture = Fixture::new("global-burn", "BUYSELL", NpcPurchaseSource::Trade);
    let (allocator, _) = bind(&mut fixture);
    let operation = fixture.operation(1);
    let before = fixture.live();
    let bytes = fs::read(&fixture.path).unwrap();
    let high = allocator.issued_through().unwrap();
    fixture
        .config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(matches!(
        fixture.session.try_durable_npc_purchase(operation),
        Err(NpcPurchaseDurableError::Unknown { .. })
    ));
    let burned = allocator.issued_through().unwrap();
    assert!(burned > high);
    assert_eq!(fixture.live(), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    let execution = fixture.session.try_durable_npc_purchase(operation).unwrap();
    assert_commit(
        &execution,
        operation,
        NpcPurchaseCurrency::Gold,
        NpcPurchaseSource::Trade,
    );
    let NpcPurchaseProcessingOutcome::Committed {
        incoming_unique_id, ..
    } = execution.receipt.entry.outcome
    else {
        panic!("commit");
    };
    assert!(incoming_unique_id > burned);
    fixture.assert_checkpoint();
}

#[test]
fn global_uid_merged_purchase_retains_retired_incoming_identity_in_history_floor() {
    let mut fixture = Fixture::new("global-merged", "BUYSELL", NpcPurchaseSource::Trade);
    let (allocator, _) = bind(&mut fixture);
    let first = fixture.operation(1);
    let one = fixture.session.try_durable_npc_purchase(first).unwrap();
    let NpcPurchaseProcessingOutcome::Committed {
        incoming_unique_id: first_id,
        ..
    } = one.receipt.entry.outcome
    else {
        panic!("first commit");
    };
    let second = fixture.operation(2);
    let two = fixture.session.try_durable_npc_purchase(second).unwrap();
    let NpcPurchaseProcessingOutcome::Committed {
        incoming_unique_id: second_id,
        ..
    } = two.receipt.entry.outcome
    else {
        panic!("second commit");
    };
    assert!(second_id > first_id);
    let inv = fixture.session.app.world().resource::<InventoryResource>();
    assert!(inv
        .belt_items
        .iter()
        .chain(inv.inventory_items.iter())
        .all(|i| i.unique_id != second_id));
    let floor = super::super::super::item_uid_issuance::historical_uid_floor(
        &serde_json::to_value(fixture.store_from_file()).unwrap(),
        0,
    )
    .unwrap();
    assert!(floor >= second_id);
    assert_eq!(allocator.issued_through().unwrap(), second_id);
    fixture.assert_checkpoint();
}

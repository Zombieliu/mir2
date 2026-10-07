//! Actual durable owner-boundary tests, included below the existing File fixture.
//! Every receipt comes from real Session processing or original-ID recovery.
//! Fixture paths remain owned files under the runner's guarded TEMP; no cleanup,
//! network, environment mutation or separate executable helper is introduced.

use super::*;
use crate::{NpcPurchaseOwnerAction as Action, NpcPurchaseOwnerExecution,
    NpcPurchaseOwnerReply as Reply};

const OWNER_EPOCH: [u8; 32] = [71; 32];
const NEXT_OWNER_EPOCH: [u8; 32] = [83; 32];

fn owner_state(fixture: &Fixture) -> (serde_json::Value, Option<[u8; 32]>, Option<[u8; 32]>, Option<u64>) {
    let session = fixture.session.app.world().resource::<SessionResource>();
    (fixture.live(), session.npc_purchase_producer, session.npc_purchase_owner_epoch,
        session.ranking_inspect_generation())
}

fn assert_before_execution(result: Result<NpcPurchaseOwnerExecution, NpcPurchaseDurableError>) {
    assert!(matches!(result, Err(NpcPurchaseDurableError::BeforeExecution { .. })));
}

fn owner_begin(fixture: &mut Fixture, epoch: [u8; 32]) -> NpcPurchaseProducer {
    let execution = fixture.session.execute_npc_purchase_owner(true, epoch, Action::Begin).unwrap();
    let Reply::Producer { producer } = execution.reply else { panic!("Begin must return actual producer") };
    assert!(execution.packets.is_empty());
    assert_eq!(execution.authority, Some(producer));
    assert_eq!(fixture.session.npc_purchase_owner_authority(epoch).unwrap(), producer);
    assert_eq!(producer.server_revision, fixture.save_from_file().revision);
    assert_eq!(producer.actor, fixture.save_from_file().npc_purchase_journal.unwrap().actor);
    fixture.assert_checkpoint();
    producer
}

fn owner_operation(fixture: &mut Fixture, epoch: [u8; 32], sequence: u64) -> NpcPurchaseOperation {
    let producer = owner_begin(fixture, epoch);
    let execution = fixture.session.execute_npc_purchase_owner(true, epoch,
        Action::Quote { request: fixture.request }).unwrap();
    let Reply::Quote { intent } = execution.reply else { panic!("Quote must return actual catalog intent") };
    assert!(execution.packets.is_empty());
    assert_eq!(execution.authority, Some(producer));
    NpcPurchaseOperation { actor: producer.actor, request_scope: producer.producer_scope, sequence, intent }
}

#[test]
fn owner_capability_requires_real_durable_source_and_refuses_memory_runtime() {
    let config = SimulationConfig::default();
    assert!(config.account_store_path.is_none());
    assert!(config.account_store_database_url.is_none());
    let mut memory = SimulationSession::new(config);
    assert!(!memory.supports_durable_npc_purchase_owner());
    assert_before_execution(memory.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Begin));
    assert!(memory.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
    let fixture = Fixture::new("owner-capability", "BUYSELL", NpcPurchaseSource::Trade);
    assert!(fixture.session.supports_durable_npc_purchase_owner());
    assert!(fixture.config.account_store_path.is_some());
    assert!(fixture.config.account_store_database_url.is_none());
}

#[test]
fn owner_authentication_epoch_and_enrollment_preflight_leave_actual_source_unchanged() {
    let mut fixture = Fixture::new("owner-preflight", "BUYSELL", NpcPurchaseSource::Trade);
    // Obtain a genuine local producer/intent without enrolling the owner API.
    let operation = fixture.operation(1);
    let before = owner_state(&fixture);
    let bytes = fs::read(&fixture.path).unwrap();
    assert_eq!(before.2, None);
    for action in [Action::Begin, Action::Quote { request: fixture.request },
        Action::Query { operation }, Action::Purchase { operation }] {
        assert_before_execution(fixture.session.execute_npc_purchase_owner(false, OWNER_EPOCH, action));
        assert_eq!(owner_state(&fixture), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        assert_before_execution(fixture.session.execute_npc_purchase_owner(true, [0; 32], action));
        assert_eq!(owner_state(&fixture), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    }
    for action in [Action::Quote { request: fixture.request }, Action::Query { operation }, Action::Purchase { operation }] {
        assert_before_execution(fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, action));
        assert_eq!(owner_state(&fixture), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    }
    assert!(fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
    assert!(fixture.save_from_file().npc_purchase_journal.unwrap().entries.is_empty());
}

#[test]
fn owner_begin_same_epoch_is_stable_idempotent_and_reports_verified_complete_revision() {
    let mut fixture = Fixture::new("owner-begin", "BUYSELL", NpcPurchaseSource::Trade);
    let original = fixture.save_from_file();
    assert!(original.npc_purchase_journal.is_none());
    let producer = owner_begin(&mut fixture, OWNER_EPOCH);
    assert!(producer.actor.iter().any(|byte| *byte != 0));
    assert!(producer.producer_scope.iter().any(|byte| *byte != 0));
    assert_eq!(producer.server_revision, original.revision + 1);
    assert_eq!(economic_checkpoint(&fixture.save_from_file()), economic_checkpoint(&original));
    let before = owner_state(&fixture);
    let bytes = fs::read(&fixture.path).unwrap();
    for _ in 0..3 {
        assert_eq!(owner_begin(&mut fixture, OWNER_EPOCH), producer);
        assert_eq!(owner_state(&fixture), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    }
    assert!(fixture.save_from_file().npc_purchase_journal.unwrap().entries.is_empty());
}

#[test]
fn owner_quote_and_original_id_query_never_write_file_revision_or_live_state() {
    let mut fixture = Fixture::new("owner-read-only", "BUYSELL", NpcPurchaseSource::Trade);
    let operation = owner_operation(&mut fixture, OWNER_EPOCH, 1);
    let before = owner_state(&fixture);
    let bytes = fs::read(&fixture.path).unwrap();
    let quoted = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH,
        Action::Quote { request: fixture.request }).unwrap();
    assert_eq!(quoted.reply, Reply::Quote { intent: operation.intent });
    assert!(quoted.packets.is_empty());
    assert_eq!(quoted.authority.unwrap().server_revision, fixture.save_from_file().revision);
    let missing = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Query { operation }).unwrap();
    assert_eq!(missing.reply, Reply::Recovery { receipt: None });
    assert!(missing.packets.is_empty());
    assert_eq!(owner_state(&fixture), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    // Real execution creates the only receipt used by the following lookup.
    let actual = fixture.session.try_durable_npc_purchase(operation).unwrap();
    let before = owner_state(&fixture);
    let bytes = fs::read(&fixture.path).unwrap();
    let recovered = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Query { operation }).unwrap();
    assert_eq!(recovered.reply, Reply::Recovery { receipt: Some(actual.receipt.clone()) });
    assert!(recovered.packets.is_empty());
    assert_eq!(recovered.authority.unwrap().server_revision, actual.receipt.entry.server_revision);
    assert_eq!(owner_state(&fixture), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    fixture.assert_checkpoint();
}

#[test]
fn owner_actual_purchase_and_exact_replay_preserve_receipt_and_full_live_authority() {
    for (label, currency, source) in [
        ("BUYSELL", NpcPurchaseCurrency::Gold, NpcPurchaseSource::Trade),
        ("PEARLBUY", NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Trade),
        ("BUYBACK", NpcPurchaseCurrency::Gold, NpcPurchaseSource::BuyBack),
        ("BUYUSED", NpcPurchaseCurrency::Gold, NpcPurchaseSource::Used),
        ("PEARLBUY", NpcPurchaseCurrency::Pearls, NpcPurchaseSource::Used),
    ] {
        let mut fixture = Fixture::new("owner-five-branches", label, source);
        let operation = owner_operation(&mut fixture, OWNER_EPOCH, 1);
        assert_eq!(operation.intent.currency, currency);
        assert_eq!(operation.intent.source, source);
        let before = fixture.save_from_file();
        let execution = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Purchase { operation }).unwrap();
        let Reply::Purchase { receipt, replayed } = execution.reply else { panic!("Purchase must return real receipt") };
        assert!(!replayed);
        assert_eq!(receipt.entry.operation, operation);
        let incoming = execution.packets.iter().find_map(|packet| match packet {
            ServerPacket::GainedItem { item } => Some(item), _ => None,
        }).expect("real delivery packet");
        assert_eq!(receipt.entry.outcome, NpcPurchaseProcessingOutcome::Committed {
            request: operation.intent.request, currency, source, charged: 160,
            admitted_count: 2, incoming_unique_id: incoming.unique_id,
        });
        assert_eq!(incoming.count, 2);
        let after = fixture.save_from_file();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(receipt.entry.server_revision, after.revision);
        assert_eq!(after.gold, if currency == NpcPurchaseCurrency::Gold { 99_840 } else { 100_000 });
        let systems: Stage5SystemsState = serde_json::from_str(after.stage5_systems_json.as_deref().unwrap()).unwrap();
        assert_eq!(systems.intelligent_creature_pearls, if currency == NpcPurchaseCurrency::Pearls { 840 } else { 1_000 });
        assert_eq!(after.npc_purchase_journal.unwrap().entries, vec![receipt.entry.clone()]);
        let npc = fixture.session.app.world().resource::<NpcStateResource>();
        assert!(npc.npc_buy_back_items.iter().all(|stock| stock.items.is_empty()));
        assert!(npc.npc_used_goods_items.iter().all(|stock| stock.items.is_empty()));
        let authority = execution.authority.expect("complete published checkpoint");
        assert_eq!(authority.actor, operation.actor);
        assert_eq!(authority.producer_scope, operation.request_scope);
        assert_eq!(authority.server_revision, receipt.entry.server_revision);
        assert_eq!(fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).unwrap(), authority);
        fixture.assert_checkpoint();
        let before = owner_state(&fixture);
        let bytes = fs::read(&fixture.path).unwrap();
        let replay = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Purchase { operation }).unwrap();
        assert_eq!(replay.reply, Reply::Purchase { receipt: receipt.clone(), replayed: true });
        assert!(replay.packets.is_empty());
        assert_eq!(replay.authority, Some(authority));
        assert_eq!(owner_state(&fixture), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        fixture.assert_checkpoint();
    }
}

#[test]
fn owner_epoch_change_requires_begin_and_recovers_original_identity_without_old_scope_execution() {
    let mut fixture = Fixture::new("owner-epoch", "BUYUSED", NpcPurchaseSource::Used);
    let operation = owner_operation(&mut fixture, OWNER_EPOCH, 1);
    let actual = fixture.session.try_durable_npc_purchase(operation).unwrap();
    let old_authority = fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).unwrap();
    let before = owner_state(&fixture);
    let bytes = fs::read(&fixture.path).unwrap();
    for action in [Action::Quote { request: fixture.request }, Action::Query { operation }, Action::Purchase { operation }] {
        assert_before_execution(fixture.session.execute_npc_purchase_owner(true, NEXT_OWNER_EPOCH, action));
        assert_eq!(owner_state(&fixture), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    }
    let producer = owner_begin(&mut fixture, NEXT_OWNER_EPOCH);
    assert_eq!(producer.actor, old_authority.actor);
    assert_ne!(producer.producer_scope, old_authority.producer_scope);
    assert_eq!(producer.server_revision, old_authority.server_revision);
    assert_eq!(fixture.live(), before.0);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    assert!(fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
    let before = owner_state(&fixture);
    let recovered = fixture.session.execute_npc_purchase_owner(true, NEXT_OWNER_EPOCH, Action::Query { operation }).unwrap();
    let Reply::Recovery { receipt: Some(receipt) } = recovered.reply else { panic!("original committed ID must recover") };
    assert_eq!(receipt.entry, actual.receipt.entry);
    assert_eq!(receipt.entry.operation, operation);
    assert_eq!(receipt.producer_scope, producer.producer_scope);
    assert!(recovered.packets.is_empty());
    assert_eq!(recovered.authority, Some(producer));
    assert_before_execution(fixture.session.execute_npc_purchase_owner(true, NEXT_OWNER_EPOCH, Action::Purchase { operation }));
    assert_before_execution(fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Query { operation }));
    assert_eq!(owner_state(&fixture), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn owner_partial_or_stale_live_publication_cannot_certify_recovered_receipt_revision() {
    let mut fixture = Fixture::new("owner-incomplete-witness", "BUYSELL", NpcPurchaseSource::Trade);
    let operation = owner_operation(&mut fixture, OWNER_EPOCH, 1);
    let before_commit = fixture.save_from_file();
    let actual = fixture.session.try_durable_npc_purchase(operation).unwrap();
    let complete = fixture.save_from_file();
    let bytes = fs::read(&fixture.path).unwrap();
    // Model partial publication after a genuine durable commit. Only the live
    // witness is changed; the File and the recovered receipt remain actual.
    fixture.session.app.world().resource::<SessionResource>().bind_active_save_revision(before_commit.revision);
    assert!(fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
    let before = owner_state(&fixture);
    let stale = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Query { operation }).unwrap();
    assert_eq!(stale.reply, Reply::Recovery { receipt: Some(actual.receipt.clone()) });
    assert_eq!(stale.authority, None);
    assert!(stale.packets.is_empty());
    assert_eq!(owner_state(&fixture), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    {
        let mut session = fixture.session.app.world_mut().resource_mut::<SessionResource>();
        session.bind_active_save_revision(complete.revision);
        session.npc_purchase_journal.as_mut().unwrap().entries.clear();
    }
    assert!(fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
    let before = owner_state(&fixture);
    let partial = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Query { operation }).unwrap();
    assert_eq!(partial.reply, Reply::Recovery { receipt: Some(actual.receipt.clone()) });
    assert_eq!(partial.authority, None);
    assert!(partial.packets.is_empty());
    assert_eq!(owner_state(&fixture), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    // Genuine complete checkpoint application retires the old producer. Begin
    // must establish the new producer before complete authority can return.
    fixture.session.restore_active_character_checkpoint(&complete).unwrap();
    assert!(fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
    assert_before_execution(fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Query { operation }));
    let producer = owner_begin(&mut fixture, OWNER_EPOCH);
    assert_eq!(producer.actor, operation.actor);
    assert_ne!(producer.producer_scope, operation.request_scope);
    assert_eq!(producer.server_revision, actual.receipt.entry.server_revision);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn owner_failed_begin_and_failed_logout_or_disconnect_retire_authority_without_losing_recovery() {
    let mut uninitialized = Fixture::new("owner-failed-begin", "BUYSELL", NpcPurchaseSource::Trade);
    let before = owner_state(&uninitialized);
    let bytes = fs::read(&uninitialized.path).unwrap();
    uninitialized.config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(matches!(uninitialized.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Begin),
        Err(NpcPurchaseDurableError::Unknown { .. })));
    assert_eq!(owner_state(&uninitialized), before);
    assert_eq!(fs::read(&uninitialized.path).unwrap(), bytes);
    assert!(uninitialized.session.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
    for packet in [ClientPacket::LogOut, ClientPacket::Disconnect] {
        let mut fixture = Fixture::new("owner-failed-retirement", "BUYSELL", NpcPurchaseSource::Trade);
        let operation = owner_operation(&mut fixture, OWNER_EPOCH, 1);
        let actual = fixture.session.try_durable_npc_purchase(operation).unwrap();
        let before = fixture.live();
        let bytes = fs::read(&fixture.path).unwrap();
        fixture.config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        fixture.session.handle_packet(packet);
        assert_eq!(fixture.live(), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        assert!(fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
        for action in [Action::Begin, Action::Quote { request: fixture.request }, Action::Query { operation }, Action::Purchase { operation }] {
            assert_before_execution(fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, action));
        }
        assert_eq!(fixture.live(), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        let mut reconnected = login_start(fixture.config.clone(), 0);
        assert!(reconnected.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
        let execution = reconnected.execute_npc_purchase_owner(true, NEXT_OWNER_EPOCH, Action::Begin).unwrap();
        let Reply::Producer { producer } = execution.reply else { panic!("new real owner must Begin") };
        assert_eq!(producer.actor, operation.actor);
        assert_ne!(producer.producer_scope, operation.request_scope);
        assert_eq!(execution.authority, Some(producer));
        let reconnected_bytes = fs::read(&fixture.path).unwrap();
        let recovered = reconnected.execute_npc_purchase_owner(true, NEXT_OWNER_EPOCH, Action::Query { operation }).unwrap();
        let Reply::Recovery { receipt: Some(receipt) } = recovered.reply else { panic!("new owner must recover exact original operation") };
        assert_eq!(receipt.entry, actual.receipt.entry);
        assert_eq!(receipt.producer_scope, producer.producer_scope);
        assert_eq!(recovered.authority, Some(producer));
        assert!(recovered.packets.is_empty());
        assert_eq!(fs::read(&fixture.path).unwrap(), reconnected_bytes);
    }
}

#[test]
fn owner_current_revision_and_journal_cannot_certify_unpublished_wallet_or_delivery() {
    for missing_delivery in [false, true] {
        let mut fixture = Fixture::new("owner-partial-economics", "BUYUSED", NpcPurchaseSource::Used);
        let operation = owner_operation(&mut fixture, OWNER_EPOCH, 1);
        let baseline = fixture.save_from_file();
        let actual = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH,
            Action::Purchase { operation }).unwrap();
        let incoming = actual.packets.iter().find_map(|packet| match packet {
            ServerPacket::GainedItem { item } => Some(item.clone()), _ => None,
        }).expect("genuine owner purchase delivery");
        let Reply::Purchase { receipt, replayed } = actual.reply else { panic!("actual purchase must retain its terminal receipt") };
        assert!(!replayed);
        assert_eq!(receipt.entry.operation, operation);
        assert!(matches!(&receipt.entry.outcome, NpcPurchaseProcessingOutcome::Committed { .. }));
        let complete = fixture.save_from_file();
        assert_eq!(receipt.entry.server_revision, complete.revision);
        assert_eq!(actual.authority.unwrap().server_revision, complete.revision);
        assert_eq!(complete.npc_purchase_journal.as_ref().unwrap().lookup(&operation).unwrap(), Some(&receipt.entry));
        fixture.assert_checkpoint();
        let bytes = fs::read(&fixture.path).unwrap();
        if missing_delivery {
            // The baseline had no items, so this is the exact actual delivery,
            // with its real carrier identity rather than an invented UID.
            let mut inventory = fixture.session.app.world_mut().resource_mut::<InventoryResource>();
            assert!(inventory.inventory_items.is_empty());
            assert_eq!(inventory.belt_items.len(), 1);
            let removed = inventory.belt_items.pop().unwrap();
            assert_eq!(removed.container, ItemContainer::Belt);
            assert_eq!(removed.slot, 0);
            let raw = crate::runtime::items::user_item_from_item_state(&removed);
            assert_eq!(raw.unique_id, incoming.unique_id);
            assert_eq!(raw.item_index, incoming.item_index);
            assert_eq!(raw.count, incoming.count);
        } else {
            // Model the wallet half of publication still holding its genuine
            // pre-commit value while revision/history are already current.
            let mut runtime = fixture.session.app.world_mut().resource_mut::<PlayerRuntimeResource>();
            assert_ne!(runtime.gold, baseline.gold);
            runtime.gold = baseline.gold;
        }
        let session = fixture.session.app.world().resource::<SessionResource>();
        assert_eq!(session.active_save_revision(), Some(complete.revision));
        assert_eq!(session.npc_purchase_journal, complete.npc_purchase_journal);
        assert_eq!(session.npc_purchase_owner_epoch, Some(OWNER_EPOCH));
        assert_eq!(session.npc_purchase_producer, Some(operation.request_scope));
        assert!(fixture.session.npc_purchase_owner_authority(OWNER_EPOCH).is_err(),
            "complete authority requires actual wallet and inventory publication");
        let before = owner_state(&fixture);
        let query = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH, Action::Query { operation }).unwrap();
        assert_eq!(query.reply, Reply::Recovery { receipt: Some(receipt.clone()) });
        assert_eq!(query.authority, None);
        assert!(query.packets.is_empty());
        assert_eq!(owner_state(&fixture), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        assert_eq!(fixture.save_from_file().npc_purchase_journal.as_ref().unwrap().lookup(&operation).unwrap(), Some(&receipt.entry));
    }
}

#[test]
fn owner_actual_checkpoint_replica_apply_mirrors_complete_state_without_reexecution_or_source_writes() {
    for (service, source, currency) in [
        ("BUYBACK", NpcPurchaseSource::BuyBack, NpcPurchaseCurrency::Gold),
        ("PEARLBUY", NpcPurchaseSource::Used, NpcPurchaseCurrency::Pearls),
    ] {
        let mut fixture = Fixture::new("owner-replica-apply", service, source);
        let operation = owner_operation(&mut fixture, OWNER_EPOCH, 1);
        let before = fixture.session.npc_purchase_owner_checkpoint(OWNER_EPOCH).unwrap();
        assert_eq!(serde_json::to_value(&before).unwrap(), serde_json::to_value(fixture.save_from_file()).unwrap());
        // Fork before the genuine purchase: this standby cannot observe the
        // source commit through a shared AccountStore or an external repository.
        let replica_config = fixture.config.fork_for_replica_apply().unwrap();
        assert!(replica_config.account_store_path.is_none());
        assert!(replica_config.account_store_database_url.is_none());
        assert!(!std::sync::Arc::ptr_eq(&replica_config.account_store, &fixture.config.account_store));
        let mut replica = SimulationSession::new(replica_config.clone());
        assert!(replica.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() })
            .iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        replica.handle_packet(ClientPacket::StartGame { character_index: before.character.index });
        assert_eq!(replica.active_identity().unwrap().character_index, before.character.index);
        let standby_before = snapshot_active_character_save(replica.app.world()).unwrap();
        assert_eq!(economic_checkpoint(&standby_before), economic_checkpoint(&before));
        assert_eq!(standby_before.npc_purchase_journal, before.npc_purchase_journal);
        let actual = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH,
            Action::Purchase { operation }).unwrap();
        let Reply::Purchase { receipt, replayed } = actual.reply else { panic!("real source purchase must commit once") };
        assert!(!replayed);
        assert!(matches!(&receipt.entry.outcome, NpcPurchaseProcessingOutcome::Committed { .. }));
        let checkpoint = fixture.session.npc_purchase_owner_checkpoint(OWNER_EPOCH).unwrap();
        assert_eq!(checkpoint.revision, before.revision + 1);
        assert_eq!(receipt.entry.server_revision, checkpoint.revision);
        assert_eq!(checkpoint.npc_purchase_journal.as_ref().unwrap().entries, vec![receipt.entry.clone()]);
        assert_eq!(serde_json::to_value(&checkpoint).unwrap(), serde_json::to_value(fixture.save_from_file()).unwrap());
        let isolated_before = replica_config.account_store.lock().unwrap().accounts["demo"].saves[&before.character.index].clone();
        assert_eq!(isolated_before.npc_purchase_journal.as_ref().unwrap().entries.len(), 0);
        assert_eq!(isolated_before.revision, standby_before.revision);
        let source_before_replay = owner_state(&fixture);
        let bytes = fs::read(&fixture.path).unwrap();
        replica.apply_npc_purchase_owner_replica_checkpoint(&checkpoint).unwrap();
        let applied = snapshot_active_character_save(replica.app.world()).unwrap();
        assert_eq!(economic_checkpoint(&applied), economic_checkpoint(&checkpoint));
        assert_eq!(applied.revision, checkpoint.revision);
        assert_eq!(applied.npc_purchase_journal, checkpoint.npc_purchase_journal);
        assert_eq!(applied.gold, if currency == NpcPurchaseCurrency::Gold { 99_840 } else { 100_000 });
        assert_eq!(replica.app.world().resource::<Stage5SystemsResource>().stage5_systems.intelligent_creature_pearls,
            if currency == NpcPurchaseCurrency::Pearls { 840 } else { 1_000 });
        let inventory = replica.app.world().resource::<InventoryResource>();
        assert!(inventory.inventory_items.is_empty());
        assert_eq!(inventory.belt_items.len(), 1);
        assert_eq!(inventory.belt_items[0].quantity, 2);
        assert_eq!(inventory.belt_items[0].container, ItemContainer::Belt);
        assert_eq!(inventory.belt_items[0].slot, 0);
        let npc = replica.app.world().resource::<NpcStateResource>();
        assert!(npc.npc_buy_back_items.iter().all(|stock| stock.items.is_empty()));
        assert!(npc.npc_used_goods_items.iter().all(|stock| stock.items.is_empty()));
        let isolated = replica_config.account_store.lock().unwrap().accounts["demo"].saves[&checkpoint.character.index].clone();
        assert_eq!(serde_json::to_value(&isolated).unwrap(), serde_json::to_value(&checkpoint).unwrap());
        assert_eq!(serde_json::to_value(&replica_config.account_store.lock().unwrap().accounts["demo"].characters.iter()
            .find(|character| character.index == checkpoint.character.index).unwrap()).unwrap(),
            serde_json::to_value(&checkpoint.character).unwrap());
        assert!(!replica.supports_durable_npc_purchase_owner());
        assert!(replica.npc_purchase_owner_authority(OWNER_EPOCH).is_err());
        let session = replica.app.world().resource::<SessionResource>();
        assert_eq!(session.npc_purchase_producer, None);
        assert_eq!(session.npc_purchase_owner_epoch, None);
        assert_eq!(owner_state(&fixture), source_before_replay);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        // Apply the same checkpoint again; no second execution or delivery is
        // requested, and both isolated history and real source remain exact.
        let memory_before = serde_json::to_value(&*replica_config.account_store.lock().unwrap()).unwrap();
        replica.apply_npc_purchase_owner_replica_checkpoint(&checkpoint).unwrap();
        let repeated = snapshot_active_character_save(replica.app.world()).unwrap();
        assert_eq!(economic_checkpoint(&repeated), economic_checkpoint(&applied));
        assert_eq!(repeated.revision, applied.revision);
        assert_eq!(repeated.npc_purchase_journal, applied.npc_purchase_journal);
        assert_eq!(serde_json::to_value(&*replica_config.account_store.lock().unwrap()).unwrap(), memory_before);
        assert_eq!(owner_state(&fixture), source_before_replay);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        assert_eq!(replica.app.world().resource::<SessionResource>().npc_purchase_producer, None);
        assert_eq!(replica.app.world().resource::<SessionResource>().npc_purchase_owner_epoch, None);
    }
}

#[test]
fn owner_replica_refuses_source_truth_apply_and_older_foreign_or_rewritten_actual_history() {
    let mut fixture = Fixture::new("owner-replica-rejection", "BUYUSED", NpcPurchaseSource::Used);
    let operation = owner_operation(&mut fixture, OWNER_EPOCH, 1);
    let older = fixture.session.npc_purchase_owner_checkpoint(OWNER_EPOCH).unwrap();
    let replica_config = fixture.config.fork_for_replica_apply().unwrap();
    assert!(replica_config.account_store_path.is_none());
    assert!(replica_config.account_store_database_url.is_none());
    let mut replica = SimulationSession::new(replica_config.clone());
    assert!(replica.handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() })
        .iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    replica.handle_packet(ClientPacket::StartGame { character_index: older.character.index });
    assert_eq!(replica.active_identity().unwrap().character_index, older.character.index);
    let actual = fixture.session.execute_npc_purchase_owner(true, OWNER_EPOCH,
        Action::Purchase { operation }).unwrap();
    let Reply::Purchase { receipt, replayed } = actual.reply else { panic!("source operation must actually commit") };
    assert!(!replayed);
    assert!(matches!(&receipt.entry.outcome, NpcPurchaseProcessingOutcome::Committed { .. }));
    let checkpoint = fixture.session.npc_purchase_owner_checkpoint(OWNER_EPOCH).unwrap();
    assert_eq!(checkpoint.npc_purchase_journal.as_ref().unwrap().lookup(&operation).unwrap(), Some(&receipt.entry));
    let source_before = owner_state(&fixture);
    let bytes = fs::read(&fixture.path).unwrap();
    let refusal = fixture.session.apply_npc_purchase_owner_replica_checkpoint(&checkpoint).unwrap_err();
    assert!(refusal.contains("isolated replica"));
    assert_eq!(owner_state(&fixture), source_before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    replica.apply_npc_purchase_owner_replica_checkpoint(&checkpoint).unwrap();
    let applied = snapshot_active_character_save(replica.app.world()).unwrap();
    let memory_before = serde_json::to_value(&*replica_config.account_store.lock().unwrap()).unwrap();
    // Use another genuine server-issued Actor to corrupt only the binding in a
    // copy of the actual checkpoint. No terminal execution/receipt is invented.
    let mut other_actor = Fixture::new("owner-replica-foreign-actor", "BUYSELL", NpcPurchaseSource::Trade);
    let foreign_actor = owner_begin(&mut other_actor, OWNER_EPOCH).actor;
    assert_ne!(foreign_actor, operation.actor);
    let mut foreign = checkpoint.clone();
    let foreign_journal = foreign.npc_purchase_journal.as_mut().unwrap();
    foreign_journal.actor = foreign_actor;
    for entry in &mut foreign_journal.entries { entry.operation.actor = foreign_actor; }
    // This history remains structurally valid; exact predecessor preservation
    // must reject the changed catalog proof under the same original key.
    let mut rewritten = checkpoint.clone();
    rewritten.npc_purchase_journal.as_mut().unwrap().entries[0]
        .operation.intent.service_catalog_proof[0] ^= 1;
    let mut conflicting = checkpoint.clone();
    conflicting.gold = conflicting.gold.checked_add(1).unwrap();
    assert_eq!(conflicting.revision, checkpoint.revision);
    assert_eq!(conflicting.npc_purchase_journal, checkpoint.npc_purchase_journal);
    for (candidate, reason) in [(older, "roll back"), (foreign, "replace an actor"), (rewritten, "terminal history"),
        (conflicting, "conflicting complete contents")] {
        candidate.npc_purchase_journal.as_ref().unwrap().validate_for("demo",
            candidate.character.index, &candidate.character.name, candidate.revision).unwrap();
        let refusal = replica.apply_npc_purchase_owner_replica_checkpoint(&candidate).unwrap_err();
        assert!(refusal.contains(reason), "expected {reason}: {refusal}");
        let retained = snapshot_active_character_save(replica.app.world()).unwrap();
        assert_eq!(economic_checkpoint(&retained), economic_checkpoint(&applied));
        assert_eq!(retained.revision, applied.revision);
        assert_eq!(retained.npc_purchase_journal, applied.npc_purchase_journal);
        assert_eq!(serde_json::to_value(&*replica_config.account_store.lock().unwrap()).unwrap(), memory_before);
        assert_eq!(owner_state(&fixture), source_before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        assert!(!replica.supports_durable_npc_purchase_owner());
        assert_eq!(replica.app.world().resource::<SessionResource>().npc_purchase_producer, None);
        assert_eq!(replica.app.world().resource::<SessionResource>().npc_purchase_owner_epoch, None);
    }
}

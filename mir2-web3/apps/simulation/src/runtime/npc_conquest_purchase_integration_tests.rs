//! Cross-branch purchase paths use the actual castle account transaction.
use super::*;
use super::tests::{durable, fixture, inventory_image, stock_image, treasury};
use crate::npc_purchase_journal::{NpcPurchaseIntent, NpcPurchaseOperation};
use super::super::npc_purchase_outcome::{NpcPurchaseCurrency, NpcPurchaseSource};
use super::super::npc_gold_buy_outcome::{NpcGoldBuyRequest, NpcGoldBuyProcessingOutcome};

#[test]
fn typed_castle_gold_purchase_commits_tax_and_authoritative_charge_together() {
    let (mut session, config, item, template) = fixture(false);
    let info = crystal_npc_info_by_script_key("MongchonProvince/SabukWall/Potion").unwrap();
    let base = npc::crystal_npc_purchase_cost(&template, 2, source_price_rate(info.rate, 0, true));
    let charged = npc::crystal_npc_purchase_cost(&template, 2, source_price_rate(info.rate, 25, false));
    let request = NpcGoldBuyRequest { item_index: item, count: 2, panel_type: 0 };
    let execution = session.try_npc_gold_buy_with_outcome(request).unwrap();
    assert!(matches!(execution.outcome, NpcGoldBuyProcessingOutcome::Committed {
        request: actual, gold_spent, ..
    } if actual == request && gold_spent == charged));
    assert!(execution.packets.iter().any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
    assert_eq!(durable(&config).gold, 50_000 - charged);
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, 50_000 - charged);
    assert_eq!(treasury(&config), charged - base);
}

#[test]
fn typed_castle_failed_publication_never_reports_the_preview_as_committed() {
    let (mut session, config, item, _) = fixture(false);
    let before_inventory = inventory_image(&session);
    let before_stock = stock_image(&session);
    let before_save = serde_json::to_value(durable(&config)).unwrap();
    let before_tax = treasury(&config);
    config.inject_account_store_transaction_fault(crate::config::AccountStoreTransactionFault::BeforePersist);
    let result = session.try_npc_purchase_with_outcome(NpcPurchaseRequest {
        item_index: item, count: 2, panel_type: 0,
    });
    assert!(matches!(result, Err(super::super::npc_purchase_outcome::NpcPurchaseProcessingError::Unknown { .. })));
    assert_eq!(inventory_image(&session), before_inventory);
    assert_eq!(stock_image(&session), before_stock);
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, 50_000);
    assert_eq!(serde_json::to_value(durable(&config)).unwrap(), before_save);
    assert_eq!(treasury(&config), before_tax);
}

#[test]
fn durable_castle_overlap_refuses_without_debit_stock_or_journal_and_cannot_bypass_tax() {
    let (mut session, mut config, item, template) = fixture(false);
    let temporary = std::env::temp_dir().canonicalize().unwrap();
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let directory = temporary.join(format!("mir2-castle-receipt-integration-{}-{unique}", std::process::id()));
    assert!(directory.starts_with(&temporary));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("accounts.json");
    // Seed only this freshly created fixture file, then bind the real shared
    // File authority through its official loader. Assigning a path to a memory
    // config does not establish publication authority.
    session.save_active_character().unwrap();
    let seed = serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap();
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
        file.write_all(&seed).unwrap();
    }
    config = config.with_account_store_path(&path);
    session.app.world_mut().resource_mut::<RuntimeConfigResource>().config = config.clone();
    assert!(std::sync::Arc::ptr_eq(
        &session.app.world().resource::<RuntimeConfigResource>().config.account_store,
        &config.account_store,
    ));
    session.save_active_character().unwrap();
    let producer = session.begin_npc_purchase_producer().unwrap();
    let request = NpcPurchaseRequest { item_index: item, count: 2, panel_type: 0 };
    let before_file = std::fs::read(&path).unwrap();
    let before_inventory = inventory_image(&session);
    let before_stock = stock_image(&session);
    let before_tax = treasury(&config);
    let before_gold = session.app.world().resource::<PlayerRuntimeResource>().gold;
    assert!(matches!(session.npc_purchase_intent(request),
        Err(super::super::NpcPurchaseDurableError::BeforeExecution { detail }) if detail.contains("castle-tax")));
    // Bypass the trusted intent constructor to prove execution has its own guard.
    let operation = NpcPurchaseOperation {
        actor: producer.actor, request_scope: producer.producer_scope, sequence: 1,
        intent: NpcPurchaseIntent { request, currency: NpcPurchaseCurrency::Gold,
            source: NpcPurchaseSource::Trade, service_catalog_proof: [1; 32] },
    };
    assert!(matches!(session.try_durable_npc_purchase(operation),
        Err(super::super::NpcPurchaseDurableError::BeforeExecution { detail }) if detail.contains("castle-tax")));
    assert_eq!(session.query_npc_purchase(operation).unwrap(), None);
    assert_eq!(std::fs::read(&path).unwrap(), before_file);
    assert_eq!(inventory_image(&session), before_inventory);
    assert_eq!(stock_image(&session), before_stock);
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, before_gold);
    assert_eq!(treasury(&config), before_tax);

    // The established ordinary route stays available and charges the tax once.
    let packets = session.try_handle_packet(request.packet()).unwrap();
    assert!(packets.iter().any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
    let info = crystal_npc_info_by_script_key("MongchonProvince/SabukWall/Potion").unwrap();
    let base = npc::crystal_npc_purchase_cost(&template, 2, source_price_rate(info.rate, 0, true));
    let charged = npc::crystal_npc_purchase_cost(&template, 2, source_price_rate(info.rate, 25, false));
    assert_eq!(session.app.world().resource::<PlayerRuntimeResource>().gold, before_gold - charged);
    assert_eq!(treasury(&config) - before_tax, charged - base);
    assert!(durable(&config).npc_purchase_journal.as_ref().is_none_or(|journal| journal.entries.is_empty()));
}

use mir2_game_data::{BILLING_MONTHLY_CARD_GAME_SHOP_INDEX, BILLING_MONTHLY_CARD_ITEM_INDEX};
use mir2_protocol::{ClientPacket, MirGridType, ServerPacket};
use mir2_simulation::billing::{RechargeOffer, RechargeOrderState, VerifiedRechargePayment};
use mir2_simulation::monthly_card::{
    monthly_card_now_ms, MonthlyCardPolicy, MONTHLY_CARD_DURATION_MS,
};
use mir2_simulation::{
    AccountStoreRepository, FileAccountStoreRepository, InProcessWorldRuntime,
    NativeGameShopPurchaseRequest, SimulationConfig, SimulationSession, WorldCommand, WorldRuntime,
    NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
};

const KEY: &str = "monthly-card-test-issuance-key-only-not-a-deployment-key";
fn config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    config.billing_monthly_card_credit_price = Some(10);
    // Prepared wallet balance. These tests exercise purchases, not Stripe acceptance.
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&config.default_character.index)
        .unwrap()
        .credit = 100;
    config
}
fn saved_credit(config: &SimulationConfig) -> u32 {
    config.account_store.lock().unwrap().accounts["demo"].saves[&config.default_character.index]
        .credit
}
fn serialized(config: &SimulationConfig) -> serde_json::Value {
    serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
}

fn fund_file_wallet(config: &SimulationConfig, path: &std::path::Path, now: u64, suffix: &str) {
    // Bind the file authority before funding. Its store is loaded independently
    // of any balance prepared in the original in-memory SimulationConfig.
    // Payment confirmation is a trusted fixture, not a Stripe transport check.
    let index = config.default_character.index;
    let order = config
        .prepare_recharge_order(
            "demo",
            index,
            &format!("monthly-card-{suffix}-funding"),
            &RechargeOffer {
                id: "monthly-card-test-credits".into(),
                label: "Prepared Credits".into(),
                currency: "usd".into(),
                amount_minor: 100,
                credits: 100,
            },
            now,
            false,
        )
        .unwrap();
    let paid = config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &VerifiedRechargePayment {
                session_id: format!("cs_monthly_{suffix}"),
                payment_intent_id: format!("pi_monthly_{suffix}"),
                amount_minor: 100,
                currency: "usd".into(),
                livemode: false,
            },
            now,
        )
        .unwrap();
    assert_eq!(paid.state, RechargeOrderState::PaidPending);
    let applied = config
        .consume_inactive_pending_recharge_credits("demo", index, now)
        .unwrap();
    assert_eq!((applied.granted, applied.credit), (100, 100));
    let durable = FileAccountStoreRepository::new(path)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(durable.accounts["demo"].saves[&index].credit, 100);
    assert_eq!(
        durable.accounts["demo"].billing.as_ref().unwrap().orders[&order.id].state,
        RechargeOrderState::Applied
    );
}

#[test]
fn billing_card_before_start_game_is_atomic_and_replays_without_new_asset() {
    let config =
        config().with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
    let index = config.default_character.index;
    let now = monthly_card_now_ms();
    let mut session = SimulationSession::new(config.clone());
    session.passkey_login("demo");
    assert!(!session
        .handle_packet(ClientPacket::StartGame {
            character_index: index
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let card = config
        .buy_monthly_card_for_character("demo", index, "select-purchase-0001", now)
        .unwrap();
    assert!(card.unique_id > u64::from(u32::MAX) && card.unique_id < (1u64 << 53));
    assert_eq!(saved_credit(&config), 90);
    let before = serialized(&config);
    let replay = config
        .buy_monthly_card_for_character("demo", index, "select-purchase-0001", now + 1)
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.unique_id, card.unique_id);
    assert_eq!(serialized(&config), before);
    assert_eq!(
        config
            .list_billing_monthly_cards("demo", index)
            .unwrap()
            .len(),
        1
    );
    let activated = config
        .activate_monthly_card_for_character("demo", index, card.unique_id, now)
        .unwrap();
    assert_eq!(
        activated.status.expires_at_ms,
        Some(now + MONTHLY_CARD_DURATION_MS)
    );
    let before = serialized(&config);
    assert!(
        config
            .activate_monthly_card_for_character("demo", index, card.unique_id, now + 2)
            .unwrap()
            .replayed
    );
    assert_eq!(serialized(&config), before);
    assert!(config
        .list_billing_monthly_cards("demo", index)
        .unwrap()
        .is_empty());
    assert!(session
        .handle_packet(ClientPacket::StartGame {
            character_index: index
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
}

#[test]
fn billing_card_stacks_with_codes_and_old_uid_does_not_consume_new_card() {
    let config =
        config().with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
    let index = config.default_character.index;
    let now = monthly_card_now_ms();
    let issued = config
        .issue_monthly_card("demo", "operator-code-0001", now)
        .unwrap();
    config
        .redeem_monthly_card("demo", &issued.code, now)
        .unwrap();
    let first = config
        .buy_monthly_card_for_character("demo", index, "card-purchase-0001", now)
        .unwrap();
    config
        .activate_monthly_card_for_character("demo", index, first.unique_id, now)
        .unwrap();
    let second = config
        .buy_monthly_card_for_character("demo", index, "card-purchase-0002", now)
        .unwrap();
    assert_ne!(first.unique_id, second.unique_id);
    assert!(
        config
            .activate_monthly_card_for_character("demo", index, first.unique_id, now)
            .unwrap()
            .replayed
    );
    assert_eq!(
        config.list_billing_monthly_cards("demo", index).unwrap()[0].unique_id,
        second.unique_id
    );
    let receipt = config
        .activate_monthly_card_for_character("demo", index, second.unique_id, now)
        .unwrap();
    assert_eq!(
        receipt.status.expires_at_ms,
        Some(now + 3 * MONTHLY_CARD_DURATION_MS)
    );
    config.account_store.lock().unwrap().accounts["demo"]
        .monthly_card
        .as_ref()
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn billing_card_file_restart_retains_unit_and_consumption_receipts() {
    let dir = std::env::temp_dir().join(format!(
        "mir2-billing-card-{}-{}",
        std::process::id(),
        monthly_card_now_ms()
    ));
    let path = dir.join("accounts.json");
    let mut config = SimulationConfig::default().with_account_store_path(&path);
    config.billing_monthly_card_credit_price = Some(10);
    let index = config.default_character.index;
    let now = monthly_card_now_ms();
    fund_file_wallet(&config, &path, now, "restart");
    let card = config
        .buy_monthly_card_for_character("demo", index, "restart-card-0001", now)
        .unwrap();
    let loaded = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(
        loaded.accounts["demo"]
            .monthly_card
            .as_ref()
            .unwrap()
            .item_receipts
            .len(),
        1
    );
    drop(config);
    let mut reopened = SimulationConfig::default().with_account_store_path(&path);
    reopened.billing_monthly_card_credit_price = Some(10);
    assert!(
        reopened
            .buy_monthly_card_for_character("demo", index, "restart-card-0001", now)
            .unwrap()
            .replayed
    );
    reopened
        .activate_monthly_card_for_character("demo", index, card.unique_id, now)
        .unwrap();
    drop(reopened);
    let reopened = SimulationConfig::default().with_account_store_path(&path);
    assert!(
        reopened
            .activate_monthly_card_for_character("demo", index, card.unique_id, now + 1)
            .unwrap()
            .replayed
    );
    assert_eq!(saved_credit(&reopened), 90);
    assert_eq!(
        reopened
            .monthly_card_status("demo", now)
            .unwrap()
            .expires_at_ms,
        Some(now + MONTHLY_CARD_DURATION_MS)
    );
}

#[cfg(feature = "test-support")]
#[test]
fn billing_card_failed_publication_rolls_back_inventory_credit_and_access() {
    use mir2_simulation::AccountStoreTransactionFault;
    let config = config();
    let index = config.default_character.index;
    let now = monthly_card_now_ms();
    let before = serialized(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(config
        .buy_monthly_card_for_character("demo", index, "failure-card-0001", now)
        .is_err());
    assert_eq!(serialized(&config), before);
    let card = config
        .buy_monthly_card_for_character("demo", index, "failure-card-0001", now)
        .unwrap();
    let before = serialized(&config);
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    assert!(config
        .activate_monthly_card_for_character("demo", index, card.unique_id, now)
        .is_err());
    assert_eq!(serialized(&config), before);
    assert_eq!(
        config
            .list_billing_monthly_cards("demo", index)
            .unwrap()
            .len(),
        1
    );
    assert!(!config.monthly_card_status("demo", now).unwrap().active);
    assert!(
        !config
            .activate_monthly_card_for_character("demo", index, card.unique_id, now)
            .unwrap()
            .replayed
    );
}

#[test]
fn billing_card_real_native_shop_mail_and_use_keep_permanent_identity() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let config = config();
    let index = config.default_character.index;
    let mut runtime = InProcessWorldRuntime::new(config.clone());
    runtime
        .execute(WorldCommand::PasskeyLogin {
            account_id: "demo".into(),
        })
        .unwrap();
    let boot = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    let product = boot
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::GameShopInfo { item, .. }
                if item.g_index == BILLING_MONTHLY_CARD_GAME_SHOP_INDEX =>
            {
                Some(item)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(product.item_index, BILLING_MONTHLY_CARD_ITEM_INDEX);
    assert!(product.can_buy_credit && !product.can_buy_gold);
    assert_eq!(product.info.price, 0);
    assert_eq!(product.info.stack_size, 1);
    let request = NativeGameShopPurchaseRequest {
        protocol_version: NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
        server_idempotency_key: URL_SAFE_NO_PAD.encode([23u8; 32]),
        gateway_session_id: "billing-test-session".into(),
        account_id: "demo".into(),
        character_index: index,
        client_request_id: "purchase-1".into(),
        g_index: BILLING_MONTHLY_CARD_GAME_SHOP_INDEX,
        quantity: 1,
        price_type: 0,
    };
    let packets = runtime
        .execute(WorldCommand::NativeGameShopPurchase(request.clone()))
        .unwrap();
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseCredit { credit: 10 })));
    let before = serialized(&config);
    runtime
        .execute(WorldCommand::NativeGameShopPurchase(request))
        .unwrap();
    assert_eq!(serialized(&config), before);
    let (uid, mail_id) = {
        let store = config.account_store.lock().unwrap();
        let account = &store.accounts["demo"];
        let uid = account
            .monthly_card
            .as_ref()
            .unwrap()
            .item_receipts
            .values()
            .next()
            .unwrap()
            .unique_id;
        let systems: mir2_simulation::Stage5SystemsState = serde_json::from_str(
            account.saves[&index]
                .stage5_systems_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        (
            uid,
            systems
                .mail
                .iter()
                .find(|mail| !mail.deleted && !mail.claimed)
                .unwrap()
                .id,
        )
    };
    let gained = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
            mail_id: u64::from(mail_id),
        }))
        .unwrap();
    assert!(gained.iter().any(
        |packet| matches!(packet, ServerPacket::GainedItem { item } if item.unique_id == uid)
    ));
    let before = serialized(&config);
    assert!(runtime.execute(WorldCommand::ClientPacket(ClientPacket::UseItem { unique_id: uid,
        grid: MirGridType::Belt })).unwrap().iter()
        .any(|packet| matches!(packet, ServerPacket::UseItem { success: false, unique_id, .. } if *unique_id == uid)));
    assert_eq!(serialized(&config), before);
    let use_packet = ClientPacket::UseItem {
        unique_id: uid,
        grid: MirGridType::Inventory,
    };
    assert!(runtime.execute(WorldCommand::ClientPacket(use_packet.clone())).unwrap().iter()
        .any(|packet| matches!(packet, ServerPacket::UseItem { success: true, unique_id, .. } if *unique_id == uid)));
    let expiry = config
        .monthly_card_status("demo", monthly_card_now_ms())
        .unwrap()
        .expires_at_ms;
    runtime
        .execute(WorldCommand::ClientPacket(use_packet))
        .unwrap();
    assert_eq!(
        config
            .monthly_card_status("demo", monthly_card_now_ms())
            .unwrap()
            .expires_at_ms,
        expiry
    );
    assert!(config
        .list_billing_monthly_cards("demo", index)
        .unwrap()
        .is_empty());
}

#[test]
fn billing_card_offline_select_can_activate_unclaimed_game_shop_mail() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let config = config();
    let index = config.default_character.index;
    let mut runtime = InProcessWorldRuntime::new(config.clone());
    runtime
        .execute(WorldCommand::PasskeyLogin {
            account_id: "demo".into(),
        })
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::NativeGameShopPurchase(
            NativeGameShopPurchaseRequest {
                protocol_version: NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
                server_idempotency_key: URL_SAFE_NO_PAD.encode([24u8; 32]),
                gateway_session_id: "billing-mail-test".into(),
                account_id: "demo".into(),
                character_index: index,
                client_request_id: "mail-purchase-1".into(),
                g_index: BILLING_MONTHLY_CARD_GAME_SHOP_INDEX,
                quantity: 2,
                price_type: 0,
            },
        ))
        .unwrap();
    drop(runtime);
    let config = config.with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
    let now = monthly_card_now_ms();
    assert!(
        !config
            .monthly_card_status("demo", now)
            .unwrap()
            .can_enter_game
    );
    let cards = config.list_billing_monthly_cards("demo", index).unwrap();
    assert_eq!(cards.len(), 2);
    let first = config
        .activate_monthly_card_for_character("demo", index, cards[0].unique_id, now)
        .unwrap();
    assert!(first.status.can_enter_game);
    assert_eq!(saved_credit(&config), 80);
    let remaining = config.list_billing_monthly_cards("demo", index).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_ne!(remaining[0].unique_id, first.unique_id);
    let before = serialized(&config);
    assert!(
        config
            .activate_monthly_card_for_character("demo", index, first.unique_id, now)
            .unwrap()
            .replayed
    );
    assert_eq!(serialized(&config), before);
    let store = config.account_store.lock().unwrap();
    let systems: mir2_simulation::Stage5SystemsState = serde_json::from_str(
        store.accounts["demo"].saves[&index]
            .stage5_systems_json
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    let mail = systems
        .mail
        .iter()
        .find(|mail| !mail.deleted && !mail.claimed)
        .unwrap();
    assert_eq!(mail.items.len(), 1);
    assert_eq!(mail.item_states_json.len(), 1);
}

#[test]
fn billing_card_key_alias_cannot_consume_a_card() {
    let config = config();
    let index = config.default_character.index;
    let now = monthly_card_now_ms();
    let card = config
        .buy_monthly_card_for_character("demo", index, "alias-card-000001", now)
        .unwrap();
    let mut runtime = InProcessWorldRuntime::new(config.clone());
    runtime
        .execute(WorldCommand::PasskeyLogin {
            account_id: "demo".into(),
        })
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    runtime
        .execute(WorldCommand::UseItem {
            key: "crystal-item-1000001".into(),
        })
        .unwrap();
    assert_eq!(
        config.list_billing_monthly_cards("demo", index).unwrap()[0].unique_id,
        card.unique_id
    );
    assert!(!config.monthly_card_status("demo", now).unwrap().active);
}

#[test]
fn billing_card_malformed_or_duplicate_owned_item_cannot_extend_access() {
    let config = config();
    let index = config.default_character.index;
    let now = monthly_card_now_ms();
    let card = config
        .buy_monthly_card_for_character("demo", index, "malformed-card-0001", now)
        .unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&index)
            .unwrap();
        let mut item: serde_json::Value =
            serde_json::from_str(&save.inventory_items_json[0]).unwrap();
        item["quantity"] = serde_json::json!(2);
        save.inventory_items_json[0] = serde_json::to_string(&item).unwrap();
    }
    let before = serialized(&config);
    assert!(config
        .activate_monthly_card_for_character("demo", index, card.unique_id, now)
        .is_err());
    assert_eq!(serialized(&config), before);
    assert!(!config.monthly_card_status("demo", now).unwrap().active);
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&index)
            .unwrap();
        let mut item: serde_json::Value =
            serde_json::from_str(&save.inventory_items_json[0]).unwrap();
        item["quantity"] = serde_json::json!(1);
        save.inventory_items_json[0] = serde_json::to_string(&item).unwrap();
        save.belt_items_json
            .push(save.inventory_items_json[0].clone());
    }
    let before = serialized(&config);
    assert!(config
        .activate_monthly_card_for_character("demo", index, card.unique_id, now)
        .is_err());
    assert_eq!(serialized(&config), before);
}

#[cfg(feature = "test-support")]
#[test]
fn billing_card_uncertain_file_publication_does_not_allow_a_second_activation() {
    use mir2_simulation::AccountStoreTransactionFault;
    let dir = std::env::temp_dir().join(format!(
        "mir2-billing-card-unknown-{}-{}",
        std::process::id(),
        monthly_card_now_ms()
    ));
    let path = dir.join("accounts.json");
    let mut config = SimulationConfig::default().with_account_store_path(&path);
    config.billing_monthly_card_credit_price = Some(10);
    let index = config.default_character.index;
    let now = monthly_card_now_ms();
    fund_file_wallet(&config, &path, now, "uncertain");
    let card = config
        .buy_monthly_card_for_character("demo", index, "unknown-card-0001", now)
        .unwrap();
    config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(config
        .activate_monthly_card_for_character("demo", index, card.unique_id, now)
        .is_err());
    assert!(config
        .activate_monthly_card_for_character("demo", index, card.unique_id, now)
        .is_err());
    let durable = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    let ledger = durable.accounts["demo"].monthly_card.as_ref().unwrap();
    assert_eq!(ledger.expires_at_ms, now + MONTHLY_CARD_DURATION_MS);
    assert_eq!(
        ledger
            .item_receipts
            .values()
            .filter(|row| row.redeemed_at_ms.is_some())
            .count(),
        1
    );
}

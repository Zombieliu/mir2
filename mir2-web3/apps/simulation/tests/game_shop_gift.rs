//! Headless Source transactions. Prepared test wallets are not Stripe evidence.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use mir2_game_data::BILLING_MONTHLY_CARD_GAME_SHOP_INDEX;
use mir2_protocol::{ClientPacket, MirDirection, MirGridType, Point, ServerPacket};
use mir2_simulation::monthly_card::{
    monthly_card_now_ms, MonthlyCardPolicy, MONTHLY_CARD_DURATION_MS,
};
use mir2_simulation::{
    AccountRecord, AccountStoreRepository, CharacterSaveRecord, FileAccountStoreRepository,
    GameShopPurchaseExecution, GameShopPurchaseFailure, InProcessWorldRuntime,
    NativeGameShopGiftRequest, NativeGameShopPurchaseRequest, SimulationConfig, Stage5MailMessage,
    Stage5SystemsState, WorldCommand, WorldRuntime, NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const SENDER: &str = "gift_sender";
const RECEIVER: &str = "gift_receiver";
const SENDER_INDEX: i32 = 41;
const RECEIVER_INDEX: i32 = 42;
const PASSWORD: &str = "gift-isolated-fixture-password";
const RECEIVER_NAME: &str = "GiftFriend";
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    config: SimulationConfig,
    directory: Option<PathBuf>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(directory) = &self.directory {
            // Only our uniquely generated temporary fixture directory is owned.
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

fn seed(config: &SimulationConfig) {
    let mut store = config.account_store.lock().unwrap();
    for (account_id, index, name, credit) in [
        (SENDER, SENDER_INDEX, "GiftSender", 1000),
        (RECEIVER, RECEIVER_INDEX, RECEIVER_NAME, 7),
    ] {
        let mut character = config.default_character.clone();
        character.index = index;
        character.name = name.into();
        let mut account = AccountRecord::new(character);
        account.password = PASSWORD.into();
        let save = account.saves.get_mut(&index).unwrap();
        save.credit = credit;
        save.gold = 321;
        store.accounts.insert(account_id.into(), account);
    }
    store.next_character_index = 100;
}

fn fixture(file: bool) -> Fixture {
    let directory = file.then(|| {
        std::env::temp_dir().join(format!(
            "mir2-gift-{}-{}-{}",
            std::process::id(),
            monthly_card_now_ms(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ))
    });
    let mut config = match &directory {
        Some(directory) => {
            SimulationConfig::default().with_account_store_path(directory.join("accounts.json"))
        }
        None => SimulationConfig::default(),
    };
    config.billing_monthly_card_credit_price = Some(10);
    seed(&config);
    if file {
        config.save_account_store().unwrap();
    }
    Fixture { config, directory }
}

fn start(config: &SimulationConfig, account: &str, index: i32) -> InProcessWorldRuntime {
    let mut runtime = InProcessWorldRuntime::new(config.clone());
    let login = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: account.into(),
            password: PASSWORD.into(),
        }))
        .unwrap();
    assert!(login.iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let bootstrap = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap();
    assert!(bootstrap.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(runtime.active_identity().unwrap().account_id, account);
    runtime
}

fn request(key: u8) -> NativeGameShopGiftRequest {
    NativeGameShopGiftRequest {
        purchase: NativeGameShopPurchaseRequest {
            protocol_version: NATIVE_GAME_SHOP_PURCHASE_PROTOCOL_V2,
            server_idempotency_key: URL_SAFE_NO_PAD.encode([key; 32]),
            gateway_session_id: "gift-fixture-gateway-session".into(),
            account_id: SENDER.into(),
            character_index: SENDER_INDEX,
            client_request_id: format!("gift-{key}"),
            g_index: BILLING_MONTHLY_CARD_GAME_SHOP_INDEX,
            quantity: 1,
            price_type: 0,
        },
        recipient_name: RECEIVER_NAME.into(),
    }
}

fn gift(
    runtime: &mut InProcessWorldRuntime,
    request: NativeGameShopGiftRequest,
) -> Result<GameShopPurchaseExecution, String> {
    let execution = runtime.execute_with_outcome(WorldCommand::NativeGameShopGift(request))?;
    Ok(GameShopPurchaseExecution {
        packets: execution.packets,
        outcome: execution
            .game_shop_purchase_outcome
            .expect("Gift must have a typed Source receipt"),
    })
}

fn save(config: &SimulationConfig, account: &str, index: i32) -> CharacterSaveRecord {
    config.account_store.lock().unwrap().accounts[account].saves[&index].clone()
}

fn mail(config: &SimulationConfig, account: &str, index: i32) -> Vec<Stage5MailMessage> {
    serde_json::from_str::<Stage5SystemsState>(
        save(config, account, index)
            .stage5_systems_json
            .as_deref()
            .unwrap_or("{}"),
    )
    .unwrap()
    .mail
}

fn serialized(config: &SimulationConfig) -> serde_json::Value {
    serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
}

fn card_uid(config: &SimulationConfig, account: &str) -> u64 {
    config.account_store.lock().unwrap().accounts[account]
        .monthly_card
        .as_ref()
        .unwrap()
        .item_receipts
        .values()
        .next()
        .unwrap()
        .unique_id
}

fn fill_mail(config: &SimulationConfig, account_id: &str, index: i32) {
    let mut store = config.account_store.lock().unwrap();
    let save = store
        .accounts
        .get_mut(account_id)
        .unwrap()
        .saves
        .get_mut(&index)
        .unwrap();
    let mut state = Stage5SystemsState::default();
    for id in 1..=100 {
        state.mail.push(Stage5MailMessage {
            id,
            delivery_nonce: format!("gift-fixture-full-{account_id}-{id}"),
            from: "Fixture".into(),
            to: save.character.name.clone(),
            subject: "Full mailbox fixture".into(),
            body: String::new(),
            gold: 0,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        });
    }
    save.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
}

#[test]
fn gift_monthly_card_debits_payer_and_exact_recipient_can_claim_use_once() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let recipient_before = save(config, RECEIVER, RECEIVER_INDEX);
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let bought = gift(&mut sender, request(1)).unwrap();
    assert!(bought.outcome.success);
    assert_eq!(save(config, SENDER, SENDER_INDEX).credit, 990);
    let received = save(config, RECEIVER, RECEIVER_INDEX);
    assert_eq!(received.credit, recipient_before.credit);
    assert_eq!(received.revision, recipient_before.revision);
    assert_eq!(
        received.inventory_items_json,
        recipient_before.inventory_items_json
    );
    assert!(bought
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseCredit { credit: 10 })));
    assert!(!bought
        .packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ReceiveMail { .. })));
    let uid = card_uid(config, RECEIVER);
    assert!(config
        .list_billing_monthly_cards(SENDER, SENDER_INDEX)
        .unwrap()
        .is_empty());
    assert!(config
        .activate_monthly_card_for_character(SENDER, SENDER_INDEX, uid, monthly_card_now_ms())
        .is_err());
    let mut recipient = start(config, RECEIVER, RECEIVER_INDEX);
    let claim = recipient
        .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
            mail_id: bought.outcome.mail_id.unwrap(),
        }))
        .unwrap();
    assert!(claim.iter().any(
        |packet| matches!(packet, ServerPacket::GainedItem { item } if item.unique_id == uid)
    ));
    let use_item = ClientPacket::UseItem {
        unique_id: uid,
        grid: MirGridType::Inventory,
    };
    assert!(recipient.execute(WorldCommand::ClientPacket(use_item.clone())).unwrap().iter()
        .any(|packet| matches!(packet, ServerPacket::UseItem { unique_id, success: true, .. } if *unique_id == uid)));
    let expiry = config
        .monthly_card_status(RECEIVER, monthly_card_now_ms())
        .unwrap()
        .expires_at_ms;
    recipient
        .execute(WorldCommand::ClientPacket(use_item))
        .unwrap();
    assert_eq!(
        config
            .monthly_card_status(RECEIVER, monthly_card_now_ms())
            .unwrap()
            .expires_at_ms,
        expiry
    );
    assert_eq!(save(config, RECEIVER, RECEIVER_INDEX).credit, 7);
    let before = serialized(config);
    assert_eq!(
        gift(&mut sender, request(1)).unwrap().outcome,
        bought.outcome
    );
    assert_eq!(serialized(config), before);
}

#[test]
fn gift_key_and_client_correlation_cannot_rebind_recipient_or_payer() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    gift(&mut sender, request(2)).unwrap();
    let before = serialized(config);
    let mut changed = request(2);
    changed.recipient_name = "OtherName".into();
    assert!(gift(&mut sender, changed)
        .unwrap_err()
        .contains("ReplayMismatch"));
    let mut changed = request(2);
    changed.purchase.server_idempotency_key = URL_SAFE_NO_PAD.encode([91u8; 32]);
    assert!(gift(&mut sender, changed)
        .unwrap_err()
        .contains("ReplayMismatch"));
    let mut changed = request(2);
    changed.purchase.quantity = 2;
    assert!(gift(&mut sender, changed).is_err());
    let mut changed = request(2);
    changed.purchase.account_id = RECEIVER.into();
    assert!(gift(&mut sender, changed).is_err());
    assert_eq!(serialized(config), before);
}

#[test]
fn gift_recipient_name_obeys_crystal_character_rule_before_any_write() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let before = serialized(config);
    for name in [
        "ab",
        "abcdefghijklmnop",
        " GiftFriend",
        "GiftFriend ",
        "Gift Friend",
        "Gift-Friend",
        "Gift.Friend",
        "Gift\nFriend",
        "Gifté",
        "Gift😀",
        "Gift\u{4dff}",
        "Gift\u{9fa6}",
    ] {
        let mut invalid = request(20);
        invalid.recipient_name = name.into();
        assert_eq!(
            gift(&mut sender, invalid).unwrap_err(),
            "gameShopGiftRecipientNameInvalid",
            "invalid Crystal character name {name:?}"
        );
        assert_eq!(serialized(config), before);
    }
    let accepted = "中".repeat(15);
    assert_eq!(accepted.len(), 45);
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut(RECEIVER).unwrap();
        account.characters[0].name = accepted.clone();
        account
            .saves
            .get_mut(&RECEIVER_INDEX)
            .unwrap()
            .character
            .name = accepted.clone();
    }
    let mut valid = request(20);
    valid.recipient_name = accepted;
    assert!(gift(&mut sender, valid).unwrap().outcome.success);
    assert_eq!(save(config, SENDER, SENDER_INDEX).credit, 990);
    assert_eq!(mail(config, RECEIVER, RECEIVER_INDEX).len(), 1);
}

#[test]
fn gift_client_view_hides_both_internal_ledgers_and_preserves_raw_snapshot() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let purchase = request(21).purchase;
    let self_purchase = sender
        .execute_with_outcome(WorldCommand::NativeGameShopPurchase(purchase.clone()))
        .unwrap();
    assert!(self_purchase.game_shop_purchase_outcome.unwrap().success);
    let gifted = request(22);
    assert!(gift(&mut sender, gifted.clone()).unwrap().outcome.success);

    let raw = sender.world_snapshot();
    let raw_json = serde_json::to_value(&raw).unwrap();
    assert_eq!(
        raw.stage5_systems
            .mail
            .iter()
            .filter(|row| matches!(
                row.subject.as_str(),
                "NativeGameShopLedgerV2" | "NativeGameShopGiftLedgerV1"
            ))
            .count(),
        2
    );
    let expected_visible_mail: Vec<_> = raw
        .stage5_systems
        .mail
        .iter()
        .filter(|row| !row.deleted)
        .collect();
    assert_eq!(expected_visible_mail.len(), 1);
    let projected = serde_json::to_value(raw.client_view()).unwrap();
    assert_eq!(
        projected["stage5Systems"]["mail"],
        serde_json::to_value(&expected_visible_mail).unwrap()
    );
    let client_json = serde_json::to_string(&projected).unwrap();
    for private in [
        "NativeGameShopLedgerV2",
        "NativeGameShopGiftLedgerV1",
        "native-gameshop-ledger-v2",
        "native-gameshop-gift-ledger-v1",
        purchase.server_idempotency_key.as_str(),
        gifted.purchase.server_idempotency_key.as_str(),
        purchase.gateway_session_id.as_str(),
        RECEIVER,
    ] {
        assert!(!client_json.contains(private), "client leaked {private}");
    }
    assert_eq!(serde_json::to_value(&raw).unwrap(), raw_json);
    assert_eq!(sender.world_snapshot(), raw);
}

#[test]
fn gift_replay_does_not_resolve_a_renamed_or_reused_character_name() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let bought = gift(&mut sender, request(3)).unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut(RECEIVER).unwrap();
        account.characters[0].name = "RenamedFriend".into();
        account
            .saves
            .get_mut(&RECEIVER_INDEX)
            .unwrap()
            .character
            .name = "RenamedFriend".into();
        let mut character = config.default_character.clone();
        character.name = RECEIVER_NAME.into();
        character.index = 43;
        store
            .accounts
            .insert("reused_name".into(), AccountRecord::new(character));
    }
    let before = serialized(config);
    assert_eq!(
        gift(&mut sender, request(3)).unwrap().outcome,
        bought.outcome
    );
    assert_eq!(serialized(config), before);
    assert!(mail(config, "reused_name", 43).is_empty());
    assert_eq!(mail(config, RECEIVER, RECEIVER_INDEX).len(), 1);
}

#[test]
fn gift_rejects_missing_ambiguous_self_gold_and_replays_business_rejection() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let mut missing = request(4);
    missing.recipient_name = "MissingFriend".into();
    assert_eq!(
        gift(&mut sender, missing.clone()).unwrap().outcome.failure,
        Some(GameShopPurchaseFailure::RecipientUnavailable)
    );
    {
        let mut store = config.account_store.lock().unwrap();
        let mut character = config.default_character.clone();
        character.name = "MissingFriend".into();
        character.index = 44;
        store
            .accounts
            .insert("created_later".into(), AccountRecord::new(character));
        let mut duplicate = config.default_character.clone();
        duplicate.name = "giftfriend".into();
        duplicate.index = 45;
        store
            .accounts
            .insert("ambiguous".into(), AccountRecord::new(duplicate));
    }
    assert_eq!(
        gift(&mut sender, missing).unwrap().outcome.failure,
        Some(GameShopPurchaseFailure::RecipientUnavailable)
    );
    assert_eq!(
        gift(&mut sender, request(5)).unwrap().outcome.failure,
        Some(GameShopPurchaseFailure::RecipientUnavailable)
    );
    let mut self_gift = request(6);
    self_gift.recipient_name = "GiftSender".into();
    assert_eq!(
        gift(&mut sender, self_gift).unwrap().outcome.failure,
        Some(GameShopPurchaseFailure::SelfGiftUnavailable)
    );
    let mut gold = request(7);
    gold.purchase.price_type = 1;
    assert_eq!(
        gift(&mut sender, gold).unwrap().outcome.failure,
        Some(GameShopPurchaseFailure::PaymentUnavailable)
    );
    assert_eq!(save(config, SENDER, SENDER_INDEX).credit, 1000);
    assert_eq!(save(config, SENDER, SENDER_INDEX).gold, 321);
    assert!(config
        .list_billing_monthly_cards(RECEIVER, RECEIVER_INDEX)
        .unwrap()
        .is_empty());
}

#[test]
fn gift_recipient_full_is_durable_rejection_but_sender_full_does_not_block() {
    let fixture = fixture(false);
    let config = &fixture.config;
    fill_mail(config, RECEIVER, RECEIVER_INDEX);
    fill_mail(config, SENDER, SENDER_INDEX);
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let failed = gift(&mut sender, request(8)).unwrap();
    assert_eq!(
        failed.outcome.failure,
        Some(GameShopPurchaseFailure::MailFull)
    );
    assert_eq!(save(config, SENDER, SENDER_INDEX).credit, 1000);
    assert_eq!(mail(config, RECEIVER, RECEIVER_INDEX).len(), 100);
    assert!(config
        .list_billing_monthly_cards(RECEIVER, RECEIVER_INDEX)
        .unwrap()
        .is_empty());
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(RECEIVER)
        .unwrap()
        .saves
        .get_mut(&RECEIVER_INDEX)
        .unwrap()
        .stage5_systems_json = None;
    assert_eq!(
        gift(&mut sender, request(8)).unwrap().outcome,
        failed.outcome
    );
    assert!(gift(&mut sender, request(9)).unwrap().outcome.success);
    assert_eq!(save(config, SENDER, SENDER_INDEX).credit, 990);
    assert_eq!(
        mail(config, SENDER, SENDER_INDEX)
            .iter()
            .filter(|mail| !mail.deleted)
            .count(),
        100
    );
    assert_eq!(mail(config, RECEIVER, RECEIVER_INDEX).len(), 1);
}

#[test]
fn gift_same_account_distinct_characters_preserves_both_saves_and_recipient_ownership() {
    let fixture = fixture(false);
    let config = &fixture.config;
    {
        let mut store = config.account_store.lock().unwrap();
        let receiver = store.accounts.remove(RECEIVER).unwrap();
        let sender = store.accounts.get_mut(SENDER).unwrap();
        sender.characters.extend(receiver.characters);
        sender.saves.extend(receiver.saves);
    }
    let recipient_revision = save(config, SENDER, RECEIVER_INDEX).revision;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    assert!(gift(&mut sender, request(10)).unwrap().outcome.success);
    assert_eq!(save(config, SENDER, SENDER_INDEX).credit, 990);
    assert_eq!(save(config, SENDER, RECEIVER_INDEX).credit, 7);
    assert_eq!(
        save(config, SENDER, RECEIVER_INDEX).revision,
        recipient_revision
    );
    let uid = card_uid(config, SENDER);
    assert!(config
        .activate_monthly_card_for_character(SENDER, SENDER_INDEX, uid, monthly_card_now_ms())
        .is_err());
    assert!(
        !config
            .activate_monthly_card_for_character(SENDER, RECEIVER_INDEX, uid, monthly_card_now_ms())
            .unwrap()
            .replayed
    );
    let before = serialized(config);
    gift(&mut sender, request(10)).unwrap();
    assert_eq!(serialized(config), before);
}

#[test]
fn gift_online_tick_mail_merge_and_logout_preserve_unsaved_gold_and_transform() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let mut recipient = start(config, RECEIVER, RECEIVER_INDEX);
    let original = save(config, RECEIVER, RECEIVER_INDEX);
    let position = Point {
        x: original.position.x + 2,
        y: original.position.y + 1,
    };
    recipient.force_authoritative_player_transform(position.clone(), MirDirection::Left);
    assert!(recipient
        .execute(WorldCommand::CreditGoldFromOre {
            account: RECEIVER.into(),
            gold: 77,
            idempotency_key: "gift-unsaved-owner-coin-fixture".into(),
        })
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedGold { gold: 77 })));
    let bought = gift(&mut sender, request(11)).unwrap();
    assert_eq!(
        save(config, RECEIVER, RECEIVER_INDEX).revision,
        original.revision
    );
    // Only ordinary Source Tick: no panel command or manual refresh.
    let notifications = recipient.execute(WorldCommand::Tick).unwrap();
    let delivered_id = bought.outcome.mail_id.unwrap();
    assert!(notifications.iter().any(|packet| matches!(
        packet,
        ServerPacket::ReceiveMail { mail }
            if mail.iter().any(|row| row.mail_id == delivered_id)
    )));
    assert!(!recipient
        .execute(WorldCommand::Tick)
        .unwrap()
        .iter()
        .any(|packet| matches!(
            packet,
            ServerPacket::ReceiveMail { mail }
                if mail.iter().any(|row| row.mail_id == delivered_id)
        )));
    assert!(recipient
        .world_snapshot()
        .stage5_systems
        .mail
        .iter()
        .any(|mail| u64::from(mail.id) == bought.outcome.mail_id.unwrap()));
    recipient
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    let durable = save(config, RECEIVER, RECEIVER_INDEX);
    assert_eq!(durable.gold, original.gold + 77);
    assert_eq!(
        (durable.position, durable.direction),
        (position, MirDirection::Left)
    );
    assert_eq!(durable.credit, original.credit);
    assert_eq!(
        mail(config, RECEIVER, RECEIVER_INDEX)
            .iter()
            .filter(|mail| !mail.deleted)
            .count(),
        1
    );
    let mut reconnect = start(config, RECEIVER, RECEIVER_INDEX);
    let uid = card_uid(config, RECEIVER);
    assert!(reconnect
        .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
            mail_id: bought.outcome.mail_id.unwrap(),
        }))
        .unwrap()
        .iter()
        .any(
            |packet| matches!(packet, ServerPacket::GainedItem { item } if item.unique_id == uid)
        ));
}

#[test]
fn gift_actual_file_restart_retains_receipt_and_offline_expired_recipient_can_activate() {
    let fixture = fixture(true);
    let config = &fixture.config;
    let path = fixture.directory.as_ref().unwrap().join("accounts.json");
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let bought = gift(&mut sender, request(12)).unwrap();
    let uid = card_uid(config, RECEIVER);
    drop(sender);
    let mut reopened = SimulationConfig::default().with_account_store_path(&path);
    // Replay succeeds even when the current catalog no longer sells this item.
    reopened.billing_monthly_card_credit_price = None;
    let mut restarted_sender = start(&reopened, SENDER, SENDER_INDEX);
    let before = std::fs::read(&path).unwrap();
    assert_eq!(
        gift(&mut restarted_sender, request(12)).unwrap().outcome,
        bought.outcome
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(restarted_sender);
    reopened.monthly_card_policy = MonthlyCardPolicy::new(
        true,
        Some("gift-isolated-policy-key-only-not-a-deployment-key"),
    )
    .unwrap();
    let now = monthly_card_now_ms();
    assert!(
        !reopened
            .monthly_card_status(RECEIVER, now)
            .unwrap()
            .can_enter_game
    );
    let first = reopened
        .activate_monthly_card_for_character(RECEIVER, RECEIVER_INDEX, uid, now)
        .unwrap();
    assert!(!first.replayed && first.status.can_enter_game);
    assert_eq!(
        first.status.expires_at_ms,
        Some(now + MONTHLY_CARD_DURATION_MS)
    );
    let loaded = FileAccountStoreRepository::new(&path)
        .load(reopened.default_character.clone())
        .unwrap();
    assert_eq!(loaded.accounts[SENDER].saves[&SENDER_INDEX].credit, 990);
    assert_eq!(loaded.accounts[RECEIVER].saves[&RECEIVER_INDEX].credit, 7);
    assert_eq!(
        loaded.accounts[RECEIVER]
            .monthly_card
            .as_ref()
            .unwrap()
            .item_receipts
            .len(),
        1
    );
    assert!(
        reopened
            .activate_monthly_card_for_character(RECEIVER, RECEIVER_INDEX, uid, now + 1)
            .unwrap()
            .replayed
    );
}

#[cfg(feature = "test-support")]
#[test]
fn gift_actual_file_known_failure_before_persist_or_rename_rolls_back_every_effect() {
    use mir2_simulation::AccountStoreTransactionFault;
    for fault in [
        AccountStoreTransactionFault::BeforePersist,
        AccountStoreTransactionFault::BeforeFileRename,
    ] {
        let fixture = fixture(true);
        let config = &fixture.config;
        let path = fixture.directory.as_ref().unwrap().join("accounts.json");
        let mut sender = start(config, SENDER, SENDER_INDEX);
        let before = serialized(config);
        let file_before = std::fs::read(&path).unwrap();
        config.inject_account_store_transaction_fault(fault);
        assert!(!gift(&mut sender, request(13))
            .unwrap_err()
            .contains("OUTCOME_UNKNOWN"));
        assert_eq!(serialized(config), before);
        assert_eq!(std::fs::read(&path).unwrap(), file_before);
        assert!(gift(&mut sender, request(13)).unwrap().outcome.success);
        assert_eq!(save(config, SENDER, SENDER_INDEX).credit, 990);
        assert_eq!(mail(config, RECEIVER, RECEIVER_INDEX).len(), 1);
        let committed = std::fs::read(&path).unwrap();
        assert!(gift(&mut sender, request(13)).unwrap().packets.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), committed);
    }
}

#[cfg(feature = "test-support")]
#[test]
fn gift_actual_file_unknown_commit_freezes_all_touched_accounts_and_reopen() {
    use mir2_simulation::AccountStoreTransactionFault;
    let mut fixture = fixture(true);
    let config = &fixture.config;
    let path = fixture.directory.as_ref().unwrap().join("accounts.json");
    let mut sender = start(config, SENDER, SENDER_INDEX);
    let mut recipient = start(config, RECEIVER, RECEIVER_INDEX);
    let memory_before = serialized(config);
    config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(gift(&mut sender, request(14))
        .unwrap_err()
        .contains("OUTCOME_UNKNOWN"));
    assert_eq!(serialized(config), memory_before);
    let committed_bytes = std::fs::read(&path).unwrap();
    assert!(gift(&mut sender, request(14))
        .unwrap_err()
        .contains("writes are frozen"));
    assert!(recipient
        .save_active_character()
        .unwrap_err()
        .contains("writes are frozen"));
    assert!(recipient
        .execute(WorldCommand::BillingBuyMonthlyCard {
            character_index: RECEIVER_INDEX,
            request_id: "gift-recipient-after-unknown".into(),
        })
        .unwrap_err()
        .contains("writes are frozen"));
    // A separately configured handle for the same File authority is frozen,
    // not only the sender session's local config.
    let another_handle = SimulationConfig::default()
        .with_account_store_path(&path)
        .with_billing_monthly_card_credit_price(Some(10))
        .unwrap();
    assert!(another_handle
        .buy_monthly_card_for_character(
            RECEIVER,
            RECEIVER_INDEX,
            "gift-recipient-another-handle",
            monthly_card_now_ms(),
        )
        .unwrap_err()
        .contains("writes are frozen"));
    assert_eq!(serialized(config), memory_before);
    assert_eq!(std::fs::read(&path).unwrap(), committed_bytes);
    let durable = FileAccountStoreRepository::new(&path)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(durable.accounts[SENDER].saves[&SENDER_INDEX].credit, 990);
    assert_eq!(durable.accounts[RECEIVER].saves[&RECEIVER_INDEX].credit, 7);
    assert_eq!(
        durable.accounts[RECEIVER]
            .monthly_card
            .as_ref()
            .unwrap()
            .item_receipts
            .len(),
        1
    );
    drop(another_handle);
    drop(recipient);
    drop(sender);
    // Release the in-process authority. The next handle must load the actual
    // committed bytes and its durable unknown-publication fence.
    fixture.config = SimulationConfig::default();
    let reopened = SimulationConfig::default()
        .with_account_store_path(&path)
        .with_billing_monthly_card_credit_price(Some(10))
        .unwrap();
    assert_eq!(save(&reopened, SENDER, SENDER_INDEX).credit, 990);
    assert_eq!(mail(&reopened, RECEIVER, RECEIVER_INDEX).len(), 1);
    let ledger = mail(&reopened, SENDER, SENDER_INDEX)
        .into_iter()
        .find(|row| row.subject == "NativeGameShopGiftLedgerV1")
        .unwrap();
    let receipt: serde_json::Value = serde_json::from_str(&ledger.body).unwrap();
    assert_eq!(receipt["entries"].as_array().unwrap().len(), 1);
    assert_eq!(
        receipt["entries"][0]["purchase"],
        serde_json::to_value(request(14).purchase).unwrap()
    );
    assert!(reopened
        .buy_monthly_card_for_character(
            RECEIVER,
            RECEIVER_INDEX,
            "gift-recipient-after-authority-restart",
            monthly_card_now_ms(),
        )
        .unwrap_err()
        .contains("writes are frozen"));
    assert_eq!(std::fs::read(&path).unwrap(), committed_bytes);
}

#[test]
fn gift_normal_catalog_item_is_exact_mail_and_claim_does_not_spend_recipient_credit() {
    let fixture = fixture(false);
    let config = &fixture.config;
    // Use the actual normal StartGame catalog rather than fabricate a product.
    let mut sender = InProcessWorldRuntime::new(config.clone());
    sender
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: SENDER.into(),
            password: PASSWORD.into(),
        }))
        .unwrap();
    let packets = sender
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: SENDER_INDEX,
        }))
        .unwrap();
    let item = packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::GameShopInfo { item, .. }
                if item.g_index != BILLING_MONTHLY_CARD_GAME_SHOP_INDEX
                    && item.class.eq_ignore_ascii_case("all")
                    && item.can_buy_credit
                    && item.credit_price > 0
                    && item.credit_price <= 1000 =>
            {
                Some(item.clone())
            }
            _ => None,
        })
        .expect("actual Source catalog must contain an affordable ordinary credit product");
    let mut ordinary = request(15);
    ordinary.purchase.g_index = item.g_index;
    let bought = gift(&mut sender, ordinary.clone()).unwrap();
    assert!(bought.outcome.success);
    let delivered = mail(config, RECEIVER, RECEIVER_INDEX).pop().unwrap();
    assert!(!delivered.item_states_json.is_empty());
    assert_eq!(delivered.id as u64, bought.outcome.mail_id.unwrap());
    assert_eq!(
        save(config, SENDER, SENDER_INDEX).credit,
        1000 - item.credit_price
    );
    assert_eq!(save(config, RECEIVER, RECEIVER_INDEX).credit, 7);
    let mut recipient = start(config, RECEIVER, RECEIVER_INDEX);
    assert!(recipient
        .execute(WorldCommand::ClientPacket(ClientPacket::CollectParcel {
            mail_id: u64::from(delivered.id),
        }))
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::GainedItem { .. })));
    let after = serialized(config);
    gift(&mut sender, ordinary).unwrap();
    assert_eq!(serialized(config), after);
}

#[test]
fn gift_stale_owner_and_malformed_ledger_fail_without_debit_or_delivery() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    {
        let mut store = config.account_store.lock().unwrap();
        store
            .accounts
            .get_mut(SENDER)
            .unwrap()
            .saves
            .get_mut(&SENDER_INDEX)
            .unwrap()
            .revision += 1;
    }
    let before = serialized(config);
    assert!(gift(&mut sender, request(16))
        .unwrap_err()
        .contains("CheckpointStale"));
    assert_eq!(serialized(config), before);
    let mut current = start(config, SENDER, SENDER_INDEX);
    gift(&mut current, request(17)).unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut(SENDER)
            .unwrap()
            .saves
            .get_mut(&SENDER_INDEX)
            .unwrap();
        let mut state: Stage5SystemsState =
            serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
        let hidden = state
            .mail
            .iter_mut()
            .find(|mail| mail.subject == "NativeGameShopGiftLedgerV1")
            .unwrap();
        hidden.locked = false;
        save.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
    }
    let before = serialized(config);
    assert!(gift(&mut current, request(18)).is_err());
    assert_eq!(serialized(config), before);
}

#[test]
fn gift_recipient_unit_without_payer_receipt_never_recreates_a_paid_outcome() {
    let fixture = fixture(false);
    let config = &fixture.config;
    let mut sender = start(config, SENDER, SENDER_INDEX);
    assert!(gift(&mut sender, request(20)).unwrap().outcome.success);
    {
        let mut store = config.account_store.lock().unwrap();
        let payer = store.accounts.get_mut(SENDER).unwrap()
            .saves.get_mut(&SENDER_INDEX).unwrap();
        let mut state: Stage5SystemsState = serde_json::from_str(
            payer.stage5_systems_json.as_deref().unwrap(),
        ).unwrap();
        state.mail.retain(|mail| mail.subject != "NativeGameShopGiftLedgerV1");
        payer.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
    }
    let mut fresh_sender = start(config, SENDER, SENDER_INDEX);
    let before = serialized(config);
    assert_eq!(gift(&mut fresh_sender, request(20)).unwrap_err(),
        "gameShopGiftItemReceiptMissingOutcome");
    assert_eq!(serialized(config), before);
    assert_eq!(save(config, SENDER, SENDER_INDEX).credit, 990);
    assert_eq!(mail(config, RECEIVER, RECEIVER_INDEX).len(), 1);
}

#[test]
#[ignore = "requires MIR2_BILLING_TEST_ISOLATED=1 and a dedicated mir2_billing_qa_ PostgreSQL database"]
fn gift_postgres_independent_writers_mail_cas_and_online_owner_survive() {
    use mir2_simulation::AccountStoreDatabaseMode;
    use postgres::{Client, Config, NoTls};
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    assert_eq!(
        std::env::var("MIR2_BILLING_TEST_ISOLATED").ok().as_deref(),
        Some("1"),
        "explicit isolated billing opt-in required"
    );
    let url = std::env::var("MIR2_BILLING_TEST_DATABASE_URL")
        .expect("explicit private billing QA URI required");
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "PostgreSQL QA URI required"
    );
    let parsed: Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid QA URI; credentials suppressed"));
    let database = parsed.get_dbname().unwrap_or_default();
    assert!(
        database.starts_with("mir2_billing_qa_")
            && (28..=63).contains(&database.len())
            && database
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "dedicated generated billing QA database required"
    );
    assert!(
        parsed.get_options().is_none(),
        "QA base URI cannot override schema options"
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("gift_qa_{}_{stamp}", std::process::id());
    assert!(schema.len() <= 63);
    let mut base = Client::connect(&url, NoTls)
        .unwrap_or_else(|_| panic!("QA connect failed; credentials suppressed"));
    assert_eq!(
        base.query_one("SELECT current_database()", &[])
            .unwrap()
            .get::<_, String>(0),
        database
    );
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    eprintln!("gift-pg: owned schema created");
    let scoped = format!("{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",
        if url.contains('?') { "&" } else { "?" });
    let open = |uri: &str| {
        let config = SimulationConfig::default()
            .with_postgres_account_store(uri.to_owned())
            .unwrap_or_else(|_| panic!("QA load failed; credentials suppressed"))
            .with_billing_monthly_card_credit_price(Some(10))
            .unwrap();
        assert_eq!(
            config.account_store_database_mode,
            AccountStoreDatabaseMode::SourceOfTruth
        );
        assert!(config.account_store_path.is_none());
        config
    };
    let seed_config = open(&scoped);
    eprintln!("gift-pg: seed repository opened");
    seed(&seed_config);
    for account in [SENDER, RECEIVER] {
        seed_config
            .save_account_store_account(account)
            .unwrap_or_else(|_| panic!("QA seed failed; credentials suppressed"));
    }
    eprintln!("gift-pg: sender and recipient persisted");
    // Migrate the prepared legacy password through ordinary authentication
    // before opening contender caches. This fixture races Gift CAS, not two
    // password migrations for the same prepared account. Each contender still
    // performs its own normal password login and StartGame below.
    let mut credential_bootstrap = InProcessWorldRuntime::new(seed_config.clone());
    let bootstrap_packets = credential_bootstrap
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: SENDER.into(),
            password: PASSWORD.into(),
        }))
        .unwrap();
    assert!(bootstrap_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    drop(credential_bootstrap);
    eprintln!("gift-pg: ordinary sender credential migration completed before contenders");
    let recipient_config = open(&scoped);
    eprintln!("gift-pg: recipient repository opened");
    let mut recipient = start(&recipient_config, RECEIVER, RECEIVER_INDEX);
    eprintln!("gift-pg: recipient authenticated and entered");
    let original = save(&recipient_config, RECEIVER, RECEIVER_INDEX);
    assert!(!recipient
        .execute(WorldCommand::CreditGoldFromOre {
            account: RECEIVER.into(),
            gold: 77,
            idempotency_key: "gift-pg-unsaved-gold-fixture".into(),
        })
        .unwrap()
        .is_empty());
    eprintln!("gift-pg: unsaved recipient gold staged");
    let position = Point {
        x: original.position.x + 2,
        y: original.position.y + 1,
    };
    recipient.force_authoritative_player_transform(position.clone(), MirDirection::Left);
    let configs = [open(&scoped), open(&scoped)];
    eprintln!("gift-pg: independent writer repositories opened");
    // Every ordinary StartGame intentionally saves the owner and advances its
    // revision. Complete both before preparing this Source CAS race; otherwise
    // the second startup correctly makes the first live checkpoint stale.
    let mut writers: Vec<_> = configs.into_iter()
        .map(|writer| {
            let runtime = start(&writer, SENDER, SENDER_INDEX);
            (writer, runtime)
        }).collect();
    let canonical = save(&open(&scoped), SENDER, SENDER_INDEX);
    assert_eq!(canonical.character.name, "GiftSender");
    assert_eq!(canonical.character.index, SENDER_INDEX);
    assert_eq!(canonical.credit, 1000);
    assert!(serde_json::from_str::<Stage5SystemsState>(
        canonical.stage5_systems_json.as_deref().unwrap(),
    ).unwrap().mail.iter().all(|mail| mail.subject != "NativeGameShopGiftLedgerV1"));
    for (_, runtime) in &mut writers {
        // Trusted, test-only preparation of the complete authoritative owner
        // checkpoint, not a revision-only rebase or a public client command.
        // No game action has run on either sender. Recipient state is untouched.
        runtime.restore_active_character_checkpoint(&canonical).unwrap();
        let checkpoint = runtime.active_character_checkpoint().unwrap();
        assert_eq!(checkpoint.revision, canonical.revision);
        assert_eq!(checkpoint.character, canonical.character);
        assert_eq!(checkpoint.credit, 1000);
    }
    eprintln!("gift-pg: both authenticated Source contender checkpoints prepared");
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = writers
        .into_iter()
        .map(|(_writer, mut runtime)| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                eprintln!("gift-pg: writer gift transaction begins");
                gift(&mut runtime, request(19))
                    .unwrap_or_else(|error| {
                        // Print only fixed, allowlisted domain labels. Repository
                        // errors may contain a private URI or account details.
                        let label = match error.as_str() {
                            "gameShopGiftItemReceiptMissingOutcome" => "item-receipt-without-outcome",
                            "gameShopGiftOwnerCheckpointStale" => "owner-checkpoint-stale",
                            "gameShopGiftReplayMismatch" => "request-replay-mismatch",
                            _ if error.starts_with("stale postgres account-store write") => "account-cas-stale",
                            _ if error.starts_with("stale postgres character-save write") => "character-cas-stale",
                            _ if error.starts_with("stale postgres game-shop global-stock write") => "stock-cas-stale",
                            _ if error.contains("OUTCOME_UNKNOWN") => "outcome-unknown",
                            _ => "other-error-redacted",
                        };
                        panic!("QA Gift CAS failed: {label}; credentials suppressed")
                    })
                    .outcome
            })
        })
        .collect();
    let joined: Vec<_> = threads.into_iter().map(|thread| thread.join()).collect();
    let outcomes: Vec<_> = joined.into_iter().map(|result| result.unwrap()).collect();
    eprintln!("gift-pg: competing writers joined");
    assert!(outcomes[0].success);
    assert_eq!(outcomes[0], outcomes[1]);
    let observer = open(&scoped);
    assert_eq!(save(&observer, SENDER, SENDER_INDEX).credit, 990);
    assert_eq!(save(&observer, RECEIVER, RECEIVER_INDEX).credit, 7);
    assert_eq!(
        save(&observer, RECEIVER, RECEIVER_INDEX).revision,
        original.revision
    );
    assert_eq!(mail(&observer, RECEIVER, RECEIVER_INDEX).len(), 1);
    assert_eq!(
        observer.account_store.lock().unwrap().accounts[RECEIVER]
            .monthly_card
            .as_ref()
            .unwrap()
            .item_receipts
            .len(),
        1
    );
    // Ordinary commercial Source polling discovers a Gift written by another
    // process even though this recipient has never had a recharge order.
    let notifications = recipient.execute(WorldCommand::Tick).unwrap();
    eprintln!("gift-pg: ordinary recipient tick returned");
    let delivered_id = outcomes[0].mail_id.unwrap();
    assert!(notifications.iter().any(|packet| matches!(
        packet,
        ServerPacket::ReceiveMail { mail }
            if mail.iter().any(|row| row.mail_id == delivered_id)
    )));
    assert!(recipient
        .world_snapshot()
        .stage5_systems
        .mail
        .iter()
        .any(|row| u64::from(row.id) == delivered_id));
    recipient
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    eprintln!("gift-pg: recipient logout completed");
    let reopened = open(&scoped);
    let durable = save(&reopened, RECEIVER, RECEIVER_INDEX);
    assert_eq!(durable.gold, original.gold + 77);
    assert_eq!(
        (durable.position, durable.direction),
        (position, MirDirection::Left)
    );
    assert_eq!(durable.credit, 7);
    assert_eq!(mail(&reopened, RECEIVER, RECEIVER_INDEX).len(), 1);
    let uid = card_uid(&reopened, RECEIVER);
    assert!(
        !reopened
            .activate_monthly_card_for_character(
                RECEIVER,
                RECEIVER_INDEX,
                uid,
                monthly_card_now_ms()
            )
            .unwrap()
            .replayed
    );
    assert!(
        reopened
            .activate_monthly_card_for_character(
                RECEIVER,
                RECEIVER_INDEX,
                uid,
                monthly_card_now_ms()
            )
            .unwrap()
            .replayed
    );
    assert_eq!(save(&open(&scoped), SENDER, SENDER_INDEX).credit, 990);
    eprintln!("gift-pg: durable receipt and activation replay verified");
    // A recipient unit is not proof of a complete payer transaction. Exercise
    // the PostgreSQL reread path against an intentionally incomplete receipt in
    // this owned fixture, and require failure without minting or another debit.
    let orphaned = open(&scoped);
    {
        let mut store = orphaned.account_store.lock().unwrap();
        let payer = store.accounts.get_mut(SENDER).unwrap()
            .saves.get_mut(&SENDER_INDEX).unwrap();
        let mut state: Stage5SystemsState = serde_json::from_str(
            payer.stage5_systems_json.as_deref().unwrap(),
        ).unwrap();
        state.mail.retain(|mail| mail.subject != "NativeGameShopGiftLedgerV1");
        payer.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
    }
    orphaned.save_account_store_account(SENDER)
        .unwrap_or_else(|_| panic!("QA orphan fixture save failed; credentials suppressed"));
    let observed = open(&scoped);
    let mut fresh_sender = start(&observed, SENDER, SENDER_INDEX);
    let before = serialized(&observed);
    assert_eq!(gift(&mut fresh_sender, request(19)).unwrap_err(),
        "gameShopGiftItemReceiptMissingOutcome");
    assert_eq!(serialized(&observed), before);
    assert_eq!(save(&open(&scoped), SENDER, SENDER_INDEX).credit, 990);
    assert_eq!(mail(&open(&scoped), RECEIVER, RECEIVER_INDEX).len(), 1);
    eprintln!("gift-pg: incomplete payer receipt rejected without write");
    // This schema was generated above inside the verified dedicated QA DB.
    base.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .unwrap();
}

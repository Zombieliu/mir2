//! Real typed ledger construction and validation; no database, issuer or network.
use super::*;
use crate::config::{
    SharedGuildMember, SharedGuildRank, SharedGuildRecord, SharedGuildStoredItem,
    Stage5FriendIdentity,
};
use crate::monthly_card::{
    monthly_card_item_identity, MonthlyCardItemDelivery, MonthlyCardItemReceipt, MonthlyCardLedger,
    MONTHLY_CARD_DURATION_MS,
};
use crate::procurement::{
    actor_ref, digest, hex, Delivery, EquipmentLock, IntentV1, ProcurementLedger, SignedIntent,
    StoredQuote,
};
use crate::SimulationConfig;
use mir2_production::{
    StoneAction, StoneBinding, StoneCatalog, StoneCommand, StoneContext, StoneGrade,
    StoneLifecycle, StoneMintPreparation, StoneMintRequest, StoneOwner, StonePossession,
    StonePreparation, StoneRegistry, StoneSource, StoneSourceKind,
};
use std::collections::BTreeMap;

fn store() -> AccountStore {
    let mut store = AccountStore::new(SimulationConfig::default().default_character);
    store.accounts.get_mut("demo").unwrap().ban_reason = format!("{{\"unique_id\":{}}}", u64::MAX);
    store
}

fn carrier(uid: u64, template_name: &str, slot: u8) -> ItemState {
    let template = mir2_game_data::crystal_item_by_name(template_name).unwrap();
    let mut item = super::super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        slot,
    );
    item.unique_id = uid;
    item.description = format!("{{\"issuedThrough\":{}}}", u64::MAX);
    item
}

fn assert_history(store: &AccountStore, highest: u64, retained: &[u64], ignored: &[u64]) {
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Store(store)).unwrap(),
        highest
    );
    for uid in retained {
        assert!(
            historical_uid_conflicts(UidHistoryRoot::Store(store), &BTreeSet::from([*uid]))
                .unwrap(),
            "retained UID {uid}"
        );
    }
    for uid in ignored {
        assert!(
            !historical_uid_conflicts(UidHistoryRoot::Store(store), &BTreeSet::from([*uid]))
                .unwrap(),
            "non-UID selector/text {uid}"
        );
    }
}

fn retired_monthly_receipt(account: &str) -> (String, MonthlyCardItemReceipt) {
    let request_id = u64::MAX.to_string();
    let (key, uid) = monthly_card_item_identity(account, 0, &request_id, 0);
    (
        key,
        MonthlyCardItemReceipt {
            account_id: account.into(),
            character_index: 0,
            request_id,
            ordinal: 0,
            purchase_quantity: 1,
            unique_id: uid,
            credit_price: 100,
            delivery: MonthlyCardItemDelivery::Mail,
            issued_at_ms: 1,
            redeemed_at_ms: Some(2),
            credited_until_ms: Some(2 + MONTHLY_CARD_DURATION_MS),
        },
    )
}

#[test]
fn typed_uid_history_redeemed_monthly_receipt_retains_uid_and_checks_actual_account() {
    let mut store = store();
    let (key, receipt) = retired_monthly_receipt("demo");
    let uid = receipt.unique_id;
    let ledger = MonthlyCardLedger {
        expires_at_ms: receipt.credited_until_ms.unwrap(),
        codes: BTreeMap::new(),
        item_receipts: BTreeMap::from([(key, receipt)]),
    };
    ledger.validate().unwrap();
    // The consumed card has no remaining physical item in any character save.
    store.accounts.get_mut("demo").unwrap().monthly_card = Some(ledger);
    assert_history(&store, uid, &[uid], &[u64::MAX]);
    let (foreign_key, foreign_receipt) = retired_monthly_receipt("another-account");
    let ledger = store
        .accounts
        .get_mut("demo")
        .unwrap()
        .monthly_card
        .as_mut()
        .unwrap();
    ledger.item_receipts = BTreeMap::from([(foreign_key, foreign_receipt)]);
    ledger.validate().unwrap();
    assert!(historical_uid_floor(UidHistoryRoot::Store(&store)).is_err());
    let ledger = store
        .accounts
        .get_mut("demo")
        .unwrap()
        .monthly_card
        .as_mut()
        .unwrap();
    ledger.item_receipts.values_mut().next().unwrap().unique_id = u64::MAX;
    assert!(ledger.validate().is_err());
    assert!(historical_uid_floor(UidHistoryRoot::Store(&store)).is_err());
}

fn retired_procurement_ledger(store: &AccountStore) -> ProcurementLedger {
    // The same fixed non-production Ed25519 seed used by procurement's tests.
    let key = ed25519_consensus::SigningKey::from([7; 32]);
    let save = &store.accounts["demo"].saves[&0];
    let intent = IntentV1 {
        network_hash: [1; 32],
        package_id: [2; 32],
        dapp_storage_id: [3; 32],
        operation: 2,
        order_id: [4; 32],
        fill_id: [6; 32],
        buyer_ref: actor_ref("history-buyer", save),
        seller_ref: actor_ref("demo", save),
        template_id: 245,
        conditions_hash: digest(
            &mir2_protocol::procurement::EquipmentConditions::default()
                .canonical_bcs()
                .unwrap(),
        ),
        quantity: 1,
        price_gold: 500_000,
        expires_at_ms: 90_000,
        rule_version: 1,
        nonce: [8; 32],
        valid_until_ms: 50_000,
        actor_public_key: key.verification_key().to_bytes(),
    };
    let signed = SignedIntent {
        signature: key.sign(&intent.hash()).to_bytes().to_vec(),
        intent,
    };
    signed.verify().unwrap();
    let template = mir2_game_data::crystal_item_by_index(245).unwrap();
    let mut item = super::super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        0,
    );
    item.unique_id = 610_001;
    item.description = format!("{{\"issuedThrough\":{}}}", u64::MAX);
    assert_eq!(
        super::super::super::items::try_user_item_from_item_state(&item)
            .unwrap()
            .item_index,
        245
    );
    item.socket_slots = 2;
    item.socketed.push(carrier(610_002, "BronzeBell", 1));
    validate_committed_item_state_carrier(&item).unwrap();
    let original = serde_json::to_string(&item).unwrap();
    let lock_id = hex(&signed.intent.fill_id);
    let mut ledger = ProcurementLedger::default();
    ledger.locks.insert(
        lock_id.clone(),
        EquipmentLock {
            id: lock_id,
            order_id: hex(&signed.intent.order_id),
            buyer_account: "history-buyer".into(),
            seller_character: save.character.index,
            seller_name: save.character.name.clone(),
            signed: signed.clone(),
            original_item_json: original.clone(),
            item_uid: item.unique_id,
            instance_hash: digest(original.as_bytes()),
            fence: 1,
            released: true,
        },
    );
    // A losing/cancelled seller's released lock can retain its exact claimed
    // return delivery in the same account, even after live custody disappears.
    ledger.deliveries.insert(
        "retired-delivery".into(),
        Delivery {
            id: "retired-delivery".into(),
            order_id: hex(&signed.intent.order_id),
            character_index: save.character.index,
            character_name: save.character.name.clone(),
            gold: 0,
            original_item_json: Some(original),
            item_uid: Some(item.unique_id),
            claimed: true,
            claim_revision: Some(1),
        },
    );
    // A quote holds a client-selected reference, not locked or minted custody.
    ledger.quotes.insert(
        "unreserved-selector".into(),
        StoredQuote {
            public: mir2_protocol::procurement::ProcurementQuote {
                quote_id: "unreserved-selector".into(),
                operation: "fill".into(),
                intent: Some(signed.intent.to_wire()),
                canonical_intent_hex: Some(hex(&signed.intent.bcs_bytes())),
                conditions: Default::default(),
                item_uid: Some(u64::MAX.to_string()),
                item_description: Some(format!("{{\"uid\":{}}}", u64::MAX)),
                service_fee: Default::default(),
                quota_package_id: None,
            },
            intent: signed.intent,
            buyer_account: "history-buyer".into(),
            item_uid: Some(u64::MAX),
            character_index: save.character.index,
            character_name: save.character.name.clone(),
            request_id: u64::MAX.to_string(),
            rule: None,
        },
    );
    ledger.validate().unwrap();
    ledger
}

#[test]
fn typed_uid_history_retired_procurement_keeps_original_tree_but_ignores_quote_selector() {
    let mut store = store();
    let ledger = retired_procurement_ledger(&store);
    let account = store.accounts.get_mut("demo").unwrap();
    account.procurement = Some(ledger);
    account.billing = Some(Default::default());
    crate::procurement::validate_account(account).unwrap();
    crate::procurement::validate_store(&store).unwrap();
    assert_history(&store, 610_002, &[610_001, 610_002], &[u64::MAX]);
    let account = store.accounts.get_mut("demo").unwrap();
    let lock = account
        .procurement
        .as_mut()
        .unwrap()
        .locks
        .values_mut()
        .next()
        .unwrap();
    lock.item_uid += 1;
    account.procurement.as_ref().unwrap().validate().unwrap();
    assert!(historical_uid_floor(UidHistoryRoot::Store(&store)).is_err());
    let ledger = store
        .accounts
        .get_mut("demo")
        .unwrap()
        .procurement
        .as_mut()
        .unwrap();
    ledger.locks.values_mut().next().unwrap().item_uid -= 1;
    ledger
        .deliveries
        .values_mut()
        .next()
        .unwrap()
        .original_item_json = Some("{broken".into());
    ledger.validate().unwrap();
    assert!(historical_uid_floor(UidHistoryRoot::Store(&store)).is_err());
}

#[test]
fn typed_uid_history_shared_guild_storage_keeps_full_tree_and_rejects_broken_carrier() {
    let mut store = store();
    let character = &store.accounts["demo"].characters[0];
    let mut root = carrier(620_001, "BronzeHelmet", 0);
    root.socket_slots = 2;
    root.socketed.push(carrier(620_002, "BronzeBell", 1));
    validate_committed_item_state_carrier(&root).unwrap();
    let id = "0123456789abcdef0123456789abcdef".to_string();
    store.shared_guilds.insert(
        id.clone(),
        SharedGuildRecord {
            id: id.clone(),
            name: "HistoryGuild".into(),
            revision: 1,
            level: 0,
            experience: 0,
            spare_points: 0,
            gold: 0,
            ranks: vec![SharedGuildRank {
                index: 0,
                name: "Leader".into(),
                options: 255,
            }],
            members: vec![SharedGuildMember {
                membership_epoch: 0,
                identity: Stage5FriendIdentity {
                    account_id: "demo".into(),
                    character_index: character.index,
                },
                name: character.name.clone(),
                rank_index: 0,
            }],
            notice: vec![format!("{{\"unique_id\":{}}}", u64::MAX)],
            storage: BTreeMap::from([(
                0,
                SharedGuildStoredItem {
                    item_state_json: serde_json::to_string(&root).unwrap(),
                    depositor_character_index: character.index,
                },
            )]),
            buffs: BTreeMap::new(),
            last_buff_tick_ms: 0,
            experience_receipts: BTreeSet::new(),
            experience_receipt_payloads: BTreeMap::new(),
            active_wars: BTreeMap::new(),
        },
    );
    assert_history(&store, 620_002, &[620_001, 620_002], &[u64::MAX]);
    store
        .shared_guilds
        .get_mut(&id)
        .unwrap()
        .storage
        .get_mut(&0)
        .unwrap()
        .item_state_json = "{broken".into();
    assert!(historical_uid_floor(UidHistoryRoot::Store(&store)).is_err());
}

#[test]
fn typed_uid_history_cut_stone_tombstone_keeps_carrier_not_serial_or_request_text() {
    let mut store = store();
    let owner = StoneOwner {
        account_id: "demo".into(),
        character_incarnation: "history-actor".into(),
    };
    let request = StoneMintRequest {
        owner: owner.clone(),
        request_id: u64::MAX.to_string(),
        serial: u64::MAX - 1,
        carrier_uid: 630_001,
        source_template_id: 824,
        raw_template_id: 980_001,
        raw_weight: 4,
        grade: StoneGrade::Rough,
        binding: StoneBinding::default(),
        source: StoneSource {
            kind: StoneSourceKind::MiningWorkshop,
            source_version: "mining-workshop-v1".into(),
            event_id: u64::MAX.to_string(),
        },
    };
    let StoneMintPreparation::Change(minted) = StoneRegistry::default()
        .prepare_mint(&StoneCatalog::v1(), &request, 0)
        .unwrap()
    else {
        panic!("fresh test mint replayed")
    };
    let possessions = [StonePossession {
        holder: owner.clone(),
        serial: request.serial,
        carrier_uid: request.carrier_uid,
        raw_template_id: request.raw_template_id,
        raw_weight: request.raw_weight,
        count: 1,
        bag_slot: 0,
        binding: request.binding,
        eligible: true,
        locked: false,
    }];
    let context = StoneContext {
        owner: &owner,
        authenticated: true,
        inventory_revision: 1,
        gold: 100_000,
        possessions: &possessions,
        bag_capacity: 40,
        free_bag_slots: 39,
        current_weight: 4,
        max_weight: 1000,
    };
    let command = StoneCommand {
        request_id: "history-cut".into(),
        stone_serial: request.serial,
        expected_revision: 0,
        action: StoneAction::Cut,
    };
    let StonePreparation::Change(cut) = minted.next_registry.prepare(&context, &command).unwrap()
    else {
        panic!("fresh test cut replayed")
    };
    assert_eq!(
        cut.next_registry
            .record(request.serial)
            .unwrap()
            .lifecycle(),
        StoneLifecycle::Cut
    );
    cut.next_registry.validate().unwrap();
    store.sealed_stones =
        StoneRegistry::restore(&cut.next_registry.checkpoint_json().unwrap()).unwrap();
    assert_history(
        &store,
        request.carrier_uid,
        &[request.carrier_uid],
        &[u64::MAX, request.serial],
    );
}

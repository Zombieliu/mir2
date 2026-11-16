//! Schema-root history tests. No allocator, network, database, or guessed JSON.
use super::*;
use crate::config::{
    SharedHeroRecord, SharedHeroState, Stage5AuctionListing, Stage5ItemRentalRecordSnapshot,
    Stage5MailAttachmentOrigin, Stage5MailMessage,
};
use crate::npc_purchase_journal::{NpcPurchaseIntent, NpcPurchaseJournal, NpcPurchaseOperation};
use crate::runtime::{
    NpcPurchaseCurrency, NpcPurchaseRejection, NpcPurchaseRequest, NpcPurchaseSource,
};
use crate::{EquipmentSlot, SimulationConfig};

fn item(uid: u64, slot: u8) -> ItemState {
    let template = mir2_game_data::crystal_item_by_name("BronzeHelmet").unwrap();
    let mut item = super::super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        slot,
    );
    item.unique_id = uid;
    item
}

fn save() -> CharacterSaveRecord {
    CharacterSaveRecord::new(SimulationConfig::default().default_character)
}

fn mail(body: &str) -> Stage5MailMessage {
    Stage5MailMessage {
        id: 1,
        delivery_nonce: String::new(),
        from: "Player".into(),
        to: "Player".into(),
        subject: "Ordinary mail".into(),
        body: body.into(),
        gold: 0,
        items: Vec::new(),
        item_states_json: Vec::new(),
        attachment_origin: Stage5MailAttachmentOrigin::LegacyUnknown,
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    }
}

fn systems_floor(systems: &Stage5SystemsState) -> Result<u64, String> {
    historical_uid_floor(UidHistoryRoot::Systems {
        account_id: "demo",
        character_index: 0,
        systems,
    })
}

#[test]
fn typed_uid_history_ordinary_player_text_never_becomes_a_uid_carrier() {
    for text in [
        r#"{"unique_id":18446744073709551615}"#,
        r#"[{"uniqueId":"0002"}]"#,
        r#"{"slots":[{"incomingUniqueId":-1}]}"#,
        r#"{"custody":"{\"unique_id\":18446744073709551615}"}"#,
        "[broken JSON",
    ] {
        let mut store = AccountStore::new(SimulationConfig::default().default_character);
        let account = store.accounts.get_mut("demo").unwrap();
        account.password = text.into();
        account.storage_password = text.into();
        account.ban_reason = text.into();
        let save = account.saves.get_mut(&0).unwrap();
        save.character.name = text.into();
        save.map_title = text.into();
        save.npc_saved_values_json.push(text.into());
        let mut carrier = item(123, 0);
        carrier.name = text.into();
        carrier.description = text.into();
        save.inventory_items_json = vec![serde_json::to_string(&carrier).unwrap()];
        let mut systems = Stage5SystemsState::default();
        let mut delivery = mail(text);
        delivery.from = text.into();
        delivery.to = text.into();
        delivery.subject = text.into();
        delivery.items.push(text.into());
        systems.mail.push(delivery);
        save.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
        assert_eq!(
            historical_uid_floor(UidHistoryRoot::Store(&store)).unwrap(),
            123
        );
        assert!(!historical_uid_conflicts(
            UidHistoryRoot::Store(&store),
            &BTreeSet::from([u64::MAX])
        )
        .unwrap());
    }
}

#[test]
fn typed_uid_history_rejects_every_malformed_declared_save_asset_field() {
    let mut cases = Vec::new();
    for field in 0..11 {
        let mut candidate = save();
        let malformed = "[not-an-asset]".to_string();
        match field {
            0 => candidate.inventory_items_json.push(malformed),
            1 => candidate.belt_items_json.push(malformed),
            2 => candidate.storage_items_json.push(malformed),
            3 => candidate.hero_inventory_items_json.push(malformed),
            4 => candidate.hero_equipment_items_json.push(malformed),
            5 => candidate.equipment_items_json.push(malformed),
            6 => candidate.npc_buy_back_items_json.push(malformed),
            7 => candidate.npc_used_goods_items_json.push(malformed),
            8 => candidate.item_rental_records_json.push(malformed),
            9 => candidate.stage5_systems_json = Some(malformed),
            10 => candidate
                .inventory_items_json
                .push(r#"{"unique_id":18446744073709551615}"#.into()),
            _ => unreachable!(),
        }
        cases.push(candidate);
    }
    for (field, candidate) in cases.into_iter().enumerate() {
        assert!(
            historical_uid_floor(UidHistoryRoot::Save {
                account_id: "demo",
                save: &candidate
            })
            .is_err(),
            "field {field}"
        );
    }
}

#[test]
fn typed_uid_history_rejects_malformed_terminal_system_asset_payloads() {
    for field in 0..5 {
        let mut systems = Stage5SystemsState::default();
        let malformed = "[not-an-asset]".to_string();
        match field {
            0 => {
                let mut delivery = mail("visible text");
                delivery.claimed = true;
                delivery.deleted = true;
                delivery.item_states_json.push(malformed);
                systems.mail.push(delivery);
            }
            1 => {
                systems.guild.storage_item_states.insert(0, malformed);
            }
            2 => systems.auction.push(Stage5AuctionListing {
                item_state_json: Some(malformed),
                id: 1,
                seller: "demo".into(),
                item_key: "BronzeHelmet".into(),
                price: 1,
                currency: Default::default(),
                sold: true,
                cancelled: false,
                expired: true,
            }),
            3 => {
                systems.refine.item_states.insert(0, malformed);
            }
            4 => systems.refine.oven_item_state_json = Some(malformed),
            _ => unreachable!(),
        }
        assert!(systems_floor(&systems).is_err(), "field {field}");
    }
}

#[test]
fn typed_uid_history_reads_raw_nested_ids_and_allows_retained_mirrors() {
    let mut root = item(100, 0);
    let template = mir2_game_data::crystal_item_by_name("BronzeBell").unwrap();
    let mut child = super::super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        1,
    );
    child.unique_id = u64::MAX - 1;
    root.socket_slots = 2;
    root.socketed.push(child);
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Item(&root)).unwrap(),
        u64::MAX - 1
    );
    // The legacy compatibility projection intentionally does not emit hand-built
    // sidecar-less socket children. Construct an exact complete wire carrier
    // from the original real root and child, rather than losing that child.
    let mut wire =
        super::super::super::items::try_user_item_from_item_state(&item(100, 0)).unwrap();
    let child_wire =
        super::super::super::items::try_user_item_from_item_state(&root.socketed[0]).unwrap();
    wire.slots = vec![None, Some(child_wire)];
    let exact = super::super::super::items::try_item_state_from_user_item(root, &wire).unwrap();
    assert_eq!(exact.socketed.len(), 1);
    assert_eq!(exact.socketed[0].unique_id, u64::MAX - 1);
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Item(&exact)).unwrap(),
        u64::MAX - 1
    );
    assert!(historical_uid_conflicts(
        UidHistoryRoot::Item(&exact),
        &BTreeSet::from([u64::MAX - 1])
    )
    .unwrap());
    assert!(
        !historical_uid_conflicts(UidHistoryRoot::Item(&exact), &BTreeSet::from([u64::MAX]))
            .unwrap()
    );
    let encoded = serde_json::to_string(&exact).unwrap();
    let mut systems = Stage5SystemsState::default();
    let mut delivery = mail("retained mirrors");
    delivery.item_states_json = vec![encoded.clone(), encoded];
    systems.mail.push(delivery);
    assert_eq!(systems_floor(&systems).unwrap(), u64::MAX - 1);
}

#[test]
fn typed_uid_history_removed_child_retains_captured_slot_identity() {
    let mut wire =
        super::super::super::items::try_user_item_from_item_state(&item(110, 0)).unwrap();
    let template = mir2_game_data::crystal_item_by_name("BronzeBell").unwrap();
    let mut child = super::super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        1,
    );
    child.unique_id = u64::MAX - 1;
    wire.slots = vec![
        None,
        Some(super::super::super::items::try_user_item_from_item_state(&child).unwrap()),
    ];
    let mut retained =
        super::super::super::items::try_item_state_from_user_item(item(110, 0), &wire).unwrap();
    retained.socketed.clear();
    let projection = super::super::super::items::try_user_item_from_item_state(&retained).unwrap();
    assert!(projection.slots.iter().all(Option::is_none));
    assert!(retained
        .user_item_metadata
        .as_ref()
        .unwrap()
        .slots
        .is_empty());
    assert_eq!(
        retained
            .user_item_metadata
            .as_ref()
            .unwrap()
            .captured_socket_positions
            .as_ref()
            .unwrap()[1]
            .as_ref()
            .unwrap()
            .unique_id,
        u64::MAX - 1
    );
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Item(&retained)).unwrap(),
        u64::MAX - 1
    );
    assert!(historical_uid_conflicts(
        UidHistoryRoot::Item(&retained),
        &BTreeSet::from([u64::MAX - 1])
    )
    .unwrap());
    assert!(!historical_uid_conflicts(
        UidHistoryRoot::Item(&retained),
        &BTreeSet::from([u64::MAX])
    )
    .unwrap());
    let equipment = super::super::super::equipment::equipment_state_from_item_state(
        &retained,
        EquipmentSlot::Helmet,
    );
    let mut candidate = save();
    candidate.equipment_items_json = vec![serde_json::to_string(&equipment).unwrap()];
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Save {
            account_id: "demo",
            save: &candidate
        })
        .unwrap(),
        u64::MAX - 1
    );
    retained
        .user_item_metadata
        .as_mut()
        .unwrap()
        .captured_socket_positions
        .as_mut()
        .unwrap()[1]
        .as_mut()
        .unwrap()
        .item_index = i32::MAX;
    assert!(historical_uid_floor(UidHistoryRoot::Item(&retained)).is_err());
}

#[test]
fn typed_uid_history_wire_fresh_zero_siblings_use_only_original_real_ids() {
    let mut raw = super::super::super::items::try_user_item_from_item_state(&item(0, 0)).unwrap();
    raw.unique_id = 0;
    let mut child = raw.clone();
    child.unique_id = 0;
    raw.slots = vec![Some(child.clone()), None, Some(child)];
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::WireItem(&raw)).unwrap(),
        0
    );
    raw.slots[0].as_mut().unwrap().unique_id = 70_001;
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::WireItem(&raw)).unwrap(),
        70_001
    );
    assert_eq!(raw.unique_id, 0);
    assert_eq!(raw.slots[2].as_ref().unwrap().unique_id, 0);
    let mut damaged = raw.clone();
    damaged.slots[2].as_mut().unwrap().count = 0;
    assert!(historical_uid_floor(UidHistoryRoot::WireItem(&damaged)).is_err());
}

#[test]
fn typed_uid_history_legacy_equipment_does_not_invent_a_grid_identity() {
    let mut equipment = super::super::super::equipment::equipment_state_from_item_state(
        &item(0, 0),
        EquipmentSlot::Helmet,
    );
    equipment.user_item_unique_id = None;
    let mut candidate = save();
    candidate.equipment_items_json = vec![serde_json::to_string(&equipment).unwrap()];
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Save {
            account_id: "demo",
            save: &candidate
        })
        .unwrap(),
        0
    );
    equipment.user_item_unique_id = Some(u64::MAX - 2);
    candidate.equipment_items_json = vec![serde_json::to_string(&equipment).unwrap()];
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Save {
            account_id: "demo",
            save: &candidate
        })
        .unwrap(),
        u64::MAX - 2
    );
}

#[test]
fn typed_uid_history_terminal_system_custody_and_exact_reference_ids_are_retained() {
    let mut systems = Stage5SystemsState::default();
    let mut delivery = mail("claimed");
    delivery.claimed = true;
    delivery.deleted = true;
    delivery.item_states_json = vec![serde_json::to_string(&item(301, 0)).unwrap()];
    systems.mail.push(delivery);
    systems
        .guild
        .storage_item_states
        .insert(0, serde_json::to_string(&item(302, 0)).unwrap());
    systems.auction.push(Stage5AuctionListing {
        item_state_json: Some(serde_json::to_string(&item(303, 0)).unwrap()),
        id: 1,
        seller: "demo".into(),
        item_key: "BronzeHelmet".into(),
        price: 1,
        currency: Default::default(),
        sold: true,
        cancelled: false,
        expired: true,
    });
    systems
        .refine
        .item_states
        .insert(0, serde_json::to_string(&item(304, 0)).unwrap());
    systems.refine.oven_item_state_json = Some(serde_json::to_string(&item(305, 0)).unwrap());
    systems.refine.pending_unique_id = 306;
    systems.trade = Some(
        serde_json::from_value(serde_json::json!({
            "partner":"demo", "offeredItems":[], "offeredGold":0,
            "accepted":false, "completed":true, "offeredUniqueIds":{"0":307}
        }))
        .unwrap(),
    );
    systems
        .item_rental
        .rented_items
        .push(Stage5ItemRentalRecordSnapshot {
            item_id: 308,
            item_name: "BronzeHelmet".into(),
            renting_player_name: "other".into(),
            item_return_date_binary_datetime: 0,
        });
    let mut found = BTreeSet::new();
    visit_historical_item_uids(
        UidHistoryRoot::Systems {
            account_id: "demo",
            character_index: 0,
            systems: &systems,
        },
        &mut |uid| {
            found.insert(uid);
        },
    )
    .unwrap();
    assert_eq!(found, (301..=308).collect());
}

#[test]
fn typed_uid_history_npc_journal_counts_only_validated_committed_server_uid() {
    let mut candidate = save();
    candidate.revision = 2;
    let mut journal = NpcPurchaseJournal::new(
        "demo",
        candidate.character.index,
        &candidate.character.name,
        [1; 32],
    )
    .unwrap();
    let operation = |sequence| NpcPurchaseOperation {
        actor: [1; 32],
        request_scope: [2; 32],
        sequence,
        intent: NpcPurchaseIntent {
            request: NpcPurchaseRequest {
                item_index: u64::MAX,
                count: 1,
                panel_type: 0,
            },
            currency: NpcPurchaseCurrency::Gold,
            source: NpcPurchaseSource::Trade,
            service_catalog_proof: [3; 32],
        },
    };
    let rejected = operation(1);
    journal
        .record(
            rejected,
            1,
            NpcPurchaseProcessingOutcome::Rejected {
                request: rejected.intent.request,
                reason: NpcPurchaseRejection::InsufficientCurrency,
            },
        )
        .unwrap();
    candidate.npc_purchase_journal = Some(journal.clone());
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Save {
            account_id: "demo",
            save: &candidate
        })
        .unwrap(),
        0
    );
    let committed = operation(2);
    journal
        .record(
            committed,
            2,
            NpcPurchaseProcessingOutcome::Committed {
                request: committed.intent.request,
                currency: committed.intent.currency,
                source: committed.intent.source,
                charged: 1,
                admitted_count: 1,
                incoming_unique_id: u64::MAX - 1,
            },
        )
        .unwrap();
    candidate.npc_purchase_journal = Some(journal);
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Save {
            account_id: "demo",
            save: &candidate
        })
        .unwrap(),
        u64::MAX - 1
    );
    assert!(historical_uid_floor(UidHistoryRoot::Save {
        account_id: "other",
        save: &candidate
    })
    .is_err());
    candidate
        .npc_purchase_journal
        .as_mut()
        .unwrap()
        .protocol_version += 1;
    assert!(historical_uid_floor(UidHistoryRoot::Save {
        account_id: "demo",
        save: &candidate
    })
    .is_err());
}

#[test]
fn typed_uid_history_real_npc_resale_and_rental_save_carriers_are_included() {
    let mut candidate = save();
    let wire = super::super::super::items::try_user_item_from_item_state(&item(901, 0)).unwrap();
    candidate.npc_buy_back_items_json.push(
        serde_json::to_string(&NpcBuyBackState {
            script_key: "npc".into(),
            player_name: candidate.character.name.clone(),
            items: vec![super::super::super::npc::NpcBuyBackItemState {
                item: wire.clone(),
                expires_at_binary_datetime: 0,
            }],
        })
        .unwrap(),
    );
    let mut used = wire;
    used.unique_id = 902;
    candidate.npc_used_goods_items_json.push(
        serde_json::to_string(&NpcUsedGoodsState {
            script_key: "npc".into(),
            items: vec![used],
        })
        .unwrap(),
    );
    candidate.item_rental_records_json.push(
        serde_json::to_string(&ItemRentalRecordState {
            item_id: 903,
            ..Default::default()
        })
        .unwrap(),
    );
    let mut found = BTreeSet::new();
    visit_historical_item_uids(
        UidHistoryRoot::Save {
            account_id: "demo",
            save: &candidate,
        },
        &mut |uid| {
            found.insert(uid);
        },
    )
    .unwrap();
    assert_eq!(found, BTreeSet::from([901, 902, 903]));
}

#[test]
fn typed_uid_history_released_hero_keeps_tree_history_and_sealed_carrier_reference() {
    let mut store = AccountStore::new(SimulationConfig::default().default_character);
    let character = &store.accounts["demo"].characters[0];
    let state = SharedHeroState {
        name: "retained".into(),
        class: character.class,
        gender: character.gender,
        hair: 0,
        grade: 0,
        level: 1,
        experience: 0,
        inventory_items_json: vec![serde_json::to_string(&item(701, 0)).unwrap()],
        equipment_items_json: vec![serde_json::to_string(&item(702, 0)).unwrap()],
        inventory_capacity: 40,
        inventory_legacy_40: true,
        vitals: None,
        magics: Vec::new(),
        seal_count: 1,
        auto_pot: false,
        auto_hp_percent: 0,
        auto_mp_percent: 0,
        hp_item_index: 0,
        mp_item_index: 0,
    };
    store.shared_heroes.insert(
        1,
        SharedHeroRecord {
            id: 1,
            revision: 1,
            custody: SharedHeroCustody::Released,
            state,
        },
    );
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Store(&store)).unwrap(),
        702
    );
    store.shared_heroes.get_mut(&1).unwrap().custody =
        SharedHeroCustody::Sealed { carrier_uid: 703 };
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Store(&store)).unwrap(),
        703
    );
    store
        .shared_heroes
        .get_mut(&1)
        .unwrap()
        .state
        .inventory_items_json
        .push("{broken".into());
    assert!(historical_uid_floor(UidHistoryRoot::Store(&store)).is_err());
}

#[test]
fn typed_uid_history_unknown_store_schema_is_not_silently_certified() {
    let mut store = AccountStore::new(SimulationConfig::default().default_character);
    for version in [0, crate::billing::BILLING_SCHEMA_VERSION + 1] {
        store.schema_version = version;
        assert!(historical_uid_floor(UidHistoryRoot::Store(&store)).is_err());
    }
    store.schema_version = 1;
    assert_eq!(
        historical_uid_floor(UidHistoryRoot::Store(&store)).unwrap(),
        0
    );
}

use super::*;

fn mail() -> Stage5MailMessage {
    Stage5MailMessage {
        id: 1,
        delivery_nonce: "test-transfer-1".into(),
        from: "Sender".into(),
        to: "Recipient".into(),
        subject: "Original parcel".into(),
        body: r#"{"unique_id":18446744073709551615}"#.into(),
        gold: 17,
        items: vec!["helmet".into()],
        item_states_json: vec![r#"{"unique_id":10,"future_extension":{"x":1}}"#.into()],
        attachment_origin: Stage5MailAttachmentOrigin::ExistingCustody,
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    }
}

#[test]
fn mail_transfer_receipt_allows_only_display_status_mutations() {
    let before = mail();
    let mut after = before.clone();
    after.id = 99;
    after.opened = true;
    after.locked = true;
    assert!(guard_existing(Some(&before), Some(&after), &payload(&before)).is_ok());
    after.locked = false;
    assert!(guard_existing(Some(&before), Some(&after), &payload(&before)).is_ok());
    for field in ["from", "to", "subject", "body", "deliveryNonce"] {
        let mut altered = serde_json::to_value(&before).unwrap();
        altered[field] = json!("changed");
        let altered = serde_json::from_value(altered).unwrap();
        assert!(guard_existing(Some(&before), Some(&altered), &payload(&before)).is_err());
    }
}

#[test]
fn mail_transfer_receipt_freezes_raw_unknown_item_fields() {
    let before = mail();
    let mut after = before.clone();
    after.item_states_json[0] = r#"{"unique_id":10,"future_extension":{"x":2}}"#.into();
    assert!(guard_existing(Some(&before), Some(&after), &payload(&before)).is_err());
    assert_ne!(
        digest(&payload(&before)).unwrap(),
        digest(&payload(&after)).unwrap()
    );
}

#[test]
fn mail_transfer_receipt_accepts_legitimate_terminal_states_and_purge() {
    let before = mail();
    let mut deleted = before.clone();
    deleted.deleted = true; // Existing DeleteMail retains an unclaimed parcel.
    assert!(guard_existing(Some(&before), Some(&deleted), &payload(&before)).is_ok());
    assert!(guard_existing(Some(&deleted), None, &payload(&before)).is_ok());
    for claimed in [false, true] {
        let mut terminal = before.clone();
        terminal.claimed = claimed;
        terminal.deleted = !claimed;
        terminal.gold = 0;
        terminal.items.clear();
        terminal.item_states_json.clear();
        assert!(guard_existing(Some(&before), Some(&terminal), &payload(&before)).is_ok());
        assert!(guard_existing(Some(&terminal), None, &payload(&before)).is_ok());
        let mut resurrected = before.clone();
        resurrected.claimed = terminal.claimed;
        resurrected.deleted = terminal.deleted;
        assert!(guard_existing(Some(&terminal), Some(&resurrected), &payload(&before)).is_err());
        assert!(guard_existing(Some(&terminal), Some(&before), &payload(&before)).is_err());
    }
}

#[test]
fn mail_transfer_receipt_rejects_active_disappearance_and_revival() {
    let before = mail();
    assert!(guard_existing(Some(&before), None, &payload(&before)).is_err());
    assert!(guard_existing(None, Some(&before), &payload(&before)).is_err());
    assert!(guard_existing(None, None, &payload(&before)).is_ok());
    let mut partial = before.clone();
    partial.claimed = true;
    partial.item_states_json.clear();
    assert!(guard_existing(Some(&before), Some(&partial), &payload(&before)).is_err());
}

#[test]
fn mail_transfer_old_nonce_cannot_upgrade_origin_or_add_attachments() {
    let after = mail();
    for origin in [
        Stage5MailAttachmentOrigin::LegacyUnknown,
        Stage5MailAttachmentOrigin::FreshGrant,
    ] {
        let mut before = after.clone();
        before.attachment_origin = origin;
        assert!(needs_new_receipt(Some(&before), &after).is_err());
    }
    let mut empty = after.clone();
    empty.item_states_json.clear();
    empty.items.clear();
    assert!(needs_new_receipt(Some(&empty), &after).is_err());
    assert!(!needs_new_receipt(Some(&after), &after).unwrap());
    assert!(needs_new_receipt(None, &after).unwrap());
    let mut downgrade = after.clone();
    downgrade.attachment_origin = Stage5MailAttachmentOrigin::FreshGrant;
    assert!(needs_new_receipt(Some(&after), &downgrade).is_err());
}

#[test]
fn mail_transfer_unrecorded_existing_nonce_only_keeps_original_or_terminal_clear() {
    let before = mail();
    let mut after = before.clone();
    after.opened = true;
    assert!(!needs_new_receipt(Some(&before), &after).unwrap());
    after.deleted = true;
    after.gold = 0;
    after.items.clear();
    after.item_states_json.clear();
    assert!(!needs_new_receipt(Some(&before), &after).unwrap());
    assert!(needs_new_receipt(Some(&after), &before).is_err());
}

#[test]
fn mail_transfer_unrelated_monthly_and_legacy_zero_bag_items_are_not_transfer_candidates() {
    let monthly = 0x001d_0000_0000_0001u64;
    assert!(crate::monthly_card::monthly_card_item_uid_is_reserved(
        monthly
    ));
    let identity = bag_identity(&json!({"unique_id":monthly}).to_string()).unwrap();
    assert_eq!(identity.unique_id, monthly);
    assert_eq!(bag_live_uids(&identity).unwrap(), BTreeSet::from([monthly]));
    let legacy = bag_identity(r#"{"key":"old","unique_id":0}"#).unwrap();
    assert_eq!(legacy.unique_id, 0);
    assert!(bag_live_uids(&legacy).unwrap().is_empty());
}

#[test]
fn mail_transfer_remaining_bag_projection_includes_live_children_not_retired_or_text() {
    let identity = bag_identity(
        &json!({
            "unique_id":10,
            "description":"{\"unique_id\":18446744073709551615}",
            "socketed":[{"unique_id":11,"socketed":[{"unique_id":12}]}],
            "user_item_metadata":{
                "slots":[null,{"unique_id":13,"slots":[{"unique_id":14}]}],
                "captured_socket_positions":[{"unique_id":u64::MAX,"item_index":1}]
            }
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        bag_live_uids(&identity).unwrap(),
        BTreeSet::from([10, 11, 12])
    );
    assert!(!bag_live_uids(&identity)
        .unwrap()
        .is_disjoint(&BTreeSet::from([12])));
}

#[test]
fn mail_transfer_remaining_bag_projection_bounds_structural_identity() {
    assert!(bag_identity(r#"{"unique_id":"10"}"#).is_err());
    let mut too_deep = json!({"unique_id":10});
    for _ in 0..9 {
        too_deep = json!({"unique_id":11,"socketed":[too_deep]});
    }
    assert!(bag_live_uids(&bag_identity(&too_deep.to_string()).unwrap()).is_err());
    let too_many = json!({"unique_id":10,"socketed":vec![json!({"unique_id":11});65]});
    assert!(bag_live_uids(&bag_identity(&too_many.to_string()).unwrap()).is_err());
}

fn planned_send() -> AccountStoreMutationPlan {
    use super::super::{AccountStore, AccountStoreMutationScope, CharacterRecord};
    let sender = CharacterRecord {
        index: 1,
        name: "Sender".into(),
        level: 1,
        class: mir2_protocol::MirClass::Warrior,
        gender: mir2_protocol::MirGender::Male,
    };
    let recipient = CharacterRecord {
        index: 2,
        name: "Recipient".into(),
        ..sender.clone()
    };
    let template = mir2_game_data::crystal_item_by_name("BronzeHelmet").unwrap();
    let raw = json!({"key":format!("crystal-item-{}",template.item_index),
        "name":template.name,"icon":template.image,"slot":0,"unique_id":10,
        "container":"bag1","quantity":1,"description":"opaque original",
        "durability_current":null,"durability_max":null,"weight":1,"equip_slot":null,
        "attack":0,"defence":0,"heal_hp":0,"heal_mp":0,
        "future_extension":{"retained":true}});
    let mut before = AccountStore::new(sender.clone());
    before.accounts.clear();
    before
        .accounts
        .insert("sender".into(), AccountRecord::new(sender));
    before
        .accounts
        .insert("recipient".into(), AccountRecord::new(recipient));
    let source = before
        .accounts
        .get_mut("sender")
        .unwrap()
        .saves
        .get_mut(&1)
        .unwrap();
    source.inventory_items_json = vec![raw.to_string()];
    // Both are unrelated persisted bag roots. They must never be run through
    // the selected ordinary attachment manifest just to send the helmet.
    let mut old = raw.clone();
    old["unique_id"] = json!(0);
    old["slot"] = json!(1);
    source.inventory_items_json.push(old.to_string());
    let mut card = raw.clone();
    card["key"] = json!(format!(
        "crystal-item-{}",
        mir2_game_data::BILLING_MONTHLY_CARD_ITEM_INDEX
    ));
    card["unique_id"] = json!(0x001d_0000_0000_0001u64);
    card["slot"] = json!(2);
    source.inventory_items_json.push(card.to_string());
    for (account, index) in [("sender", 1), ("recipient", 2)] {
        before.source_account_versions.insert(account.into(), 1);
        before
            .source_save_versions
            .entry(account.into())
            .or_default()
            .insert(index, 1);
    }
    let mut after = before.clone();
    let source = after
        .accounts
        .get_mut("sender")
        .unwrap()
        .saves
        .get_mut(&1)
        .unwrap();
    let transferred = source.inventory_items_json.remove(0);
    source.revision += 1;
    let mut delivery = mail();
    delivery.items = vec![raw["key"].as_str().unwrap().into()];
    delivery.item_states_json = vec![transferred];
    let target = after
        .accounts
        .get_mut("recipient")
        .unwrap()
        .saves
        .get_mut(&2)
        .unwrap();
    target.revision += 1;
    target.stage5_systems_json = Some(
        serde_json::to_string(&Stage5SystemsState {
            mail: vec![delivery],
            ..Default::default()
        })
        .unwrap(),
    );
    super::super::build_account_store_mutation_plan(
        &before,
        &after,
        AccountStoreMutationScope::Accounts(&["sender".into(), "recipient".into()]),
        false,
    )
}

#[test]
fn mail_transfer_send_plan_binds_exact_removed_source_with_unrelated_monthly_and_legacy_items() {
    let plan = planned_send();
    let receipts = new_receipts(&plan).unwrap();
    assert_eq!(receipts.len(), 1);
    let binding = &receipts[0].binding;
    assert_eq!(binding["rootUids"], json!(["10"]));
    assert_eq!(binding["liveUids"], json!(["10"]));
    assert_eq!(binding["sender"]["accountId"], json!("sender"));
    assert_eq!(binding["recipient"]["accountId"], json!("recipient"));
    assert!(binding["originalSourceItemJson"][0]
        .as_str()
        .unwrap()
        .contains("future_extension"));
    assert!(carries_source(&plan));
}

#[test]
fn mail_transfer_send_plan_rejects_retained_copy_and_unknown_field_replacement() {
    let mut retained = planned_send();
    let original = retained.accounts["sender"]
        .original_account
        .as_ref()
        .unwrap()
        .saves[&1]
        .inventory_items_json[0]
        .clone();
    retained
        .accounts
        .get_mut("sender")
        .unwrap()
        .desired_account
        .as_mut()
        .unwrap()
        .saves
        .get_mut(&1)
        .unwrap()
        .inventory_items_json
        .push(original);
    assert!(new_receipts(&retained).is_err());
    let mut altered = planned_send();
    let target = altered
        .accounts
        .get_mut("recipient")
        .unwrap()
        .desired_account
        .as_mut()
        .unwrap()
        .saves
        .get_mut(&2)
        .unwrap();
    let mut state = systems(target).unwrap();
    let mut item: Value = serde_json::from_str(&state.mail[0].item_states_json[0]).unwrap();
    item["future_extension"]["retained"] = json!(false);
    state.mail[0].item_states_json[0] = item.to_string();
    target.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
    assert!(new_receipts(&altered).is_err());
}

fn add_retained_socket_carrier(
    plan: &mut AccountStoreMutationPlan,
    captured: bool,
    hydrated: bool,
) -> String {
    let original = &plan.accounts["sender"]
        .original_account
        .as_ref()
        .unwrap()
        .saves[&1]
        .inventory_items_json[0];
    let mut carrier: Value = serde_json::from_str(original).unwrap();
    let index = mir2_game_data::crystal_item_by_name("BronzeHelmet")
        .unwrap()
        .item_index;
    let old_wire = json!({"unique_id":10,"item_index":index,"current_dura":0,"max_dura":0,
        "count":1,"soul_bound_id":0,"identified":true,"cursed":false,"slots":[],
        "gem_count":0,"added_stats":[],"awake_type":0,"awake_values":[],"refined_value":0,
        "refine_added":0,"refine_success_chance":0,"wedding_ring":-1,"expire_info":null,
        "rental_information":null,"is_shop_item":false,"sealed_info":null,"gm_made":false});
    carrier["unique_id"] = json!(20);
    carrier["slot"] = json!(3);
    carrier["socket_slots"] = json!(1);
    carrier["user_item_metadata"] = json!({"item_index":index,"slots":[old_wire],
        "live_socketed_at_capture":captured,"socket_layout_hydrated":hydrated,
        "captured_socket_positions":[{"unique_id":10,"item_index":index}]});
    let encoded = carrier.to_string();
    let mutation = plan.accounts.get_mut("sender").unwrap();
    for account in [
        &mut mutation.original_account,
        &mut mutation.desired_account,
    ] {
        account
            .as_mut()
            .unwrap()
            .saves
            .get_mut(&1)
            .unwrap()
            .inventory_items_json
            .push(encoded.clone());
    }
    encoded
}

#[test]
fn mail_transfer_send_ignores_legitimate_retired_metadata_slots_after_live_capture() {
    for (captured, hydrated) in [(true, false), (false, true), (true, true)] {
        let mut plan = planned_send();
        let encoded = add_retained_socket_carrier(&mut plan, captured, hydrated);
        // The strict real carrier interpreter confirms this complete valid
        // carrier retains UID10 as history, while only UID20 is physical.
        let manifest = attachment_manifest(std::slice::from_ref(&encoded)).unwrap();
        assert_eq!(manifest.live_uids, BTreeSet::from([20]));
        assert_eq!(
            bag_live_uids(&bag_identity(&encoded).unwrap()).unwrap(),
            BTreeSet::from([20])
        );
        assert_eq!(new_receipts(&plan).unwrap().len(), 1);
    }
}

#[test]
fn mail_transfer_send_still_rejects_active_legacy_metadata_socket_copy() {
    let mut plan = planned_send();
    let encoded = add_retained_socket_carrier(&mut plan, false, false);
    let manifest = attachment_manifest(std::slice::from_ref(&encoded)).unwrap();
    assert_eq!(manifest.live_uids, BTreeSet::from([10, 20]));
    assert_eq!(
        bag_live_uids(&bag_identity(&encoded).unwrap()).unwrap(),
        BTreeSet::from([10, 20])
    );
    assert!(new_receipts(&plan).is_err());
}

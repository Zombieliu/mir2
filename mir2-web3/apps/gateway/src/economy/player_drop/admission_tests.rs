use super::*;
use mir2_protocol::UserItemStat;

fn operation() -> PlayerDropPreparedOperation {
    let operation = super::super::tests::fixture();
    let intent = mir2_simulation::PlayerDropWorldIntent::new(
        operation.binding().clone(),
        operation.intent().zone().clone(),
        operation.intent().position().clone(),
        100,
        100 + 30 * 60 * 1000,
        operation.intent().outcome().clone(),
    )
    .unwrap();
    let operation_digest = digest(
        b"mir2/player-drop/prepared-source/v1\0",
        &(
            &intent,
            operation.before(),
            operation.runtime_before(),
            operation.checkpoint(),
        ),
    )
    .unwrap();
    serde_json::from_value(serde_json::json!({"intent":intent,"before":operation.before(),"runtime_before":operation.runtime_before(),"checkpoint":operation.checkpoint(),"operation_digest":operation_digest})).unwrap()
}
fn gold() -> FrozenPlayerDropAdmission {
    let operation = operation();
    FrozenPlayerDropAdmission::new(
        operation.binding().clone(),
        operation.intent().zone().clone(),
        operation.intent().position().clone(),
        operation.intent().created_at_unix_ms(),
        operation.intent().expires_at_unix_ms(),
        PlayerDropRequest::Gold { amount: 25 },
        operation.intent().outcome().clone(),
    )
    .unwrap()
}
fn split() -> FrozenPlayerDropAdmission {
    let operation = super::super::tests::fixture();
    let template = mir2_game_data::crystal_item_by_name("(HP)DrugSmall").unwrap();
    let child_template = mir2_game_data::crystal_item_by_name("BronzeBell").unwrap();
    let mut item = UserItem {
        unique_id: 0,
        item_index: template.item_index,
        current_dura: 123,
        max_dura: 456,
        count: 2,
        soul_bound_id: 0,
        identified: false,
        cursed: true,
        slots: vec![],
        gem_count: 3,
        added_stats: vec![UserItemStat { stat: 17, value: 6 }],
        awake_type: 2,
        awake_values: vec![4, 5],
        refined_value: 6,
        refine_added: 7,
        refine_success_chance: 88,
        wedding_ring: 23,
        expire_info: None,
        rental_information: None,
        is_shop_item: true,
        sealed_info: None,
        gm_made: true,
    };
    let mut child = item.clone();
    child.item_index = child_template.item_index;
    child.count = 1;
    item.slots = vec![None, Some(child)];
    let preview = PlayerDropOutcome::Item {
        quantity: 2,
        loot: GroundDropLootSnapshot::InventoryItem {
            key: format!("crystal-item-{}", template.item_index),
            name: template.name,
            description: "exact split carrier".into(),
            weight: u16::from(template.weight),
            durability_current: Some(item.current_dura),
            durability_max: Some(item.max_dura),
            added_attack: 0,
            added_defence: 0,
            added_stats: item.added_stats.clone(),
            cursed: item.cursed,
            socket_slots: 2,
            show_group_pickup: template.show_group_pickup,
            exact_item: Some(mir2_simulation::GroundDropItemPayload {
                item,
                uid_assigned: false,
            }),
        },
    };
    FrozenPlayerDropAdmission::new(
        operation.binding().clone(),
        operation.intent().zone().clone(),
        operation.intent().position().clone(),
        100,
        100 + 30 * 60 * 1000,
        PlayerDropRequest::Item {
            unique_id: 765001,
            count: 2,
            hero_inventory: false,
        },
        preview,
    )
    .unwrap()
}
#[test]
fn admission_freezes_request_placement_absolute_expiry_and_rejects_tampering() {
    let frozen = gold();
    frozen.validate_operation(&operation()).unwrap();
    assert_eq!(frozen.expected_uid_count(), 0);
    for field in [
        "position",
        "createdAtUnixMs",
        "expiresAtUnixMs",
        "request",
        "admissionDigest",
    ] {
        let mut value = serde_json::to_value(&frozen).unwrap();
        match field {
            "position" => value[field]["x"] = 99.into(),
            "request" => value[field]["amount"] = 26.into(),
            "admissionDigest" => value[field] = "0".repeat(64).into(),
            _ => value[field] = 999.into(),
        }
        let changed: FrozenPlayerDropAdmission = serde_json::from_value(value).unwrap();
        assert!(changed.validate_integrity().is_err(), "{field}");
    }
    let mut mismatch = frozen.clone();
    mismatch.request = PlayerDropRequest::Gold { amount: 26 };
    assert!(mismatch.validate_payload().is_err());
    assert!(
        FrozenPlayerDropAdmission::new(
            frozen.binding().clone(),
            frozen.zone().clone(),
            frozen.position().clone(),
            100,
            10000,
            frozen.request().clone(),
            frozen.preview().clone()
        )
        .is_err(),
        "Gold admission preserves exactly the existing thirty-minute lifetime"
    );
}
#[test]
fn admission_split_preview_accepts_only_complete_zero_tree_and_preserves_all_attributes() {
    let frozen = split();
    assert_eq!(frozen.expected_uid_count(), 2);
    frozen.validate_integrity().unwrap();
    let PlayerDropOutcome::Item {
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(exact),
                ..
            },
        ..
    } = &frozen.preview
    else {
        unreachable!()
    };
    assert_eq!(exact.item.current_dura, 123);
    assert_eq!(exact.item.slots[1].as_ref().unwrap().unique_id, 0);
    let mut invalid = frozen.clone();
    let PlayerDropOutcome::Item {
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(exact),
                ..
            },
        ..
    } = &mut invalid.preview
    else {
        unreachable!()
    };
    exact.item.slots[1].as_mut().unwrap().unique_id = 9;
    assert!(
        invalid.validate_payload().is_err(),
        "partially assigned preview is never an allocation witness"
    );
    let mut invalid = frozen.clone();
    let PlayerDropOutcome::Item {
        loot:
            GroundDropLootSnapshot::InventoryItem {
                exact_item: Some(exact),
                ..
            },
        ..
    } = &mut invalid.preview
    else {
        unreachable!()
    };
    exact.item.item_index = -123;
    assert!(invalid.validate_payload().is_err());
    let mut wrong_key = frozen.clone();
    let PlayerDropOutcome::Item {
        loot: GroundDropLootSnapshot::InventoryItem { key, .. },
        ..
    } = &mut wrong_key.preview
    else {
        unreachable!()
    };
    *key = "crystal-item-1".into();
    assert!(wrong_key.validate_payload().is_err());
    let mut legacy = frozen.clone();
    let PlayerDropOutcome::Item {
        loot: GroundDropLootSnapshot::InventoryItem { key, .. },
        ..
    } = &mut legacy.preview
    else {
        unreachable!()
    };
    *key = "red-potion".into();
    legacy.validate_payload().unwrap();
    let mut invalid = frozen;
    invalid.request = PlayerDropRequest::Item {
        unique_id: 765001,
        count: 3,
        hero_inventory: false,
    };
    assert!(invalid.validate_payload().is_err());
}
#[test]
fn admission_owner_rejection_and_successor_abort_have_distinct_epoch_rules() {
    let frozen = gold();
    let mut audit = PlayerDropAdmissionAbortAudit {
        version: 1,
        admission_digest: frozen.admission_digest().into(),
        resolution_kind: "ownerRejected".into(),
        recovery_id: "reject-1".into(),
        reason: "known rejection before source preparation".into(),
        recovery_owner_id: frozen.binding().owner_id().into(),
        recovery_epoch: frozen.binding().owner_epoch(),
        aborted_at_ms: 1000,
        bootstrap_witness: None,
        uid_reservations: None,
    };
    audit.validate(&frozen).unwrap();
    audit.recovery_owner_id = "other-owner".into();
    assert!(audit.validate(&frozen).is_err());
    audit.resolution_kind = "successorAborted".into();
    assert!(audit.validate(&frozen).is_err());
    audit.recovery_epoch += 1;
    audit.validate(&frozen).unwrap();
    let digest = audit.digest().unwrap();
    audit.reason = "changed audit reason".into();
    assert_ne!(digest, audit.digest().unwrap());
}
#[test]
fn admission_abort_retains_exact_uid_witness_and_never_accepts_reused_or_extra_ids() {
    let frozen = split();
    let mut audit = PlayerDropAdmissionAbortAudit {
        version: 1,
        admission_digest: frozen.admission_digest().into(),
        resolution_kind: "successorAborted".into(),
        recovery_id: "retire-1".into(),
        reason: "fenced unpublished admission".into(),
        recovery_owner_id: "successor".into(),
        recovery_epoch: frozen.binding().owner_epoch() + 1,
        aborted_at_ms: 1000,
        bootstrap_witness: None,
        uid_reservations: Some(vec![9001, 9002]),
    };
    audit.validate(&frozen).unwrap();
    assert_eq!(audit.uid_reservations(), Some([9001, 9002].as_slice()));
    audit.uid_reservations = Some(vec![9001, 9001]);
    assert!(audit.validate(&frozen).is_err());
    audit.uid_reservations = Some(vec![9001]);
    assert!(audit.validate(&frozen).is_err());
    audit.uid_reservations = None;
    audit.validate(&frozen).unwrap();
}

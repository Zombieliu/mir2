//! Direct real-World application tests. These do not install a NativeInbound
//! producer or prove physical delivery; actual NativeInbound/FIFO coverage is
//! separate in hunting_material_ingest_tests.rs. Preparation never is Applied.
use super::*;
use mir2_client_bevy::{
    hero_model::{project_owner_item, HeroModel},
    inventory::{
        CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel, InventoryModel,
        ItemModel,
    },
    mail::MailModel,
    read_model::UiReadModel,
    shop::ShopModel,
    skill_model::SkillModel,
    social::SocialModel,
    storage::StorageModel,
};
use mir2_client_wire::U64;
use serde_json::json;

const INPUT: u64 = 9_100_001;
const OUTPUT: u64 = u64::MAX - 1;

fn epoch() -> NativeHuntingMaterialEpoch {
    NativeHuntingMaterialEpoch {
        run: 1,
        connection: 2,
        procedure: 3,
        owner_epoch: 4,
        scene_epoch: 5,
        cancellation: 0,
        actor: 3,
        map: 0,
    }
}

fn owner() -> NativeHuntingMaterialOwner {
    NativeHuntingMaterialOwner::new("synthetic-native-material", "alice", 7).unwrap()
}

fn item(uid: u64, slot: u8, count: u16) -> Value {
    item_from_template(uid, slot, count, "(HP)DrugSmall")
}

fn item_from_template(uid: u64, slot: u8, count: u16, name: &str) -> Value {
    let template = mir2_game_data::crystal_item_by_name(name).unwrap();
    let real = mir2_game_data::crystal_real_item_for_player(
        &template,
        8,
        mir2_protocol::MirClass::Warrior,
    );
    let info: CrystalItemInfoModel =
        serde_json::from_value(serde_json::to_value(&template).unwrap()).unwrap();
    let real_info: CrystalItemInfoModel =
        serde_json::from_value(serde_json::to_value(real).unwrap()).unwrap();
    let tooltip = CrystalItemTooltipSourceModel {
        info,
        real_info: Some(real_info),
        user_item: Some(CrystalUserItemModel {
            unique_id: uid,
            item_index: template.item_index,
            count,
            ..Default::default()
        }),
        socket_infos: vec![],
        real_socket_infos: vec![],
    };
    json!({"uniqueId":uid,"key":format!("crystal-item-{}",template.item_index),
        "name":template.name,"slot":slot,"container":"bag1","quantity":count,
        "icon":template.image,"description":"Synthetic complete native material fixture",
        "durabilityCurrent":null,"durabilityMax":null,"sellValue":0,"grade":"common",
        "addedAttack":0,"addedDefence":0,"tooltipSource":tooltip})
}

fn rebuild_inventory(projection: &mut NativeNpcEconomyProjection, owner: &Value) {
    let rows = owner["inventoryItems"].as_array().unwrap();
    let items = rows
        .iter()
        .map(|row| {
            let (container, base) = match row["container"].as_str().unwrap() {
                "bag1" => (0, 0),
                "bag2" => (0, 40),
                "quest" => (3, 0),
                _ => panic!("Unsupported test inventory container"),
            };
            project_owner_item(row, container, base + row["slot"].as_u64().unwrap()).unwrap()
        })
        .collect();
    projection.owner_json = owner.to_string();
    projection.inventory_json = serde_json::to_string(&InventoryModel {
        gold: 90,
        items,
        ..Default::default()
    })
    .unwrap();
}

pub(crate) fn fixture(
    partial: bool,
) -> (
    NativeHuntingMaterialGate,
    Quote,
    Receipt,
    NativeNpcEconomyProjection,
) {
    let (_, _, mut projection) = crate::npc_purchase_economy::tests::fixture();
    let mut source: Value = serde_json::from_str(&projection.owner_json).unwrap();
    source["mapIndex"] = json!(0);
    source["inventoryItems"] = if partial {
        json!([item(INPUT, 0, 9), item(OUTPUT, 1, 1)])
    } else {
        json!([item(INPUT, 0, 10)])
    };
    rebuild_inventory(&mut projection, &source);
    let quantity = if partial { 1 } else { 10 };
    let quote = Quote {
        quote_id: "ab".repeat(32),
        unique_id: U64::new(INPUT),
        item_index: mir2_game_data::crystal_item_by_name("(HP)DrugSmall")
            .unwrap()
            .item_index,
        input_count: U64::new(10),
        quantity: U64::new(quantity),
        unit_gold: U64::new(125),
        gold_equivalent: U64::new(125 * quantity),
        fee_unbound_gold: U64::new((125 * quantity).div_ceil(50)),
        license_atoms_debit: U64::new(125 * quantity),
        valuation_version: "synthetic-native-prices-v1".into(),
        created_at_ms: 1_000,
        expires_at_ms: 31_000,
    };
    let receipt = Receipt {
        quote_id: quote.quote_id.clone(),
        input_unique_id: quote.unique_id,
        output_unique_id: U64::new(if partial { OUTPUT } else { INPUT }),
        input_count: quote.input_count,
        quantity: quote.quantity,
        gold_equivalent: quote.gold_equivalent,
        fee_unbound_gold: quote.fee_unbound_gold,
        license_atoms_debit: quote.license_atoms_debit,
        valuation_version: quote.valuation_version.clone(),
        operation_key: "cd".repeat(32),
        request_digest: format!("sha256:{}", "ef".repeat(32)),
        business_day: 10,
        committed_at_ms: 2_000,
    };
    receipt.validate_for_quote(&quote).unwrap();
    let gate = NativeHuntingMaterialGate::new(epoch(), owner()).unwrap();
    (gate, quote, receipt, projection)
}

fn models(world: &World) -> Value {
    json!({
        "ui":world.get_resource::<UiReadModel>().map(|model|serde_json::to_value(model).unwrap()),
        "inventory":world.get_resource::<InventoryModel>().map(|model|serde_json::to_value(model).unwrap()),
        "mail":world.get_resource::<MailModel>().map(|model|serde_json::to_value(model).unwrap()),
        "storage":world.get_resource::<StorageModel>().map(|model|serde_json::to_value(model).unwrap()),
        "shop":world.get_resource::<ShopModel>().map(|model|serde_json::to_value(model).unwrap()),
        "skill":world.get_resource::<SkillModel>().map(|model|serde_json::to_value(model).unwrap()),
        "hero":world.get_resource::<HeroModel>().map(|model|serde_json::to_value(model).unwrap()),
        "social":world.get_resource::<SocialModel>().map(|model|serde_json::to_value(model).unwrap()),
    })
}

#[test]
fn native_hunting_material_only_full_world_commit_emits_original_opaque_applied() {
    for partial in [false, true] {
        let (gate, quote, receipt, projection) = fixture(partial);
        let original = projection.owner_json.clone();
        let expected_inventory: Value = serde_json::from_str(&projection.inventory_json).unwrap();
        let bundle = gate.prepare(1, &quote, &receipt, projection).unwrap();
        let mut world = World::new();
        assert!(gate.try_recv_applied().is_none());
        assert!(!world.contains_resource::<InventoryModel>());
        let replay = bundle.clone();
        apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
        assert_eq!(
            serde_json::to_value(world.resource::<InventoryModel>()).unwrap(),
            expected_inventory
        );
        for key in [
            "ui",
            "inventory",
            "mail",
            "storage",
            "shop",
            "skill",
            "hero",
            "social",
        ] {
            assert!(models(&world)[key].is_object(), "complete model {key}");
        }
        let source = world.resource::<NativeHuntingMaterialSource>();
        assert_eq!(source.owner_json(), original);
        let applied = gate.try_recv_applied().unwrap();
        assert_eq!(applied.epoch(), epoch());
        assert_eq!(applied.owner(), &owner());
        assert_eq!(applied.quote(), &quote);
        assert_eq!(applied.receipt(), &receipt);
        assert_eq!(applied.application_revision(), 1);
        assert_eq!(applied.request_id(), 1);
        assert_eq!(source.applied(), &applied);
        assert_eq!(
            gate.with_current_applied(&applied, || "actual-models-complete"),
            Some("actual-models-complete")
        );
        assert!(apply_native_hunting_material_bundle(&mut world, replay).is_err());
        assert!(gate.try_recv_applied().is_none());
    }
}

#[test]
fn native_hunting_material_malformed_owner_receipt_or_projection_never_prepares_or_mutates() {
    let (gate, quote, receipt, projection) = fixture(true);
    assert!(gate
        .prepare(0, &quote, &receipt, projection.clone())
        .is_err());
    assert!(gate
        .prepare(u64::MAX, &quote, &receipt, projection.clone())
        .is_err());
    for domain in [
        "inventory",
        "mail",
        "storage",
        "shop",
        "skill",
        "hero",
        "social",
        "world",
        "ui",
    ] {
        let mut bad = projection.clone();
        *match domain {
            "inventory" => &mut bad.inventory_json,
            "mail" => &mut bad.mail_json,
            "storage" => &mut bad.storage_json,
            "shop" => &mut bad.shop_json,
            "skill" => &mut bad.skill_json,
            "hero" => &mut bad.hero_json,
            "social" => &mut bad.social_json,
            "world" => &mut bad.world_json,
            _ => &mut bad.ui_json,
        } = "null".into();
        assert!(gate.prepare(1, &quote, &receipt, bad).is_err(), "{domain}");
    }
    for field in ["fee", "quantity", "quote", "operation"] {
        let mut bad = receipt.clone();
        match field {
            "fee" => bad.fee_unbound_gold = U64::new(2),
            "quantity" => bad.quantity = U64::new(2),
            "quote" => bad.quote_id = "aa".repeat(32),
            _ => bad.operation_key = "invalid".into(),
        }
        assert!(
            gate.prepare(1, &quote, &bad, projection.clone()).is_err(),
            "{field}"
        );
    }
    for field in ["uid-alias", "cell", "map", "actor", "missing-map"] {
        let mut bad = projection.clone();
        let mut raw: Value = serde_json::from_str(&bad.owner_json).unwrap();
        match field {
            "uid-alias" => {
                raw["inventoryItems"][1] = item(INPUT, 1, 1);
            }
            "cell" => {
                raw["inventoryItems"][1] = item(OUTPUT, 0, 1);
            }
            "map" => raw["mapIndex"] = json!(1),
            "actor" => raw["playerObjectId"] = json!(4),
            _ => {
                raw.as_object_mut().unwrap().remove("mapIndex");
            }
        }
        rebuild_inventory(&mut bad, &raw);
        assert!(gate.prepare(1, &quote, &receipt, bad).is_err(), "{field}");
    }
    assert!(gate.try_recv_applied().is_none());
    // Every failed predecode left the original request/revision unconsumed.
    let bundle = gate.prepare(1, &quote, &receipt, projection).unwrap();
    let mut world = World::new();
    apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
    assert_eq!(gate.try_recv_applied().unwrap().application_revision(), 1);
}

#[test]
fn native_hunting_material_gate_instance_owner_and_full_epoch_cannot_borrow_completion() {
    let (first, quote, receipt, projection) = fixture(false);
    let mut world = World::new();
    apply_native_hunting_material_bundle(
        &mut world,
        first
            .prepare(1, &quote, &receipt, projection.clone())
            .unwrap(),
    )
    .unwrap();
    let applied = first.try_recv_applied().unwrap();
    for other_owner in [
        NativeHuntingMaterialOwner::new("other-realm", "alice", 7).unwrap(),
        NativeHuntingMaterialOwner::new("synthetic-native-material", "bob", 7).unwrap(),
        NativeHuntingMaterialOwner::new("synthetic-native-material", "alice", 8).unwrap(),
        owner(),
    ] {
        let other = NativeHuntingMaterialGate::new(epoch(), other_owner).unwrap();
        let mut separate = World::new();
        apply_native_hunting_material_bundle(
            &mut separate,
            other
                .prepare(1, &quote, &receipt, projection.clone())
                .unwrap(),
        )
        .unwrap();
        assert!(
            other.with_current_applied(&applied, || ()).is_none(),
            "even identical stamps/terms cannot cross a gate instance"
        );
    }
    assert_eq!(
        first.with_current_applied(&applied, || applied.request_id()),
        Some(1)
    );
    for other in [
        NativeHuntingMaterialEpoch {
            actor: 4,
            ..epoch()
        },
        NativeHuntingMaterialEpoch { map: 1, ..epoch() },
    ] {
        let wrong = NativeHuntingMaterialGate::new(other, owner()).unwrap();
        assert!(wrong
            .prepare(1, &quote, &receipt, projection.clone())
            .is_err());
    }
    for bad in [
        NativeHuntingMaterialEpoch {
            connection: 0,
            ..epoch()
        },
        NativeHuntingMaterialEpoch { map: -1, ..epoch() },
        NativeHuntingMaterialEpoch {
            run: u64::MAX,
            ..epoch()
        },
    ] {
        assert!(NativeHuntingMaterialGate::new(bad, owner()).is_err());
    }
}

#[test]
fn native_hunting_material_superseding_preparation_and_retirement_revoke_old_tokens() {
    let (gate, quote, receipt, projection) = fixture(false);
    let first = gate
        .prepare(1, &quote, &receipt, projection.clone())
        .unwrap();
    let second = gate
        .prepare(2, &quote, &receipt, projection.clone())
        .unwrap();
    let mut world = World::new();
    assert!(!first.is_current());
    assert!(apply_native_hunting_material_bundle(&mut world, first).is_err());
    assert!(!world.contains_resource::<InventoryModel>());
    apply_native_hunting_material_bundle(&mut world, second).unwrap();
    let old = gate.try_recv_applied().unwrap();
    let current = gate
        .prepare(3, &quote, &receipt, projection.clone())
        .unwrap();
    assert!(
        gate.with_current_applied(&old, || ()).is_none(),
        "new candidate invalidates an already popped old completion"
    );
    apply_native_hunting_material_bundle(&mut world, current).unwrap();
    let current = gate.try_recv_applied().unwrap();
    assert_eq!(current.application_revision(), 3);
    assert_eq!(gate.with_current_applied(&current, || ()), Some(()));
    let late = gate.prepare(4, &quote, &receipt, projection).unwrap();
    gate.retire();
    let before = models(&world);
    assert!(apply_native_hunting_material_bundle(&mut world, late).is_err());
    assert_eq!(models(&world), before);
    assert!(gate.with_current_applied(&current, || ()).is_none());
    assert!(gate.try_recv_applied().is_none());
}

#[test]
fn native_hunting_material_existing_source_reset_revokes_popped_and_late_application() {
    for scene in [false, true] {
        let (gate, quote, receipt, projection) = fixture(true);
        let mut world = World::new();
        apply_native_hunting_material_bundle(
            &mut world,
            gate.prepare(1, &quote, &receipt, projection.clone())
                .unwrap(),
        )
        .unwrap();
        let applied = gate.try_recv_applied().unwrap();
        let late = gate.prepare(2, &quote, &receipt, projection).unwrap();
        if scene {
            world.insert_resource(crate::SceneResetRevision(1));
        } else {
            world.insert_resource(mir2_client_bevy::pending_operations::SessionResetRevision(
                1,
            ));
        }
        let before = models(&world);
        clear_stale_native_hunting_material_source(&mut world);
        assert!(!world.contains_resource::<NativeHuntingMaterialSource>());
        assert!(gate.epoch().is_none());
        assert!(gate.with_current_applied(&applied, || ()).is_none());
        assert!(apply_native_hunting_material_bundle(&mut world, late).is_err());
        assert_eq!(models(&world), before);
    }
}

#[test]
fn native_hunting_material_completion_capacity_refuses_entire_commit_and_has_no_arc_cycle() {
    let (gate, quote, receipt, projection) = fixture(false);
    let weak = Arc::downgrade(&gate.custody);
    let mut world = World::new();
    for request in 1..=MAX_APPLIED as u64 {
        let bundle = gate
            .prepare(request, &quote, &receipt, projection.clone())
            .unwrap();
        apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
    }
    let bundle = gate
        .prepare(MAX_APPLIED as u64 + 1, &quote, &receipt, projection)
        .unwrap();
    let before = models(&world);
    assert!(apply_native_hunting_material_bundle(&mut world, bundle.clone()).is_err());
    assert_eq!(models(&world), before);
    let popped = gate.try_recv_applied().unwrap();
    assert!(gate.with_current_applied(&popped, || ()).is_none());
    apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
    drop(world);
    drop(gate);
    assert!(
        weak.upgrade().is_none(),
        "queued completions retain only weak custody"
    );
}

#[test]
fn native_hunting_material_display_gate_requires_exact_live_lifetime_and_preserves_clock() {
    let (gate, quote, receipt, projection) = fixture(false);
    let wrong = NativeHeroOwnerGate::new(NativeHeroOwnerEpoch {
        cancellation: 1,
        ..epoch().hero_epoch()
    })
    .unwrap();
    assert!(gate
        .prepare(1, &quote, &receipt, projection.clone())
        .unwrap()
        .with_hero_owner_gate(&wrong)
        .is_err());
    let display = NativeHeroOwnerGate::new(epoch().hero_epoch()).unwrap();
    let mut world = World::new();
    world.insert_resource(HeroModel {
        session_epoch: 17,
        hero_generation: 4,
        revision: 10,
        ..Default::default()
    });
    let raw: Value = serde_json::from_str(&projection.owner_json).unwrap();
    assert!(crate::npc_purchase_economy::apply_native_hero_owner_update(
        &mut world,
        display.prepare(&raw).unwrap()
    ));
    let before = world.resource::<HeroModel>().clone();
    let bundle = gate
        .prepare(2, &quote, &receipt, projection.clone())
        .unwrap()
        .with_hero_owner_gate(&display)
        .unwrap();
    apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
    let current = world.resource::<HeroModel>();
    assert_eq!(
        (current.session_epoch, current.hero_generation),
        (before.session_epoch, before.hero_generation)
    );
    assert!(current.revision > before.revision);
    let late = gate
        .prepare(3, &quote, &receipt, projection)
        .unwrap()
        .with_hero_owner_gate(&display)
        .unwrap();
    display.retire();
    let before = models(&world);
    assert!(apply_native_hunting_material_bundle(&mut world, late).is_err());
    assert_eq!(models(&world), before);
}

#[test]
fn native_hunting_material_final_callback_holds_original_custody_against_retirement() {
    use std::sync::{mpsc, TryLockError};
    use std::time::Duration;
    let (gate, quote, receipt, projection) = fixture(false);
    let mut world = World::new();
    apply_native_hunting_material_bundle(
        &mut world,
        gate.prepare(1, &quote, &receipt, projection).unwrap(),
    )
    .unwrap();
    let applied = gate.try_recv_applied().unwrap();
    let (begin_tx, begin_rx) = mpsc::channel();
    let (attempt_tx, attempt_rx) = mpsc::channel();
    let (retired_tx, retired_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let retiring = gate.clone();
        let worker = scope.spawn(move || {
            begin_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            attempt_tx.send(()).unwrap();
            retiring.retire();
            retired_tx.send(()).unwrap();
        });
        assert_eq!(
            gate.with_current_applied(&applied, || {
                begin_tx.send(()).unwrap();
                attempt_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                assert!(matches!(
                    gate.custody.try_lock(),
                    Err(TryLockError::WouldBlock)
                ));
                assert!(matches!(
                    retired_rx.try_recv(),
                    Err(mpsc::TryRecvError::Empty)
                ));
                "completed-under-custody"
            }),
            Some("completed-under-custody")
        );
        retired_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
    });
    assert!(gate.with_current_applied(&applied, || ()).is_none());
}

#[test]
fn native_hunting_material_actual_commit_retires_prior_purchase_and_repair_sources() {
    for from_repair in [false, true] {
        let (purchase, witness, purchase_projection) =
            crate::npc_purchase_economy::tests::fixture();
        let mut world = World::new();
        let first = purchase
            .prepare(witness, purchase_projection.clone())
            .unwrap();
        assert!(crate::npc_purchase_economy::apply_native_npc_economy_bundle(&mut world, first));
        let late_purchase = purchase
            .prepare(witness, purchase_projection.clone())
            .unwrap();
        let late_repair = if from_repair {
            let repair = crate::hunting_repair_economy::NativeHuntingRepairGate::new(
                crate::hunting_repair_economy::NativeHuntingRepairEpoch {
                    run: 1,
                    connection: 2,
                    procedure: 3,
                    owner_epoch: 4,
                    scene_epoch: 5,
                    cancellation: 0,
                    actor: 3,
                    map: 0,
                },
            )
            .unwrap();
            let first = repair
                .prepare(1, &"12".repeat(32), purchase_projection.clone())
                .unwrap();
            assert!(
                crate::hunting_repair_economy::apply_native_hunting_repair_bundle(
                    &mut world, first
                )
            );
            Some(
                repair
                    .prepare(2, &"12".repeat(32), purchase_projection)
                    .unwrap(),
            )
        } else {
            None
        };
        let (gate, quote, receipt, projection) = fixture(false);
        apply_native_hunting_material_bundle(
            &mut world,
            gate.prepare(1, &quote, &receipt, projection).unwrap(),
        )
        .unwrap();
        assert!(!world.contains_resource::<crate::npc_purchase_economy::NativeNpcEconomySource>());
        assert!(
            !world.contains_resource::<crate::hunting_repair_economy::NativeHuntingRepairSource>()
        );
        assert!(
            !crate::npc_purchase_economy::apply_native_npc_economy_bundle(
                &mut world,
                late_purchase
            )
        );
        if let Some(late) = late_repair {
            assert!(
                !crate::hunting_repair_economy::apply_native_hunting_repair_bundle(
                    &mut world, late
                )
            );
        }
        assert!(world.contains_resource::<NativeHuntingMaterialSource>());
    }
}

#[test]
fn native_hunting_material_legacy_equipment_uid_none_stays_unknown_and_cannot_alias_output() {
    let (gate, quote, receipt, mut projection) = fixture(true);
    let mut raw: Value = serde_json::from_str(&projection.owner_json).unwrap();
    raw["equipmentItems"] = json!([{"slot":"weapon","key":"legacy-weapon","name":"Legacy blade",
        "quantity":1,"icon":0,"stateImage":0,"shape":null,"description":"",
        "durabilityCurrent":0,"durabilityMax":0,"grade":"common","attack":0,"defence":0,
        "addedAttack":0,"addedDefence":0,"addedLuck":0,"socketSlots":0,
        "sealedExpiryTimeBinaryDatetime":0,"sealedNextTimeBinaryDatetime":0}]);
    projection.owner_json = raw.to_string();
    let mut inventory: InventoryModel = serde_json::from_str(&projection.inventory_json).unwrap();
    inventory.items.push(ItemModel {
        unique_id: None,
        key: "legacy-weapon".into(),
        name: "Legacy blade".into(),
        quantity: 1,
        slot: 0,
        container: 2,
        grade: Some("common".into()),
        durability_current: Some(0),
        durability_max: Some(0),
        ..Default::default()
    });
    projection.inventory_json = serde_json::to_string(&inventory).unwrap();
    let bundle = gate.prepare(1, &quote, &receipt, projection).unwrap();
    let mut world = World::new();
    apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
    let equipment = world
        .resource::<InventoryModel>()
        .items
        .iter()
        .find(|item| item.container == 2)
        .unwrap();
    assert_eq!(equipment.unique_id, None);
    assert_eq!(equipment.key, "legacy-weapon");
    let mut bad = raw.clone();
    bad["equipmentItems"][0]["uniqueId"] = json!(OUTPUT);
    assert!(validate_material_owner(&bad, epoch(), &quote, &receipt).is_err());
    let mut hidden = raw;
    hidden["inventoryItems"][0]["tooltipSource"]["userItem"]["slots"] =
        json!([CrystalUserItemModel {
            unique_id: OUTPUT,
            item_index: quote.item_index,
            count: 1,
            ..Default::default()
        }]);
    assert!(validate_material_owner(&hidden, epoch(), &quote, &receipt).is_err());
}

#[test]
fn native_hunting_material_historical_receipt_applies_current_missing_output_or_changed_count() {
    for partial in [true, false] {
        let (gate, quote, receipt, mut projection) = fixture(partial);
        let mut current: Value = serde_json::from_str(&projection.owner_json).unwrap();
        if partial {
            current["inventoryItems"]
                .as_array_mut()
                .unwrap()
                .retain(|item| item["uniqueId"].as_u64() != Some(receipt.output_unique_id.get()));
        } else {
            // Eight units of the originally converted stack were legitimately
            // consumed after commit. Query's immutable quantity remains ten.
            current["inventoryItems"][0] = item(INPUT, 0, 2);
        }
        rebuild_inventory(&mut projection, &current);
        let expected: Value = serde_json::from_str(&projection.inventory_json).unwrap();
        let bundle = gate.prepare(7, &quote, &receipt, projection).unwrap();
        let mut world = World::new();
        apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
        assert_eq!(
            serde_json::to_value(world.resource::<InventoryModel>()).unwrap(),
            expected
        );
        assert_eq!(
            world.resource::<NativeHuntingMaterialSource>().owner(),
            &current
        );
        let applied = gate.try_recv_applied().unwrap();
        assert_eq!(applied.receipt(), &receipt);
        assert_eq!(applied.quote(), &quote);
        assert_eq!(
            gate.with_current_applied(&applied, || applied.request_id()),
            Some(7)
        );
    }
}

fn quest_item(uid: u64, slot: u8) -> Value {
    let mut item = item_from_template(uid, slot, 1, "CannibalLeaf");
    item["container"] = json!("quest");
    item
}

#[test]
fn native_hunting_material_current_quest_roots_apply_full_world_without_bag_eligibility() {
    for partial in [false, true] {
        let (gate, quote, receipt, mut projection) = fixture(partial);
        let mut current: Value = serde_json::from_str(&projection.owner_json).unwrap();
        current["inventoryItems"]
            .as_array_mut()
            .unwrap()
            .extend([quest_item(INPUT + 100, 0), quest_item(INPUT + 101, 79)]);
        // Quest slot zero overlaps an ordinary Bag cell; Quest slot 79 is
        // valid even though this character has only the base Bag capacity.
        rebuild_inventory(&mut projection, &current);
        let expected: InventoryModel = serde_json::from_str(&projection.inventory_json).unwrap();
        let bundle = gate.prepare(1, &quote, &receipt, projection).unwrap();
        let mut world = World::new();
        assert!(gate.try_recv_applied().is_none());
        apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
        assert_eq!(
            serde_json::to_value(world.resource::<InventoryModel>()).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let quest: Vec<_> = world
            .resource::<InventoryModel>()
            .items
            .iter()
            .filter(|item| item.container == 3)
            .map(|item| (item.unique_id, item.slot))
            .collect();
        assert_eq!(quest, [(Some(INPUT + 100), 0), (Some(INPUT + 101), 79)]);
        assert_eq!(
            world.resource::<NativeHuntingMaterialSource>().owner(),
            &current
        );
        let applied = gate.try_recv_applied().unwrap();
        assert_eq!(applied.quote(), &quote);
        assert_eq!(applied.receipt(), &receipt);
        // Full-snapshot acceptance never weakens ordinary Bag cell authority.
        assert!(bag_cell(&current["inventoryItems"][expected.items.len() - 1], 40).is_err());
    }
}

#[test]
fn native_hunting_material_current_quest_cells_reject_duplicate_and_malformed_slots() {
    let (gate, quote, receipt, mut projection) = fixture(false);
    let mut current: Value = serde_json::from_str(&projection.owner_json).unwrap();
    current["inventoryItems"]
        .as_array_mut()
        .unwrap()
        .push(quest_item(INPUT + 100, 0));
    rebuild_inventory(&mut projection, &current);
    let mut invalids = Vec::new();
    let mut duplicate = current.clone();
    duplicate["inventoryItems"]
        .as_array_mut()
        .unwrap()
        .push(quest_item(INPUT + 101, 0));
    invalids.push(("duplicate", duplicate, true));
    let mut outside = current.clone();
    outside["inventoryItems"][1]["slot"] = json!(80);
    invalids.push(("outside", outside, true));
    let mut negative = current.clone();
    negative["inventoryItems"][1]["slot"] = json!(-1);
    invalids.push(("negative", negative, false));
    let mut missing = current.clone();
    missing["inventoryItems"][1]
        .as_object_mut()
        .unwrap()
        .remove("slot");
    invalids.push(("missing", missing, false));
    let mut unknown = current.clone();
    unknown["inventoryItems"][1]["container"] = json!("quest-other");
    invalids.push(("unknown", unknown, false));
    for (case, invalid, reproject) in invalids {
        assert!(
            validate_material_owner(&invalid, epoch(), &quote, &receipt).is_err(),
            "{case}"
        );
        let mut rejected = projection.clone();
        if reproject {
            // These malformed cells are faithfully projected, so rejection
            // tests the current custody slot contract rather than mismatch.
            rebuild_inventory(&mut rejected, &invalid);
        } else {
            rejected.owner_json = invalid.to_string();
        }
        assert!(
            gate.prepare(1, &quote, &receipt, rejected).is_err(),
            "{case}"
        );
        assert!(gate.try_recv_applied().is_none(), "{case}");
    }
    // Every rejected preparation leaves the same original request available.
    let bundle = gate.prepare(1, &quote, &receipt, projection).unwrap();
    let mut world = World::new();
    apply_native_hunting_material_bundle(&mut world, bundle).unwrap();
    assert_eq!(
        world.resource::<NativeHuntingMaterialSource>().owner(),
        &current
    );
    assert!(gate.try_recv_applied().is_some());
}

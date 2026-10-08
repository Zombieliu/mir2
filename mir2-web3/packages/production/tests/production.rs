use mir2_production::*;
use std::collections::{BTreeMap, BTreeSet};
fn fixture() -> (Catalog, ProductionAvailability) {
    let mut c = Catalog::bundled().unwrap();
    c.production_enabled = true;
    let a = ProductionAvailability {
        enabled: true,
        accepted_gates: c.source_gates.keys().cloned().collect(),
        accepted_facilities: c.facilities.keys().cloned().collect(),
        material_templates: c
            .items
            .keys()
            .enumerate()
            .map(|(i, k)| (k.clone(), i as i32 + 1000))
            .collect(),
    };
    (c, a)
}
fn lot(a: &ProductionAvailability, uid: u64, material: &str, n: u64, bound: bool) -> InputLot {
    InputLot {
        uid,
        material: material.into(),
        template: a.material_templates[material],
        quantity: n,
        bound,
        eligible: true,
        custody: format!("original-item-{uid}-metadata"),
    }
}
fn owner() -> ProductionOwner {
    ProductionOwner {
        account_id: "account".into(),
        character_id: "stable-character-id".into(),
    }
}
fn ctx<'a>(
    o: &'a ProductionOwner,
    a: &'a ProductionAvailability,
    lots: &'a [InputLot],
    now: u64,
) -> ProductionContext<'a> {
    ProductionContext {
        owner: o,
        facility_id: "public-bench",
        facility_kind: "bench",
        access_allowed: true,
        now_ms: now,
        inventory_revision: 7,
        gold: 10_000,
        lots,
        availability: a,
    }
}
fn start(c: &Catalog, id: &str, recipe: &str, batch: u16) -> ProductionCommand {
    ProductionCommand {
        request_id: id.into(),
        facility_id: "public-bench".into(),
        action: ProductionAction::Start {
            recipe: recipe.into(),
            catalog_version: c.schema.clone(),
            catalog_revision: c.revision().unwrap(),
            batch,
        },
    }
}
fn action(id: &str, job: &str, collect: bool) -> ProductionCommand {
    ProductionCommand {
        request_id: id.into(),
        facility_id: "public-bench".into(),
        action: if collect {
            ProductionAction::Collect { job: job.into() }
        } else {
            ProductionAction::Cancel { job: job.into() }
        },
    }
}
fn change(prep: ProductionPreparation) -> PreparedProduction {
    match prep {
        ProductionPreparation::Change(p) => *p,
        _ => panic!("expected transition"),
    }
}

#[test]
fn bundled_catalog_is_disabled_and_source_families_compose() {
    let c = Catalog::bundled().unwrap();
    let (_, a) = fixture();
    assert!(matches!(
        c.require_available("meal", &a),
        Err(ProductionError::Unavailable(_))
    ));
    assert_eq!(
        c.sources("buildingKit").unwrap().families,
        BTreeSet::from(["gathering".into(), "mining".into()])
    );
    assert_eq!(
        c.sources("meal").unwrap().families,
        BTreeSet::from([
            "gathering".into(),
            "mining".into(),
            "planting".into(),
            "fishing".into()
        ])
    );
    assert!(!c.sources("meal").unwrap().raw.contains("ore"));
}
#[test]
fn upstream_gate_or_unbound_intermediate_disables_final_recipe() {
    let (c, mut a) = fixture();
    a.accepted_gates.remove("saltMining");
    assert_eq!(
        c.require_available("meal", &a),
        Err(ProductionError::Unavailable("saltMining".into()))
    );
    assert!(c.require_available("ration", &a).is_ok());
    a.accepted_gates.insert("saltMining".into());
    a.material_templates.remove("charcoal");
    assert!(matches!(
        c.require_available("meal", &a),
        Err(ProductionError::Unavailable(_))
    ));
}
#[test]
fn invalid_catalog_cycles_missing_inputs_duplicates_and_overflow_reject() {
    let (c, _) = fixture();
    let mut bad = c.clone();
    bad.recipes.get_mut("charcoal").unwrap().inputs = BTreeMap::from([("metal".into(), 1)]);
    assert!(bad.validate().is_err());
    bad = c.clone();
    bad.recipes
        .get_mut("meal")
        .unwrap()
        .inputs
        .insert("unknown".into(), 1);
    assert!(bad.validate().is_err());
    bad = c.clone();
    bad.recipes.get_mut("rope").unwrap().output = "planks".into();
    assert!(bad.validate().is_err());
    bad = c;
    bad.recipes.get_mut("metal").unwrap().gold = u64::MAX;
    assert_eq!(bad.validate(), Err(ProductionError::Overflow));
}
#[test]
fn batch_plans_use_existing_intermediates_and_preserve_real_yield_surplus() {
    let (c, _) = fixture();
    let p = c.processing_plan("meal", 1, &BTreeMap::new()).unwrap();
    assert_eq!(
        p.missing_raw,
        BTreeMap::from([
            ("grain".into(), 3),
            ("fish".into(), 3),
            ("spice".into(), 1),
            ("rockSalt".into(), 3),
            ("wood".into(), 2)
        ])
    );
    assert_eq!((p.gold, p.minutes), (80, 5));
    assert_eq!(
        p.surplus,
        BTreeMap::from([
            ("flour".into(), 1),
            ("salt".into(), 2),
            ("charcoal".into(), 1)
        ])
    );
    let p = c
        .processing_plan(
            "meal",
            1,
            &BTreeMap::from([("flour".into(), 2), ("salt".into(), 1)]),
        )
        .unwrap();
    assert_eq!((p.gold, p.minutes), (40, 2));
    assert_eq!(p.missing_raw.len(), 2);
    assert_eq!(
        c.processing_plan("meal", 0, &BTreeMap::new()),
        Err(ProductionError::InvalidBatch)
    );
    assert_eq!(
        c.processing_plan("meal", 11, &BTreeMap::new()),
        Err(ProductionError::InvalidBatch)
    );
}
#[test]
fn lots_require_unique_uid_correct_template_and_server_eligibility() {
    let (c, a) = fixture();
    let mut lots = vec![lot(&a, 1, "wood", 2, false), lot(&a, 2, "wood", 1, true)];
    let p = plan_inputs(&c, "planks", 1, 10, &lots, &a.material_templates).unwrap();
    assert!(p.bound_output);
    assert_eq!(p.debits.len(), 2);
    lots[1].uid = 1;
    assert_eq!(
        plan_inputs(&c, "planks", 1, 10, &lots, &a.material_templates),
        Err(ProductionError::InvalidInventory)
    );
    lots[1].uid = 2;
    lots[1].eligible = false;
    assert!(matches!(
        plan_inputs(&c, "planks", 1, 10, &lots, &a.material_templates),
        Err(ProductionError::MissingMaterial(_))
    ));
    lots[1].eligible = true;
    lots[1].template += 1;
    assert!(matches!(
        plan_inputs(&c, "planks", 1, 10, &lots, &a.material_templates),
        Err(ProductionError::MissingMaterial(_))
    ));
}
#[test]
fn preparation_never_mutates_inventory_or_ledger_and_preserves_custody() {
    let (c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, 1, "wood", 3, true)];
    let ledger = ProductionLedger::default();
    let p = change(
        ledger
            .prepare(
                &c,
                &start(&c, "begin", "planks", 1),
                &ctx(&o, &a, &lots, 100),
            )
            .unwrap(),
    );
    assert_eq!(ledger.revision(), 0);
    assert_eq!(lots[0].quantity, 3);
    assert_eq!(p.expected_inventory_revision, 7);
    assert_eq!(p.gold_debit, 10);
    assert_eq!(p.debits[0].custody, lots[0].custody);
    assert_eq!(
        p.next_ledger
            .job(&p.receipt.job_id, &o)
            .unwrap()
            .finishes_ms,
        60_100
    );
}
#[test]
fn immutable_recipe_snapshot_survives_offline_restore_and_catalog_edit() {
    let (mut c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, 1, "wood", 3, true)];
    let p = change(
        ProductionLedger::default()
            .prepare(
                &c,
                &start(&c, "begin", "planks", 1),
                &ctx(&o, &a, &lots, 100),
            )
            .unwrap(),
    );
    let ledger = ProductionLedger::restore(&p.next_ledger.checkpoint_json().unwrap()).unwrap();
    c.recipes.get_mut("planks").unwrap().amount = 999;
    assert_eq!(
        ledger.prepare(
            &c,
            &action("claim", &p.receipt.job_id, true),
            &ctx(&o, &a, &lots, 60_099)
        ),
        Err(ProductionError::TooEarly)
    );
    let collected = change(
        ledger
            .prepare(
                &c,
                &action("claim", &p.receipt.job_id, true),
                &ctx(&o, &a, &lots, 60_100),
            )
            .unwrap(),
    );
    assert_eq!(collected.deliveries[0].quantity, 3);
    assert!(collected.deliveries[0].bound);
    assert_eq!(collected.gold_debit, 0);
    // Rejecting capacity/persistence leaves the original running ledger untouched.
    assert_eq!(
        ledger.job(&p.receipt.job_id, &o).unwrap().status,
        JobStatus::Running
    );
    assert!(ProductionLedger::restore(&collected.next_ledger.checkpoint_json().unwrap()).is_ok());
}
#[test]
fn stable_request_replay_is_exact_and_changed_payload_conflicts() {
    let (c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, 1, "wood", 9, false)];
    let command = start(&c, "begin", "planks", 1);
    let p = change(
        ProductionLedger::default()
            .prepare(&c, &command, &ctx(&o, &a, &lots, 100))
            .unwrap(),
    );
    assert_eq!(
        p.next_ledger
            .prepare(&c, &command, &ctx(&o, &a, &lots, 999)),
        Ok(ProductionPreparation::Replay(p.receipt.clone()))
    );
    assert_eq!(
        p.next_ledger.prepare(
            &c,
            &start(&c, "begin", "planks", 2),
            &ctx(&o, &a, &lots, 100)
        ),
        Err(ProductionError::RequestConflict)
    );
    assert_eq!(
        p.next_ledger.prepare(
            &c,
            &start(&c, "second", "planks", 1),
            &ctx(&o, &a, &lots, 100)
        ),
        Err(ProductionError::Busy)
    );
}
#[test]
fn cancel_returns_only_direct_composite_inputs_with_per_material_floor() {
    let (c, a) = fixture();
    let o = owner();
    let lots = vec![
        lot(&a, 1, "planks", 1, false),
        lot(&a, 2, "planks", 1, true),
        lot(&a, 3, "metal", 1, false),
        lot(&a, 4, "rope", 1, false),
    ];
    let p = change(
        ProductionLedger::default()
            .prepare(&c, &start(&c, "begin", "timber", 1), &ctx(&o, &a, &lots, 0))
            .unwrap(),
    );
    let cancel = change(
        p.next_ledger
            .prepare(
                &c,
                &action("cancel", &p.receipt.job_id, false),
                &ctx(&o, &a, &lots, 1),
            )
            .unwrap(),
    );
    assert_eq!(cancel.deliveries.len(), 1);
    let d = &cancel.deliveries[0];
    assert_eq!((&d.material, d.quantity), (&"planks".to_string(), 1));
    assert!(d.bound);
    assert_eq!(d.refund_origin.as_ref().unwrap().uid, 2);
    assert_eq!(d.refund_origin.as_ref().unwrap().custody, lots[1].custody);
    assert_eq!(cancel.gold_debit, 0);
    assert_eq!(cancel.receipt.status, JobStatus::Cancelled);
    assert_eq!(
        cancel.next_ledger.prepare(
            &c,
            &action("claim", &p.receipt.job_id, true),
            &ctx(&o, &a, &lots, 100_000)
        ),
        Err(ProductionError::AlreadyFinished)
    );
}
#[test]
fn exact_completion_refuses_cancel_and_owner_or_access_change_refuses_operation() {
    let (c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, 1, "wood", 3, false)];
    let p = change(
        ProductionLedger::default()
            .prepare(&c, &start(&c, "begin", "planks", 1), &ctx(&o, &a, &lots, 0))
            .unwrap(),
    );
    assert_eq!(
        p.next_ledger.prepare(
            &c,
            &action("cancel", &p.receipt.job_id, false),
            &ctx(&o, &a, &lots, 60_000)
        ),
        Err(ProductionError::AlreadyFinished)
    );
    let mut other = o.clone();
    other.character_id = "reused-slot-new-character".into();
    assert_eq!(
        p.next_ledger.prepare(
            &c,
            &action("claim", &p.receipt.job_id, true),
            &ctx(&other, &a, &lots, 60_000)
        ),
        Err(ProductionError::Unauthorized)
    );
    let mut access = ctx(&o, &a, &lots, 60_000);
    access.access_allowed = false;
    assert_eq!(
        p.next_ledger
            .prepare(&c, &action("claim", &p.receipt.job_id, true), &access),
        Err(ProductionError::Unauthorized)
    );
}
#[test]
fn restored_checkpoint_rejects_tampered_materials_status_and_clock() {
    let (c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, 1, "wood", 3, false)];
    let p = change(
        ProductionLedger::default()
            .prepare(&c, &start(&c, "begin", "planks", 1), &ctx(&o, &a, &lots, 0))
            .unwrap(),
    );
    let json = p.next_ledger.checkpoint_json().unwrap();
    for (field, value) in [
        ("finishes_ms", serde_json::json!(1)),
        ("status", serde_json::json!("Collected")),
    ] {
        let mut doc: serde_json::Value = serde_json::from_str(&json).unwrap();
        doc["jobs"][&p.receipt.job_id][field] = value;
        assert!(ProductionLedger::restore(&doc.to_string()).is_err());
    }
    let mut doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    doc["jobs"][&p.receipt.job_id]["debits"][0]["quantity"] = serde_json::json!(999);
    assert!(ProductionLedger::restore(&doc.to_string()).is_err());
}
#[test]
fn failed_gold_material_binding_or_time_overflow_never_creates_job() {
    let (c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, 1, "wood", 3, false)];
    let ledger = ProductionLedger::default();
    let mut context = ctx(&o, &a, &lots, 0);
    context.gold = 0;
    assert_eq!(
        ledger.prepare(&c, &start(&c, "begin", "planks", 1), &context),
        Err(ProductionError::InsufficientGold)
    );
    context.gold = 100;
    context.now_ms = u64::MAX;
    assert_eq!(
        ledger.prepare(&c, &start(&c, "begin", "planks", 1), &context),
        Err(ProductionError::Overflow)
    );
    assert_eq!(ledger.revision(), 0);
}

#[test]
fn upstream_facility_and_conflicting_template_bindings_are_required() {
    let (c, mut a) = fixture();
    a.accepted_facilities.remove("kiln");
    assert_eq!(
        c.require_available("buildingKit", &a),
        Err(ProductionError::Unavailable("facility:kiln".into()))
    );
    assert!(c.require_available("planks", &a).is_ok());
    a.accepted_facilities.insert("kiln".into());
    a.material_templates
        .insert("rockSalt".into(), a.material_templates["ore"]);
    assert_eq!(
        c.require_available("meal", &a),
        Err(ProductionError::Unavailable(
            "invalid material bindings".into()
        ))
    );
}

#[test]
fn edited_catalog_rejects_old_preview_but_exact_durable_request_replays() {
    let (mut c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, 1, "wood", 30, false)];
    let stale = start(&c, "begin", "planks", 1);
    let ledger = ProductionLedger::default();
    let original = change(ledger.prepare(&c, &stale, &ctx(&o, &a, &lots, 0)).unwrap());
    c.recipes.get_mut("planks").unwrap().gold = 11;
    assert_eq!(
        ledger.prepare(&c, &stale, &ctx(&o, &a, &lots, 0)),
        Err(ProductionError::Unavailable("catalog revision".into()))
    );
    // After durable commit, replay refers to the original payment, not a newly quoted price.
    let restored =
        ProductionLedger::restore(&original.next_ledger.checkpoint_json().unwrap()).unwrap();
    assert_eq!(
        restored.prepare(&c, &stale, &ctx(&o, &a, &lots, 0)),
        Ok(ProductionPreparation::Replay(original.receipt))
    );
}

#[test]
fn restored_checkpoint_rejects_changed_request_recipe_snapshot_and_duplicate_active_jobs() {
    let (c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, 1, "wood", 30, false)];
    let ledger = ProductionLedger::default();
    let context = ctx(&o, &a, &lots, 0);
    let first = change(
        ledger
            .prepare(&c, &start(&c, "first", "planks", 1), &context)
            .unwrap(),
    );
    let json = first.next_ledger.checkpoint_json().unwrap();
    let mut changed: serde_json::Value = serde_json::from_str(&json).unwrap();
    changed["receipts"][0]["command"]["action"]["Start"]["recipe"] = serde_json::json!("rope");
    assert_eq!(
        ProductionLedger::restore(&changed.to_string()),
        Err(ProductionError::InvalidCheckpoint)
    );
    let mut changed: serde_json::Value = serde_json::from_str(&json).unwrap();
    changed["receipts"][0]["command"]["request_id"] = serde_json::json!("bad:request");
    changed["receipts"][0]["receipt"]["request_id"] = serde_json::json!("bad:request");
    assert!(ProductionLedger::restore(&changed.to_string()).is_err());
    let mut changed: serde_json::Value = serde_json::from_str(&json).unwrap();
    changed["jobs"][&first.receipt.job_id]["recipe"]["amount"] = serde_json::json!(999);
    assert_eq!(
        ProductionLedger::restore(&changed.to_string()),
        Err(ProductionError::InvalidCheckpoint)
    );
    let second = change(
        ledger
            .prepare(&c, &start(&c, "second", "planks", 1), &context)
            .unwrap(),
    );
    let mut changed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let doc: serde_json::Value =
        serde_json::from_str(&second.next_ledger.checkpoint_json().unwrap()).unwrap();
    changed["jobs"][&second.receipt.job_id] = doc["jobs"][&second.receipt.job_id].clone();
    changed["receipts"]
        .as_array_mut()
        .unwrap()
        .push(doc["receipts"][0].clone());
    changed["revision"] = serde_json::json!(2);
    assert_eq!(
        ProductionLedger::restore(&changed.to_string()),
        Err(ProductionError::InvalidCheckpoint)
    );
}

#[test]
fn batch_ten_and_large_uid_preserve_cost_timing_quantity_and_bound_refunds() {
    let (c, a) = fixture();
    let o = owner();
    let lots = vec![lot(&a, u64::MAX, "wood", 31, true)];
    let first = change(
        ProductionLedger::default()
            .prepare(
                &c,
                &start(&c, "begin", "planks", 10),
                &ctx(&o, &a, &lots, 123),
            )
            .unwrap(),
    );
    assert_eq!(first.gold_debit, 100);
    assert_eq!(first.debits[0].quantity, 30);
    assert_eq!(first.debits[0].uid, u64::MAX);
    let json = first.next_ledger.checkpoint_json().unwrap();
    let ledger = ProductionLedger::restore(&json).unwrap();
    let j = ledger.job(&first.receipt.job_id, &o).unwrap();
    assert_eq!(j.finishes_ms, 600_123);
    let refund = change(
        ledger
            .prepare(
                &c,
                &action("cancel", &j.id, false),
                &ctx(&o, &a, &lots, 124),
            )
            .unwrap(),
    );
    assert_eq!(refund.deliveries[0].quantity, 15);
    assert!(refund.deliveries[0].bound);
    assert_eq!(
        refund.deliveries[0].refund_origin.as_ref().unwrap().uid,
        u64::MAX
    );
    let collected = change(
        ledger
            .prepare(
                &c,
                &action("claim", &j.id, true),
                &ctx(&o, &a, &lots, 600_123),
            )
            .unwrap(),
    );
    assert_eq!(collected.deliveries[0].quantity, 30);
    // Alternatives target the same ledger version. The authority must commit just one with inventory.
    assert_eq!(
        refund.expected_ledger_revision,
        collected.expected_ledger_revision
    );
    assert_eq!(ledger.revision(), 1);
}

#[test]
fn catalog_traversal_memoizes_shared_upstream_dependencies() {
    let (mut c, _) = fixture();
    // Repeated diamond dependencies would be exponential without source memoization.
    let mut previous = vec!["planks".to_string(), "rope".to_string()];
    for level in 0..24 {
        let mut current = Vec::new();
        for side in 0..2 {
            let key = format!("diamond{level}-{side}");
            c.items.insert(
                key.clone(),
                Material {
                    name: key.clone(),
                    stage: "component".into(),
                    family: None,
                    source_gate: None,
                    runtime_template_id: None,
                },
            );
            c.recipes.insert(
                key.clone(),
                Recipe {
                    name: key.clone(),
                    facility: "bench".into(),
                    inputs: previous.iter().map(|k| (k.clone(), 1)).collect(),
                    gold: 0,
                    minutes: 1,
                    output: key.clone(),
                    amount: 1,
                    unlock_gates: Vec::new(),
                },
            );
            current.push(key);
        }
        previous = current;
    }
    assert!(c.validate().is_ok());
    assert_eq!(
        c.sources(&previous[0]).unwrap().raw,
        BTreeSet::from(["wood".into(), "fiber".into()])
    );
    assert!(matches!(
        c.processing_plan(&previous[0], 1, &BTreeMap::new()),
        Err(ProductionError::InvalidCatalog(_))
    ));
}

#[test]
fn generated_checkpoint_budget_is_checked_before_any_economic_change() {
    let (mut c, a) = fixture();
    let o = owner();
    c.recipes
        .get_mut("planks")
        .unwrap()
        .inputs
        .insert("wood".into(), 512);
    // Every byte expands sixfold in JSON. Raw custody fits per-lot bounds but save would exceed 16 MiB.
    let mut lots: Vec<_> = (1..=512).map(|id| lot(&a, id, "wood", 1, false)).collect();
    for l in &mut lots {
        l.custody = "\u{0000}".repeat(16_384)
    }
    let ledger = ProductionLedger::default();
    assert_eq!(
        ledger.prepare(
            &c,
            &start(&c, "oversize", "planks", 1),
            &ctx(&o, &a, &lots, 0)
        ),
        Err(ProductionError::LedgerFull)
    );
    assert_eq!(ledger.revision(), 0);
    assert_eq!(lots[0].quantity, 1);
}

#[test]
fn restored_state_reserves_space_for_terminal_receipts() {
    let (mut c, a) = fixture();
    let o = owner();
    c.recipes
        .get_mut("planks")
        .unwrap()
        .inputs
        .insert("wood".into(), 512);
    let lots: Vec<_> = (1..=512).map(|id| lot(&a, id, "wood", 1, false)).collect();
    let prepared = change(
        ProductionLedger::default()
            .prepare(&c, &start(&c, "begin", "planks", 1), &ctx(&o, &a, &lots, 0))
            .unwrap(),
    );
    let mut doc: serde_json::Value =
        serde_json::from_str(&prepared.next_ledger.checkpoint_json().unwrap()).unwrap();
    for debit in doc["jobs"][&prepared.receipt.job_id]["debits"]
        .as_array_mut()
        .unwrap()
    {
        debit["custody"] = serde_json::json!("x");
    }
    let target = 16 * 1024 * 1024 - 128;
    let mut remaining = target - doc.to_string().len();
    for debit in doc["jobs"][&prepared.receipt.job_id]["debits"]
        .as_array_mut()
        .unwrap()
    {
        if remaining == 0 {
            break;
        }
        let encoded = (remaining + 1).min(16_379 * 6);
        debit["custody"] = serde_json::json!(format!(
            "{}{}",
            "\u{0000}".repeat(encoded / 6),
            "x".repeat(encoded % 6)
        ));
        remaining -= encoded - 1;
    }
    assert_eq!(remaining, 0);
    let encoded = doc.to_string();
    assert_eq!(encoded.len(), target);
    // The bytes fit, but a subsequent maximum-length claim receipt would not.
    assert_eq!(
        ProductionLedger::restore(&encoded),
        Err(ProductionError::LedgerFull)
    );
}

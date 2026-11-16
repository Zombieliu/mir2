use super::*;
use serde_json::{json, Value};

fn owner(account: &str, incarnation: &str) -> StoneOwner {
    StoneOwner { account_id: account.into(), character_incarnation: incarnation.into() }
}
fn creator() -> StoneOwner { owner("account-a", "character-a") }
fn request(grade: StoneGrade, serial: u64, carrier_uid: u64) -> StoneMintRequest {
    let (source_template_id, raw_template_id) = match grade {
        StoneGrade::Rough => (824, 980_001),
        StoneGrade::Fine => (826, 980_002),
        StoneGrade::Precious => (827, 980_003),
    };
    StoneMintRequest { owner: creator(), request_id: format!("mint-{serial}"), serial,
        carrier_uid, source_template_id, raw_template_id, raw_weight: 4, grade,
        binding: StoneBinding::default(), source: StoneSource { kind: StoneSourceKind::MiningWorkshop,
            source_version: "mining-workshop-v1".into(), event_id: format!("source-{serial}") } }
}
fn mint(catalog: &StoneCatalog, request: &StoneMintRequest, roll: u16) -> StoneRegistry {
    match StoneRegistry::default().prepare_mint(catalog, request, roll).unwrap() {
        StoneMintPreparation::Change(plan) => plan.next_registry,
        StoneMintPreparation::Replay(_) => panic!("new mint unexpectedly replayed"),
    }
}
fn minted(grade: StoneGrade, roll: u16) -> StoneRegistry {
    mint(&StoneCatalog::v1(), &request(grade, 1, 9_007_199_254_740_993), roll)
}
fn witness(registry: &StoneRegistry, serial: u64, holder: &StoneOwner) -> StonePossession {
    let record = registry.record(serial).unwrap();
    StonePossession { holder: holder.clone(), serial, carrier_uid: record.carrier_uid(),
        raw_template_id: record.raw_template_id(), raw_weight: record.raw_weight(), count: 1,
        bag_slot: 0, binding: record.binding(), eligible: true, locked: false }
}
fn context<'a>(holder: &'a StoneOwner, possessions: &'a [StonePossession]) -> StoneContext<'a> {
    StoneContext { owner: holder, authenticated: true, inventory_revision: 27, gold: 100_000,
        possessions, bag_capacity: 40, free_bag_slots: 39, current_weight: 4, max_weight: 1000 }
}
fn command(action: StoneAction, request_id: &str, expected_revision: u64) -> StoneCommand {
    StoneCommand { request_id: request_id.into(), stone_serial: 1, expected_revision, action }
}
fn change(result: Result<StonePreparation, StoneError>) -> Box<PreparedStone> {
    match result.unwrap() {
        StonePreparation::Change(plan) => plan,
        StonePreparation::Replay(_) => panic!("new operation unexpectedly replayed"),
    }
}
fn checkpoint_value(registry: &StoneRegistry) -> Value {
    serde_json::from_str(&registry.checkpoint_json().unwrap()).unwrap()
}
fn rejected_checkpoint(value: Value) {
    assert_eq!(StoneRegistry::restore(&value.to_string()), Err(StoneError::InvalidCheckpoint));
}
fn future_catalog() -> StoneCatalog {
    let mut catalog = StoneCatalog::v1();
    catalog.version = 2;
    for rule in &mut catalog.grades {
        rule.appraisal_gold = 1000;
        rule.cutting_gold = 10_000;
        for (row, basis_points) in rule.outcomes.iter_mut().zip([1000, 3000, 1000, 2000, 1500, 1500]) {
            row.basis_points = basis_points;
        }
    }
    catalog.validate().unwrap();
    catalog
}

#[test]
fn v1_distribution_and_every_interval_boundary_are_exact() {
    let catalog = StoneCatalog::v1();
    catalog.validate().unwrap();
    let expected = [
        (StoneGrade::Rough, [8000, 1000, 800, 100, 50, 50]),
        (StoneGrade::Fine, [4000, 3500, 2000, 200, 200, 100]),
        (StoneGrade::Precious, [1000, 4000, 3000, 700, 700, 600]),
    ];
    for (grade, weights) in expected {
        let frozen = catalog.freeze(grade).unwrap();
        let mut counts = BTreeMap::<StoneMaterial, u32>::new();
        for roll in 0..STONE_ROLL_SCALE {
            let output = frozen.select(roll).unwrap();
            assert_eq!(output.template_id, output.material.template_id());
            assert_eq!(output.quantity, 1);
            *counts.entry(output.material).or_default() += 1;
        }
        let mut start = 0_u16;
        for (index, (material, weight)) in StoneMaterial::ALL.into_iter().zip(weights).enumerate() {
            assert_eq!(counts[&material], u32::from(weight));
            assert_eq!(frozen.select(start).unwrap().material, material);
            assert_eq!(frozen.select(start + weight - 1).unwrap().material, material);
            let actual = minted(grade, start).record(1).unwrap().private_output().clone();
            assert_eq!(actual.material, material);
            assert_eq!(actual.template_id, [824, 826, 904, 734, 735, 736][index]);
            start += weight;
        }
        assert_eq!(start, STONE_ROLL_SCALE);
        assert_eq!(frozen.select(STONE_ROLL_SCALE), Err(StoneError::InvalidRequest));
        assert_eq!(frozen.select(u16::MAX), Err(StoneError::InvalidRequest));
    }
}

#[test]
fn canonical_catalog_rejects_changes_and_clue_conditioned_npc_arbitrage() {
    let catalog = StoneCatalog::v1();
    let mut cases = Vec::new();
    let mut invalid = catalog.clone(); invalid.version = 0; cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades.pop(); cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades.swap(0, 1); cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades[0].outcomes[0].basis_points -= 1; cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades[0].outcomes[0].template_id = 827; cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades[0].outcomes[0].quantity = 0; cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades[0].outcomes[0].quantity = 17; cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades[0].outcomes.swap(0, 1); cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades[0].appraisal_gold = 101; cases.push(invalid);
    let mut invalid = catalog.clone(); invalid.grades[0].cutting_gold = 0; cases.push(invalid);
    let mut invalid = future_catalog(); invalid.grades[0].cutting_gold = MAX_GOLD_FEE + 1; cases.push(invalid);
    let mut invalid = future_catalog(); invalid.grades[0].cutting_gold = 1; cases.push(invalid);
    for invalid in cases {
        assert_eq!(invalid.validate(), Err(StoneError::InvalidCatalog));
        assert_eq!(StoneCatalog::from_json(&serde_json::to_string(&invalid).unwrap()), Err(StoneError::InvalidCatalog));
    }
    for rule in &catalog.grades {
        for clue in [StoneClue::Metallic, StoneClue::Crystalline] {
            let rows: Vec<_> = rule.outcomes.iter().filter(|row| row.material.clue() == clue).collect();
            let weight: u64 = rows.iter().map(|row| u64::from(row.basis_points)).sum();
            let nominal_sales: u64 = rows.iter().map(|row| u64::from(row.material.nominal_price())
                * u64::from(row.quantity) * u64::from(row.basis_points)).sum();
            assert!(rule.cutting_gold * 2 * weight >= nominal_sales);
        }
    }
    let mut unknown = serde_json::to_value(catalog).unwrap(); unknown["seed"] = json!(7);
    assert_eq!(StoneCatalog::from_json(&unknown.to_string()), Err(StoneError::InvalidCatalog));
    assert_eq!(StoneCatalog::from_json(&" ".repeat(MAX_CATALOG_BYTES + 1)), Err(StoneError::InvalidCatalog));
}

#[test]
fn mint_source_request_and_carrier_are_independent_and_do_not_reroll() {
    let req = request(StoneGrade::Rough, 17, 4242);
    let registry = mint(&StoneCatalog::v1(), &req, 9999);
    let original = registry.record(17).unwrap().clone();
    assert_eq!(original.serial(), 17);
    assert_eq!(original.carrier_uid(), 4242);
    assert_eq!(registry.next_serial(), Ok(18));
    assert_eq!(registry.mint_request(&creator(), "mint-17").unwrap(), Some(&req));
    assert_eq!(registry.prepare_mint(&future_catalog(), &req, u16::MAX),
        Ok(StoneMintPreparation::Replay(original.clone())));
    assert_eq!(registry.prepare_mint(&StoneCatalog { version: 0, grades: vec![] }, &req, 0),
        Ok(StoneMintPreparation::Replay(original)));
    let mut changed = req.clone(); changed.raw_weight = 5;
    assert_eq!(registry.prepare_mint(&StoneCatalog::v1(), &changed, 0), Err(StoneError::RequestConflict));
    let mut changed = req.clone(); changed.request_id = "another-mint".into(); changed.serial = 18;
    assert_eq!(registry.prepare_mint(&StoneCatalog::v1(), &changed, 0), Err(StoneError::SourceConflict));
    let mut changed = request(StoneGrade::Rough, 18, 4242);
    assert_eq!(registry.prepare_mint(&StoneCatalog::v1(), &changed, 0), Err(StoneError::SourceConflict));
    changed.carrier_uid = 7777; changed.serial = 16;
    assert_eq!(registry.prepare_mint(&StoneCatalog::v1(), &changed, 0), Err(StoneError::InvalidRequest));
    assert_eq!(registry.checkpoint_json().unwrap(), mint(&StoneCatalog::v1(), &req, 9999).checkpoint_json().unwrap());
}

#[test]
fn mint_requires_valid_resource_grade_and_bounded_server_provenance() {
    let req = request(StoneGrade::Rough, 1, 11);
    let mut cases = Vec::new();
    let mut bad = req.clone(); bad.source_template_id = 826; cases.push(bad);
    let mut bad = req.clone(); bad.raw_template_id = 824; cases.push(bad);
    let mut bad = req.clone(); bad.raw_template_id = 0; cases.push(bad);
    let mut bad = req.clone(); bad.raw_weight = 256; cases.push(bad);
    let mut bad = req.clone(); bad.serial = 0; cases.push(bad);
    let mut bad = req.clone(); bad.carrier_uid = 0; cases.push(bad);
    let mut bad = req.clone(); bad.request_id.clear(); cases.push(bad);
    let mut bad = req.clone(); bad.request_id = "r".repeat(129); cases.push(bad);
    let mut bad = req.clone(); bad.source.source_version = "v".repeat(65); cases.push(bad);
    let mut bad = req.clone(); bad.source.event_id = "event:client".into(); cases.push(bad);
    let mut bad = req.clone(); bad.owner.character_incarnation.clear(); cases.push(bad);
    let mut bad = req.clone(); bad.owner.account_id = " account-a".into(); cases.push(bad);
    let mut bad = req.clone(); bad.binding.soul_bound_id = -2; cases.push(bad);
    for bad in cases {
        assert_eq!(StoneRegistry::default().prepare_mint(&StoneCatalog::v1(), &bad, 0), Err(StoneError::InvalidRequest));
    }
    assert_eq!(StoneRegistry::default().prepare_mint(&StoneCatalog::v1(), &req, 10_000), Err(StoneError::InvalidRequest));
}

#[test]
fn sealed_public_projection_is_identical_for_different_private_outcomes() {
    let copper = minted(StoneGrade::Rough, 0);
    let silver = minted(StoneGrade::Rough, 8000);
    assert_ne!(copper.record(1).unwrap().private_output(), silver.record(1).unwrap().private_output());
    let public = copper.record(1).unwrap().public_view();
    assert_eq!(public, silver.record(1).unwrap().public_view());
    assert_eq!(public.uid, "9007199254740993");
    assert_eq!(public.serial, "1");
    assert_eq!(public.appraisal_clue, None);
    let value = serde_json::to_value(public).unwrap();
    let keys: BTreeSet<_> = value.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, BTreeSet::from(["uid", "serial", "grade", "lifecycle", "revision",
        "catalogVersion", "appraisalGold", "cuttingGold", "possibilities", "appraisalClue"]));
    for forbidden in ["outcome", "seed", "uniformRoll", "owner", "mint", "source", "binding", "fingerprint"] {
        assert!(!value.as_object().unwrap().contains_key(forbidden));
    }
    let holder = creator();
    let copper_items = [witness(&copper, 1, &holder)];
    let silver_items = [witness(&silver, 1, &holder)];
    let appraisal = command(StoneAction::Appraise, "appraise-1", 0);
    let copper_plan = change(copper.prepare(&context(&holder, &copper_items), &appraisal));
    let silver_plan = change(silver.prepare(&context(&holder, &silver_items), &appraisal));
    assert_eq!(copper_plan.receipt, silver_plan.receipt);
    assert_eq!(copper_plan.receipt.appraisal_clue, Some(StoneClue::Metallic));
    assert_eq!(copper_plan.delivery, None);
    assert_eq!(copper_plan.consume_carrier_uid, None);
    assert_eq!(copper_plan.next_registry.record(1).unwrap().public_view(),
        silver_plan.next_registry.record(1).unwrap().public_view());
    let private_json = copper.checkpoint_json().unwrap();
    assert!(!private_json.contains("uniformRoll") && !private_json.contains("seed"));
}

#[test]
fn appraisal_of_jade_and_a_gem_reveals_only_the_same_coarse_clue() {
    let jade = minted(StoneGrade::Rough, 9000);
    let gem = minted(StoneGrade::Rough, 9800);
    let holder = creator();
    let jade_items = [witness(&jade, 1, &holder)];
    let gem_items = [witness(&gem, 1, &holder)];
    let cmd = command(StoneAction::Appraise, "appraise", 0);
    let jade_plan = change(jade.prepare(&context(&holder, &jade_items), &cmd));
    let gem_plan = change(gem.prepare(&context(&holder, &gem_items), &cmd));
    assert_eq!(jade_plan.receipt, gem_plan.receipt);
    assert_eq!(jade_plan.receipt.appraisal_clue, Some(StoneClue::Crystalline));
    assert_eq!(jade_plan.receipt.delivery, None);
    assert_eq!(jade_plan.next_registry.record(1).unwrap().public_view(),
        gem_plan.next_registry.record(1).unwrap().public_view());
}

#[test]
fn catalog_reload_and_changed_generation_policy_cannot_change_frozen_fees_or_result() {
    let registry = minted(StoneGrade::Rough, 9800);
    let original = registry.record(1).unwrap().clone();
    let catalog = future_catalog();
    let mut new_request = request(StoneGrade::Rough, 2, 2222);
    new_request.owner = owner("account-b", "character-b");
    let extended = match registry.prepare_mint(&catalog, &new_request, 9800).unwrap() {
        StoneMintPreparation::Change(plan) => plan.next_registry,
        _ => panic!("expected a new mint"),
    };
    assert_eq!(extended.record(1), Some(&original));
    assert_eq!(extended.record(2).unwrap().catalog_version(), 2);
    assert_ne!(extended.record(2).unwrap().private_output().material, original.private_output().material);
    registry.validate_successor(&extended).unwrap();
    let restored = StoneRegistry::restore(&extended.checkpoint_json().unwrap()).unwrap();
    assert_eq!(restored, extended);
    let holder = creator();
    let items = [witness(&restored, 1, &holder)];
    let appraise = change(restored.prepare(&context(&holder, &items), &command(StoneAction::Appraise, "appraise", 0)));
    assert_eq!(appraise.gold_debit, 100);
    let cut = change(appraise.next_registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "cut", 1)));
    assert_eq!(cut.gold_debit, 1500);
    assert_eq!(cut.delivery, Some(original.private_output().clone()));
    assert_eq!(cut.expected_registry_revision, 3);
    assert_eq!(cut.expected_inventory_revision, 27);
    assert_eq!(cut.expected_stone_revision, 1);
    assert_eq!(cut.consume_serial, Some(1));
    assert_eq!(cut.consume_carrier_uid, Some(original.carrier_uid()));
}

#[test]
fn current_physical_holder_can_appraise_and_cut_after_transfer() {
    let registry = minted(StoneGrade::Precious, 9999);
    let original = registry.record(1).unwrap().private_output().clone();
    let recipient = owner("account-b", "character-b");
    let items = [witness(&registry, 1, &recipient)];
    let recipient_context = context(&recipient, &items);
    assert_eq!(registry.public_view(&recipient_context).unwrap().len(), 1);
    let appraise = command(StoneAction::Appraise, "transferred-appraise", 0);
    let plan = change(registry.prepare(&recipient_context, &appraise));
    assert_eq!(plan.next_registry.record(1).unwrap().owner(), &creator());
    let next_holder = owner("account-c", "character-c");
    let next_items = [witness(&plan.next_registry, 1, &next_holder)];
    let cut = change(plan.next_registry.prepare(&context(&next_holder, &next_items), &command(StoneAction::Cut, "transferred-cut", 1)));
    assert_eq!(cut.delivery, Some(original));
    cut.next_registry.validate().unwrap();
    assert_eq!(StoneRegistry::restore(&cut.next_registry.checkpoint_json().unwrap()).unwrap(), cut.next_registry);
    let original_owner = creator();
    assert_eq!(registry.prepare(&context(&original_owner, &items), &appraise), Err(StoneError::InvalidPossession));
    let replacement = owner("account-b", "replacement-character");
    assert_eq!(registry.prepare(&context(&replacement, &items), &appraise), Err(StoneError::InvalidPossession));
    assert_eq!(plan.next_registry.query(&original_owner, &appraise).unwrap(), None);
}

#[test]
fn full_binding_bitset_and_soul_binding_are_retained_on_the_actual_output() {
    for flags in [0, 1, 0x4000, i16::MAX, i16::MIN, -1] {
        let mut req = request(StoneGrade::Precious, 1, 55);
        req.binding = StoneBinding { flags, soul_bound_id: 77 };
        let registry = mint(&StoneCatalog::v1(), &req, 9999);
        let holder = creator();
        let items = [witness(&registry, 1, &holder)];
        let plan = change(registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "bound-cut", 0)));
        let delivery = plan.delivery.unwrap();
        assert_eq!(delivery.carrier_uid, 55);
        assert_eq!(delivery.binding, req.binding);
        assert_eq!(delivery.template_id, 736);
        assert_eq!(delivery.quantity, 1);
    }
}

#[test]
fn missing_forged_locked_and_duplicate_possessions_never_change_a_registry() {
    let registry = minted(StoneGrade::Rough, 0);
    let before = registry.checkpoint_json().unwrap();
    let holder = creator();
    let item = witness(&registry, 1, &holder);
    let mut cases = vec![Vec::new()];
    let mut bad = item.clone(); bad.serial = 99; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.carrier_uid += 1; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.raw_template_id += 1; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.raw_weight += 1; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.binding.flags = 1; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.binding.soul_bound_id = 1; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.count = 2; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.count = 0; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.bag_slot = 40; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.locked = true; cases.push(vec![bad]);
    let mut bad = item.clone(); bad.eligible = false; cases.push(vec![bad]);
    cases.push(vec![item.clone(), item.clone()]);
    let mut duplicate = item.clone(); duplicate.serial = 2; duplicate.bag_slot = 1;
    cases.push(vec![item.clone(), duplicate]);
    let mut duplicate = item.clone(); duplicate.serial = 2; duplicate.carrier_uid = 2;
    cases.push(vec![item.clone(), duplicate]);
    for bad_items in cases {
        let mut bad_context = context(&holder, &bad_items);
        bad_context.free_bag_slots = 40 - bad_items.len() as u16;
        assert_eq!(registry.prepare(&bad_context, &command(StoneAction::Cut, "invalid", 0)), Err(StoneError::InvalidPossession));
        assert_eq!(registry.checkpoint_json().unwrap(), before);
    }
    let items = [item];
    let mut unauthenticated = context(&holder, &items); unauthenticated.authenticated = false;
    assert_eq!(registry.prepare(&unauthenticated, &command(StoneAction::Cut, "invalid", 0)), Err(StoneError::Unauthorized));
    for bad_capacity in [0, 81] {
        let mut bad = context(&holder, &items); bad.bag_capacity = bad_capacity;
        assert_eq!(registry.prepare(&bad, &command(StoneAction::Cut, "invalid", 0)), Err(StoneError::InvalidPossession));
    }
    let mut bad = context(&holder, &items); bad.free_bag_slots = 40;
    assert_eq!(registry.prepare(&bad, &command(StoneAction::Cut, "invalid", 0)), Err(StoneError::InvalidPossession));
    let mut locked = items.clone(); locked[0].locked = true; locked[0].eligible = false;
    assert_eq!(registry.public_view(&context(&holder, &locked)).unwrap().len(), 1);
}

#[test]
fn replay_returns_the_original_receipt_after_reload_without_gold_or_possession() {
    let registry = minted(StoneGrade::Fine, 4000);
    let holder = creator();
    let items = [witness(&registry, 1, &holder)];
    let appraise_cmd = command(StoneAction::Appraise, "appraise", 0);
    let appraisal = change(registry.prepare(&context(&holder, &items), &appraise_cmd));
    let appraisal_receipt = appraisal.receipt.clone();
    let cut_cmd = command(StoneAction::Cut, "cut", 1);
    let cut = change(appraisal.next_registry.prepare(&context(&holder, &items), &cut_cmd));
    let cut_receipt = cut.receipt.clone();
    let restored = StoneRegistry::restore(&cut.next_registry.checkpoint_json().unwrap()).unwrap();
    let mut reconnect = context(&holder, &[]);
    reconnect.gold = 0; reconnect.bag_capacity = 0; reconnect.max_weight = 0;
    assert_eq!(restored.prepare(&reconnect, &appraise_cmd), Ok(StonePreparation::Replay(appraisal_receipt)));
    assert_eq!(restored.prepare(&reconnect, &cut_cmd), Ok(StonePreparation::Replay(cut_receipt)));
    reconnect.authenticated = false;
    assert_eq!(restored.prepare(&reconnect, &cut_cmd), Err(StoneError::Unauthorized));
    let original_mint = restored.mint_request(&holder, "mint-1").unwrap().unwrap();
    let replay = restored.prepare_mint(&future_catalog(), original_mint, 0).unwrap();
    match replay {
        StoneMintPreparation::Replay(record) => {
            assert_eq!(record.lifecycle(), StoneLifecycle::Sealed);
            assert_eq!(record.revision(), 0);
            assert_eq!(record.private_output(), restored.record(1).unwrap().private_output());
        },
        _ => panic!("original mint must replay without resurrecting its carrier"),
    }
    assert_eq!(restored.record(1).unwrap().lifecycle(), StoneLifecycle::Cut);
}

#[test]
fn conflicting_requests_and_second_mutations_are_refused() {
    let registry = minted(StoneGrade::Rough, 0);
    let holder = creator();
    let items = [witness(&registry, 1, &holder)];
    let appraisal_cmd = command(StoneAction::Appraise, "shared-id", 0);
    let appraisal = change(registry.prepare(&context(&holder, &items), &appraisal_cmd));
    for changed in [command(StoneAction::Cut, "shared-id", 1), command(StoneAction::Appraise, "shared-id", 1),
        StoneCommand { stone_serial: 2, ..appraisal_cmd.clone() }]
    {
        assert_eq!(appraisal.next_registry.prepare(&context(&holder, &items), &changed), Err(StoneError::RequestConflict));
    }
    assert_eq!(appraisal.next_registry.prepare(&context(&holder, &items), &command(StoneAction::Appraise, "new-id", 1)), Err(StoneError::AlreadyAppraised));
    assert_eq!(registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "mint-1", 0)), Err(StoneError::RequestConflict));
    let mut mint_conflict = request(StoneGrade::Rough, 2, 2222); mint_conflict.request_id = "shared-id".into();
    assert_eq!(appraisal.next_registry.prepare_mint(&StoneCatalog::v1(), &mint_conflict, 0), Err(StoneError::RequestConflict));
    assert_eq!(appraisal.next_registry.mint_request(&holder, "shared-id"), Err(StoneError::RequestConflict));
    let cut = change(appraisal.next_registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "cut", 1)));
    assert_eq!(cut.next_registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "second-cut", 2)), Err(StoneError::AlreadyCut));
    assert_eq!(cut.next_registry.prepare(&context(&holder, &items), &command(StoneAction::Appraise, "after-cut", 2)), Err(StoneError::AlreadyCut));
    assert_eq!(cut.next_registry.len(), 1);
    assert_eq!(cut.next_registry.private_snapshot().len(), 1);
    assert_eq!(cut.next_registry.public_view(&context(&holder, &items)), Err(StoneError::InvalidPossession));
}

#[test]
fn failures_preserve_money_stones_and_hidden_results_before_commit() {
    let registry = minted(StoneGrade::Rough, 9000);
    let original = registry.clone();
    let holder = creator();
    let items = [witness(&registry, 1, &holder)];
    for (action, fee) in [(StoneAction::Appraise, 100), (StoneAction::Cut, 1500)] {
        let mut insufficient = context(&holder, &items); insufficient.gold = fee - 1;
        assert_eq!(registry.prepare(&insufficient, &command(action, "no-money", 0)), Err(StoneError::InsufficientGold));
        insufficient.gold = fee;
        assert_eq!(change(registry.prepare(&insufficient, &command(action, "exact-money", 0))).gold_debit, fee);
    }
    assert_eq!(registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "stale", 1)), Err(StoneError::StaleRevision));
    let mut overweight = context(&holder, &items); overweight.max_weight = 4;
    assert_eq!(registry.prepare(&overweight, &command(StoneAction::Cut, "heavy", 0)), Err(StoneError::InsufficientCapacity));
    let mut overflow = context(&holder, &items); overflow.current_weight = u64::MAX; overflow.max_weight = u64::MAX;
    assert_eq!(registry.prepare(&overflow, &command(StoneAction::Cut, "overflow", 0)), Err(StoneError::Overflow));
    let mut impossible = context(&holder, &items); impossible.current_weight = 3;
    assert_eq!(registry.prepare(&impossible, &command(StoneAction::Cut, "bad-weight", 0)), Err(StoneError::InvalidPossession));
    assert_eq!(registry, original);
    let uncommitted = command(StoneAction::Cut, "not-committed", 0);
    let prepared = change(registry.prepare(&context(&holder, &items), &uncommitted));
    assert_eq!(registry.record(1).unwrap().lifecycle(), StoneLifecycle::Sealed);
    assert_eq!(prepared.next_registry.record(1).unwrap().lifecycle(), StoneLifecycle::Cut);
    assert_eq!(registry.query(&holder, &uncommitted).unwrap(), None);
}

#[test]
fn full_bag_can_replace_the_consumed_single_stone_but_extra_roots_need_space() {
    let registry = minted(StoneGrade::Rough, 0);
    let holder = creator();
    let items = [witness(&registry, 1, &holder)];
    let mut full = context(&holder, &items); full.free_bag_slots = 0;
    let plan = change(registry.prepare(&full, &command(StoneAction::Cut, "full-single", 0)));
    assert_eq!(plan.delivery.unwrap().quantity, 1);
    let mut future = future_catalog();
    for rule in &mut future.grades { rule.outcomes[0].quantity = 2; }
    future.validate().unwrap();
    let future_registry = mint(&future, &request(StoneGrade::Rough, 1, 55), 0);
    let future_items = [witness(&future_registry, 1, &holder)];
    let mut full = context(&holder, &future_items); full.free_bag_slots = 0;
    assert_eq!(future_registry.prepare(&full, &command(StoneAction::Cut, "full-double", 0)), Err(StoneError::InsufficientCapacity));
    full.free_bag_slots = 1;
    let plan = change(future_registry.prepare(&full, &command(StoneAction::Cut, "double", 0)));
    assert_eq!(plan.delivery.unwrap().quantity, 2);
    assert_eq!(future_registry.record(1).unwrap().lifecycle(), StoneLifecycle::Sealed);
}

#[test]
fn consumed_physical_carrier_may_be_reclassified_with_a_new_serial_and_source() {
    let registry = minted(StoneGrade::Rough, 8000);
    let holder = creator();
    let items = [witness(&registry, 1, &holder)];
    let cut = change(registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "cut", 0)));
    let uid = cut.receipt.carrier_uid;
    let mut new_req = request(StoneGrade::Fine, 2, uid);
    new_req.source.event_id = "workshop-second-source".into();
    let next = match cut.next_registry.prepare_mint(&StoneCatalog::v1(), &new_req, 9999).unwrap() {
        StoneMintPreparation::Change(plan) => plan.next_registry,
        _ => panic!("the newly consumed resource is a distinct mint"),
    };
    assert_eq!(next.len(), 2);
    assert_eq!(next.record(1).unwrap().lifecycle(), StoneLifecycle::Cut);
    assert_eq!(next.record(2).unwrap().lifecycle(), StoneLifecycle::Sealed);
    assert_eq!(next.record(1).unwrap().carrier_uid(), next.record(2).unwrap().carrier_uid());
    cut.next_registry.validate_successor(&next).unwrap();
    let mut old_cmd = command(StoneAction::Cut, "old-cut", 1); old_cmd.stone_serial = 1;
    let new_items = [witness(&next, 2, &holder)];
    assert_eq!(next.prepare(&context(&holder, &new_items), &old_cmd), Err(StoneError::AlreadyCut));
}

#[test]
fn checkpoints_refuse_forged_records_receipts_and_missing_tombstones() {
    let registry = minted(StoneGrade::Rough, 0);
    let holder = creator();
    let items = [witness(&registry, 1, &holder)];
    let cut = change(registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "cut", 0)));
    let value = checkpoint_value(&cut.next_registry);
    let mut bad = value.clone(); bad["stones"][0]["outcome"]["templateId"] = json!(827); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"][0]["outcome"]["quantity"] = json!(0); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"][0]["outcome"]["carrierUid"] = json!("88"); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"][0]["outcome"]["binding"]["flags"] = json!(1); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"][0]["frozen"]["fingerprint"] = json!("sha256:forged"); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"][0]["frozen"]["rule"]["cuttingGold"] = json!(1); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"][0]["revision"] = json!("0"); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"][0]["lifecycle"] = json!("sealed"); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["history"][1]["event"]["stored"]["receipt"]["chargedGold"] = json!(0); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["history"][1]["event"]["stored"]["receipt"]["delivery"]["templateId"] = json!(1); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["history"][1]["event"]["stored"]["command"]["expectedRevision"] = json!("2"); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["history"][1]["registryRevision"] = json!("1"); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["lastSerial"] = json!("0"); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"] = json!([]); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"].as_array_mut().unwrap().push(value["stones"][0].clone()); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["stones"][0]["seed"] = json!(42); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["history"][0]["event"]["record"]["mint"]["serial"] = json!(1); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["history"][0]["event"]["record"]["mint"]["serial"] = json!("01"); rejected_checkpoint(bad);
    let mut bad = value.clone(); bad["history"][0]["event"]["record"]["mint"]["serial"] = json!("18446744073709551616"); rejected_checkpoint(bad);
    let mut bad = value; bad["history"].as_array_mut().unwrap().pop(); rejected_checkpoint(bad);
}

#[test]
fn durable_successor_comparison_detects_structurally_valid_whole_history_rewrites() {
    let original = minted(StoneGrade::Rough, 0);
    let mut rewritten = original.clone();
    let replace = |record: &mut StoneRecord| {
        record.outcome.material = StoneMaterial::Silver;
        record.outcome.template_id = 826;
    };
    replace(rewritten.stones.get_mut(&1).unwrap());
    match &mut rewritten.history[0].event {
        StoneEvent::Mint { record } => replace(record),
        _ => panic!("expected mint"),
    }
    // Private trusted restore validates structure. Issuance authenticity and the
    // original hidden result require the previous durable CAS authority.
    rewritten.validate().unwrap();
    let structurally_valid = StoneRegistry::restore(&rewritten.checkpoint_json().unwrap()).unwrap();
    assert_eq!(original.validate_successor(&structurally_valid), Err(StoneError::InvalidCheckpoint));
    assert!(!original.is_history_prefix_of(&structurally_valid));
    assert_eq!(original.validate_successor(&StoneRegistry::default()), Err(StoneError::InvalidCheckpoint));
    original.validate_successor(&original).unwrap();
    let holder = creator();
    let items = [witness(&original, 1, &holder)];
    let cut = change(original.prepare(&context(&holder, &items), &command(StoneAction::Cut, "cut", 0)));
    original.validate_successor(&cut.next_registry).unwrap();
    assert_eq!(cut.next_registry.validate_successor(&original), Err(StoneError::InvalidCheckpoint));
}

#[test]
fn exact_u64_projection_and_private_checkpoint_are_lossless_and_bounded() {
    let req = request(StoneGrade::Rough, u64::MAX, u64::MAX);
    let registry = mint(&StoneCatalog::v1(), &req, 0);
    let public = registry.record(u64::MAX).unwrap().public_view();
    assert_eq!(public.uid, "18446744073709551615");
    assert_eq!(public.serial, public.uid);
    let restored = StoneRegistry::restore(&registry.checkpoint_json().unwrap()).unwrap();
    assert_eq!(restored, registry);
    assert_eq!(restored.next_serial(), Err(StoneError::Overflow));
    let mut next = request(StoneGrade::Rough, 2, 22); next.request_id = "next".into();
    assert_eq!(restored.prepare_mint(&StoneCatalog::v1(), &next, 0), Err(StoneError::Overflow));
    assert_eq!(StoneRegistry::restore(&" ".repeat(STONE_MAX_CHECKPOINT_BYTES + 1)), Err(StoneError::InvalidCheckpoint));
    let mut value = checkpoint_value(&registry); value["schemaVersion"] = json!(2); rejected_checkpoint(value);
    let command = command(StoneAction::Cut, "exact", u64::MAX);
    let value = serde_json::to_value(command).unwrap();
    assert_eq!(value["expectedRevision"], "18446744073709551615");
    let mut value = value; value["stoneSerial"] = json!(1);
    assert!(serde_json::from_value::<StoneCommand>(value).is_err());
}

#[test]
fn bounded_registry_reserves_action_history_and_never_evicts_consumed_records() {
    let frozen = StoneCatalog::v1().freeze(StoneGrade::Rough).unwrap();
    let mut registry = StoneRegistry::default();
    for serial in 1..=STONE_MAX_RECORDS as u64 {
        let req = request(StoneGrade::Rough, serial, serial + 50_000);
        let record = StoneRecord { outcome: StoneDelivery { carrier_uid: req.carrier_uid,
            material: StoneMaterial::Copper, template_id: 824, quantity: 1, binding: req.binding },
            mint: req, frozen: frozen.clone(), lifecycle: StoneLifecycle::Sealed, revision: 0,
            appraisal_clue: None };
        registry.stones.insert(serial, record.clone());
        registry.history.push(HistoryEntry { registry_revision: serial, event: StoneEvent::Mint { record } });
    }
    registry.revision = STONE_MAX_RECORDS as u64;
    registry.last_serial = STONE_MAX_RECORDS as u64;
    registry.validate().unwrap();
    assert_eq!(registry.reserved_history().unwrap(), STONE_MAX_HISTORY);
    let next = request(StoneGrade::Rough, STONE_MAX_RECORDS as u64 + 1, 999_999);
    assert_eq!(registry.prepare_mint(&StoneCatalog::v1(), &next, 0), Err(StoneError::LedgerFull));
    let holder = creator();
    let items = [witness(&registry, 1, &holder)];
    let appraisal = change(registry.prepare(&context(&holder, &items), &command(StoneAction::Appraise, "reserved-appraise", 0)));
    assert_eq!(appraisal.next_registry.reserved_history().unwrap(), STONE_MAX_HISTORY);
    let cut = change(appraisal.next_registry.prepare(&context(&holder, &items), &command(StoneAction::Cut, "reserved-cut", 1)));
    assert_eq!(cut.next_registry.len(), STONE_MAX_RECORDS);
    assert_eq!(cut.next_registry.record(1).unwrap().lifecycle(), StoneLifecycle::Cut);
    assert_eq!(cut.next_registry.prepare_mint(&StoneCatalog::v1(), &next, 0), Err(StoneError::LedgerFull));
    let mut too_many = checkpoint_value(&registry);
    let record = too_many["stones"][0].clone(); too_many["stones"].as_array_mut().unwrap().push(record);
    rejected_checkpoint(too_many);
}

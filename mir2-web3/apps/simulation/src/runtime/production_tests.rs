//! Controlled actual Session + File-source transactions, not live NPC/game QA.
//! Each test owns a unique retained TEMP directory; no shared save or UI is used.
use super::*;
use crate::{AccountStoreTransactionFault, SimulationConfig};
use mir2_production::{JobStatus, ProductionAction};
use mir2_protocol::{ClientPacket, MirClass};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn login(config: SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into()
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.active_identity().is_some());
    session
}
struct Fixture {
    config: SimulationConfig,
    path: PathBuf,
    session: SimulationSession,
    cfg: PersonalProductionConfig,
    start: ProductionCommand,
    input: i32,
    output: i32,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let directory = root.join(format!(
            "mir2-production-p02-{label}-{}-{}",
            now_ms().unwrap(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(directory.starts_with(&root));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("accounts.json");
        let config = SimulationConfig::default().with_account_store_path(&path);
        let mut session = login(config.clone());
        // Real Crystal carriers, synthetic isolated recipe/bindings only. This
        // fixture does not enable or mislabel bundled production resources.
        let choices: Vec<_> = mir2_game_data::crystal_item_manifest()
            .items
            .into_iter()
            .filter(|template| {
                template.item_type == 13
                    && template.stack_size >= 20
                    && template.slots == 0
                    && !template.need_identify
                    && template.weight == 1
            })
            .take(2)
            .collect();
        assert_eq!(choices.len(), 2);
        let input = choices[0].item_index;
        let output = choices[1].item_index;
        {
            let mut inv = session.app.world_mut().resource_mut::<InventoryResource>();
            inv.inventory_capacity = 46;
            inv.inventory_items.clear();
            inv.belt_items.clear();
            inv.storage_items.clear();
            inv.equipment_items.clear();
            inv.reserved_item_unique_ids.clear();
            let mut item = embedded_item_state_from_template(&choices[0], ItemContainer::Bag1, 0);
            item.unique_id = 700_001;
            item.quantity = 5;
            inv.inventory_items.push(item);
        }
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 1000;
        session.save_active_character().unwrap();
        session.begin_personal_production().unwrap();
        let active = snapshot_active_character_save(session.app.world()).unwrap();
        let catalog = Catalog::from_json(r#"{
            "schema":"mir2.production-design.v0.2","productionEnabled":true,
            "sourceGates":{"gathering":"accepted isolated fixture"},
            "items":{"raw":{"name":"Fixture input","stage":"raw","family":"wood","sourceGate":"gathering","runtimeTemplateId":null},
                "processed":{"name":"Fixture output","stage":"processed","family":null,"sourceGate":null,"runtimeTemplateId":null}},
            "facilities":{"bench":{"name":"Fixture bench","bootstrap":"fixture"}},
            "recipes":{"process":{"name":"Fixture processing","facility":"bench","inputs":{"raw":3},"gold":10,"minutes":1,
                "output":"processed","amount":2,"unlockGates":[]}}
        }"#).unwrap();
        let start = ProductionCommand {
            request_id: "start-1".into(),
            facility_id: "fixture-bench".into(),
            action: ProductionAction::Start {
                recipe: "process".into(),
                catalog_version: catalog.schema.clone(),
                catalog_revision: catalog.revision().unwrap(),
                batch: 1,
            },
        };
        let cfg = PersonalProductionConfig {
            catalog,
            availability: ProductionAvailability {
                enabled: true,
                accepted_gates: ["gathering".into()].into(),
                accepted_facilities: ["bench".into()].into(),
                material_templates: [("raw".into(), input), ("processed".into(), output)].into(),
            },
            facilities: vec![PersonalProductionFacility {
                id: "fixture-bench".into(),
                kind: "bench".into(),
                map_file_name: active.map_file_name,
                position: active.position,
                radius: 3,
            }],
            ordinary_ore_purity_caps: BTreeMap::new(),
            global_uid_allocator: UserItemUidAllocator::initialize_file(
                directory.join("uids.json"),
                0,
            )
            .unwrap(),
        };
        session.configure_personal_production(cfg.clone()).unwrap();
        Self {
            config,
            path,
            session,
            cfg,
            start,
            input,
            output,
        }
    }
    fn execute(
        &mut self,
        command: ProductionCommand,
        now: u64,
    ) -> Result<ProductionDurableExecution, ProductionDurableError> {
        self.session
            .run_personal_production(command, now, |_| Ok(()))
    }
    fn durable(&self) -> CharacterSaveRecord {
        let store: AccountStore = serde_json::from_slice(&fs::read(&self.path).unwrap()).unwrap();
        store.accounts["demo"].saves[&0].clone()
    }
    fn assert_live_durable(&self) {
        let active = snapshot_active_character_save(self.session.app.world()).unwrap();
        let save = self.durable();
        assert_eq!(active.revision, save.revision);
        assert_eq!(active.gold, save.gold);
        assert_eq!(active.inventory_items_json, save.inventory_items_json);
        assert_eq!(active.belt_items_json, save.belt_items_json);
        assert_eq!(active.personal_production, save.personal_production);
        validate_character_save_record(&save).unwrap();
    }
    fn finish_command(&self, job: &str, cancel: bool) -> ProductionCommand {
        ProductionCommand {
            request_id: if cancel { "cancel-1" } else { "collect-1" }.into(),
            facility_id: self.start.facility_id.clone(),
            action: if cancel {
                ProductionAction::Cancel { job: job.into() }
            } else {
                ProductionAction::Collect { job: job.into() }
            },
        }
    }
    fn rebind_recipe(&mut self) {
        self.start.action = ProductionAction::Start {
            recipe: "process".into(),
            catalog_version: self.cfg.catalog.schema.clone(),
            catalog_revision: self.cfg.catalog.revision().unwrap(),
            batch: 1,
        };
        self.session
            .configure_personal_production(self.cfg.clone())
            .unwrap();
    }
}
#[test]
fn production_uid_floor_includes_other_character_equipment_and_whitespace_carriers() {
    let mut f = Fixture::new("uid-equipment");
    let template = mir2_game_data::crystal_item_manifest()
        .items
        .into_iter()
        .find(|template| template.item_type == 1 && template.slots == 0)
        .unwrap();
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    item.unique_id = 9_000_001;
    let equipment = super::super::equipment::equipment_state_from_item_state(
        &item,
        crate::EquipmentSlot::Weapon,
    );
    assert_eq!(equipment.user_item_unique_id, Some(9_000_001));
    let mut saved = f.durable();
    saved.character.index = 1;
    saved.character.name = "UidFixtureOther".into();
    saved.npc_purchase_journal = None;
    saved.personal_production = None;
    saved.inventory_items_json.clear();
    saved.belt_items_json.clear();
    saved.equipment_items_json = vec![format!(
        " \n {}",
        serde_json::to_string(&equipment).unwrap()
    )];
    validate_character_save_record(&saved).unwrap();
    f.config
        .commit_account_store_transaction(&["demo".into()], |store| {
            let account = store.accounts.get_mut("demo").unwrap();
            account.characters.push(saved.character.clone());
            account.saves.insert(1, saved.clone());
            Ok(())
        })
        .unwrap();
    let job = f.execute(f.start.clone(), 1000).unwrap().receipt.job_id;
    f.execute(f.finish_command(&job, false), 61_000).unwrap();
    let inv = f.session.app.world().resource::<InventoryResource>();
    assert!(inv
        .inventory_items
        .iter()
        .filter(|item| item.unique_id != 700_001)
        .all(|item| item.unique_id > 9_000_001));
    assert!(f.cfg.global_uid_allocator.issued_through().unwrap() > 9_000_001);
    f.assert_live_durable();
}
#[test]
fn production_file_start_collect_relogin_and_replay_conserve_real_inventory_and_gold() {
    let mut f = Fixture::new("collect");
    let receipt = f.execute(f.start.clone(), 1000).unwrap();
    assert_eq!(receipt.receipt.status, JobStatus::Running);
    assert_eq!(f.durable().gold, 990);
    assert_eq!(
        f.session
            .app
            .world()
            .resource::<InventoryResource>()
            .inventory_items[0]
            .quantity,
        2
    );
    f.assert_live_durable();
    let disk = fs::read(&f.path).unwrap();
    assert!(f.execute(f.start.clone(), 1001).unwrap().replayed);
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    let collect = f.finish_command(&receipt.receipt.job_id, false);
    assert!(matches!(
        f.execute(collect.clone(), 60_999),
        Err(ProductionDurableError::BeforeExecution(_))
    ));
    f.session.save_active_character().unwrap();
    f.session = login(f.config.clone());
    f.session
        .configure_personal_production(f.cfg.clone())
        .unwrap();
    let result = f.execute(collect.clone(), 61_000).unwrap();
    assert_eq!(result.receipt.status, JobStatus::Collected);
    let inv = f.session.app.world().resource::<InventoryResource>();
    let item = inv
        .inventory_items
        .iter()
        .find(|item| try_user_item_from_item_state(item).unwrap().item_index == f.output)
        .unwrap();
    assert_eq!(item.quantity, 2);
    assert!(item.unique_id > 700_001);
    assert_eq!(
        f.cfg.global_uid_allocator.issued_through().unwrap(),
        item.unique_id
    );
    f.assert_live_durable();
    let disk = fs::read(&f.path).unwrap();
    let issued = f.cfg.global_uid_allocator.issued_through().unwrap();
    assert!(f.execute(collect, 62_000).unwrap().replayed);
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    assert_eq!(f.cfg.global_uid_allocator.issued_through().unwrap(), issued);
    f.session.save_active_character().unwrap();
    f.assert_live_durable();
}
#[test]
fn production_cancel_preserves_bound_partial_origin_and_issues_new_uid() {
    let mut f = Fixture::new("refund");
    f.session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items[0]
        .soul_bound_id = Some(0);
    f.session.save_active_character().unwrap();
    let receipt = f.execute(f.start.clone(), 1000).unwrap();
    let cancel = f.finish_command(&receipt.receipt.job_id, true);
    let result = f.execute(cancel.clone(), 1001).unwrap();
    assert_eq!(result.receipt.status, JobStatus::Cancelled);
    let inv = f.session.app.world().resource::<InventoryResource>();
    assert_eq!(inv.inventory_items.len(), 2);
    assert_eq!(inv.inventory_items[0].unique_id, 700_001);
    assert_eq!(inv.inventory_items[0].quantity, 2);
    assert_eq!(inv.inventory_items[1].quantity, 1);
    assert!(inv.inventory_items[1].unique_id > 700_001);
    assert_eq!(inv.inventory_items[1].soul_bound_id, Some(0));
    assert_eq!(
        inv.inventory_items[0].durability_current,
        inv.inventory_items[1].durability_current
    );
    assert_eq!(
        try_user_item_from_item_state(&inv.inventory_items[1])
            .unwrap()
            .item_index,
        f.input
    );
    assert_eq!(f.durable().gold, 990);
    f.assert_live_durable();
    assert!(f.execute(cancel, 1002).unwrap().replayed);
}
#[test]
fn production_belt_and_bag_lots_propagate_binding_to_frozen_output() {
    let mut f = Fixture::new("belt-bound");
    {
        let mut inv = f
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        inv.inventory_items[0].quantity = 1;
        let mut belt = inv.inventory_items[0].clone();
        belt.unique_id = 700_002;
        belt.container = ItemContainer::Belt;
        belt.quantity = 2;
        belt.soul_bound_id = Some(0);
        inv.belt_items.push(belt);
    }
    f.session.save_active_character().unwrap();
    let job = f.execute(f.start.clone(), 1000).unwrap().receipt.job_id;
    let inv = f.session.app.world().resource::<InventoryResource>();
    assert!(inv.inventory_items.is_empty());
    assert!(inv.belt_items.is_empty());
    // A catalog change cannot change a previously paid output.
    f.cfg.catalog.recipes.get_mut("process").unwrap().amount = 1;
    f.cfg.catalog.recipes.get_mut("process").unwrap().gold = 500;
    f.cfg.availability.enabled = false;
    f.session
        .configure_personal_production(f.cfg.clone())
        .unwrap();
    f.execute(f.finish_command(&job, false), 61_000).unwrap();
    let inv = f.session.app.world().resource::<InventoryResource>();
    assert_eq!(inv.inventory_items.len(), 1);
    assert_eq!(inv.inventory_items[0].quantity, 2);
    assert_eq!(inv.inventory_items[0].soul_bound_id, Some(0));
    assert!(inv.inventory_items[0].unique_id > 700_002);
    assert_eq!(f.durable().gold, 990);
    f.assert_live_durable();
}
#[test]
fn production_ore_purity_is_not_stack_count_and_black_iron_is_protected() {
    let mut f = Fixture::new("ore");
    let ores = mir2_game_data::crystal_item_manifest().items;
    let ore = ores
        .iter()
        .find(|template| template.name == "IronOre")
        .expect("real ordinary IronOre");
    {
        let mut inv = f
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        let mut item = embedded_item_state_from_template(ore, ItemContainer::Bag1, 0);
        item.unique_id = 700_001;
        item.quantity = 1;
        item.durability_current = Some(6000);
        item.durability_max = Some(6000);
        inv.inventory_items = vec![item];
    }
    f.cfg
        .availability
        .material_templates
        .insert("raw".into(), ore.item_index);
    f.cfg.ordinary_ore_purity_caps.insert("raw".into(), 5);
    f.cfg
        .catalog
        .recipes
        .get_mut("process")
        .unwrap()
        .inputs
        .insert("raw".into(), 1);
    f.rebind_recipe();
    f.session.save_active_character().unwrap();
    let disk = fs::read(&f.path).unwrap();
    assert!(f.execute(f.start.clone(), 1000).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    {
        let mut inv = f
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        inv.inventory_items[0].durability_current = Some(5000);
        inv.inventory_items[0].durability_max = Some(5000);
    }
    f.session.save_active_character().unwrap();
    let job = f.execute(f.start.clone(), 1000).unwrap().receipt.job_id;
    assert!(f
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .is_empty());
    f.execute(f.finish_command(&job, true), 1001).unwrap();
    // One ore refunds floor(1/2)=0, not half its purity or upstream quantities.
    assert!(f
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .inventory_items
        .is_empty());
    let black = ores
        .iter()
        .find(|template| template.name == "BlackIronOre")
        .unwrap();
    let mut item = embedded_item_state_from_template(black, ItemContainer::Bag1, 0);
    item.unique_id = 700_003;
    item.durability_current = Some(1000);
    item.durability_max = Some(1000);
    f.session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .push(item);
    f.cfg
        .availability
        .material_templates
        .insert("raw".into(), black.item_index);
    f.rebind_recipe();
    f.start.request_id = "protected-black-iron".into();
    f.session.save_active_character().unwrap();
    let disk = fs::read(&f.path).unwrap();
    assert!(f.execute(f.start.clone(), 1002).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), disk);
}
#[test]
fn production_full_bag_rejects_whole_delivery_without_uid_or_job_loss() {
    let mut f = Fixture::new("full");
    {
        let padding = mir2_game_data::crystal_item_by_name("DemonLeather").unwrap();
        assert_eq!(padding.weight, 0);
        let mut inv = f
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        for slot in 1..40 {
            let mut item = embedded_item_state_from_template(&padding, ItemContainer::Bag1, slot);
            item.unique_id = 700_010 + u64::from(slot);
            inv.inventory_items.push(item);
        }
    }
    f.session.save_active_character().unwrap();
    let job = f.execute(f.start.clone(), 1000).unwrap().receipt.job_id;
    let collect = f.finish_command(&job, false);
    let before = fs::read(&f.path).unwrap();
    assert!(
        matches!(f.execute(collect.clone(), 61_000), Err(ProductionDurableError::BeforeExecution(detail)) if detail.contains("bag full"))
    );
    assert_eq!(fs::read(&f.path).unwrap(), before);
    assert_eq!(f.cfg.global_uid_allocator.issued_through().unwrap(), 0);
    assert_eq!(
        f.session.personal_production_jobs().unwrap()[0].status,
        JobStatus::Running
    );
    f.session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .pop()
        .unwrap();
    f.session.save_active_character().unwrap();
    assert_eq!(
        f.execute(collect, 61_001).unwrap().receipt.status,
        JobStatus::Collected
    );
    f.assert_live_durable();
}
#[test]
fn production_overweight_rejects_whole_batch_before_global_uid_issuance() {
    let mut f = Fixture::new("weight");
    f.cfg.catalog.recipes.get_mut("process").unwrap().amount = 200;
    f.rebind_recipe();
    let job = f.execute(f.start.clone(), 1000).unwrap().receipt.job_id;
    let before = fs::read(&f.path).unwrap();
    assert!(
        matches!(f.execute(f.finish_command(&job, false), 61_000), Err(ProductionDurableError::BeforeExecution(detail)) if detail.contains("overweight"))
    );
    assert_eq!(fs::read(&f.path).unwrap(), before);
    assert_eq!(f.cfg.global_uid_allocator.issued_through().unwrap(), 0);
}
#[test]
fn production_double_session_cas_and_original_request_recovery() {
    let mut f = Fixture::new("cas");
    let mut stale = login(f.config.clone());
    stale.configure_personal_production(f.cfg.clone()).unwrap();
    let receipt = f.execute(f.start.clone(), 1000).unwrap().receipt;
    assert!(
        stale
            .run_personal_production(f.start.clone(), 1001, |_| Ok(()))
            .unwrap()
            .replayed
    );
    let mut different = f.start.clone();
    different.request_id = "stale-other".into();
    assert!(
        matches!(stale.run_personal_production(different, 1002, |_| Ok(())), Err(ProductionDurableError::BeforeExecution(detail)) if detail.contains("stale"))
    );
    assert!(stale.save_active_character().is_err());
    assert_eq!(
        f.session.query_personal_production(&f.start).unwrap(),
        Some(receipt)
    );
    f.assert_live_durable();
}
#[test]
fn production_changed_request_payload_cannot_spend_twice() {
    let mut f = Fixture::new("intent");
    f.execute(f.start.clone(), 1000).unwrap();
    let before = fs::read(&f.path).unwrap();
    let mut conflict = f.start.clone();
    conflict.facility_id = "different".into();
    assert!(matches!(
        f.execute(conflict, 1001),
        Err(ProductionDurableError::BeforeExecution(_))
    ));
    assert_eq!(fs::read(&f.path).unwrap(), before);
}
#[test]
fn production_safe_inputs_exclude_rental_equipment_and_other_owner_binding() {
    for variant in 0..3 {
        let mut f = Fixture::new(&format!("protected-{variant}"));
        {
            let mut inv = f
                .session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>();
            let item = &mut inv.inventory_items[0];
            match variant {
                0 => item.rental_locked = true,
                1 => item.added_attack = 3,
                _ => item.soul_bound_id = Some(1234),
            }
        }
        let before = fs::read(&f.path).unwrap();
        assert!(matches!(
            f.execute(f.start.clone(), 1000),
            Err(ProductionDurableError::BeforeExecution(_))
        ));
        assert_eq!(fs::read(&f.path).unwrap(), before);
    }
}
#[test]
fn production_persist_failure_leaves_live_and_file_unchanged() {
    let mut f = Fixture::new("persist");
    let active = snapshot_active_character_save(f.session.app.world()).unwrap();
    let disk = fs::read(&f.path).unwrap();
    f.config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(matches!(
        f.execute(f.start.clone(), 1000),
        Err(ProductionDurableError::Unknown(_))
    ));
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    let after = snapshot_active_character_save(f.session.app.world()).unwrap();
    assert_eq!(active.gold, after.gold);
    assert_eq!(active.inventory_items_json, after.inventory_items_json);
    assert_eq!(active.personal_production, after.personal_production);
    assert_eq!(active.revision, after.revision);
    assert!(f
        .session
        .query_personal_production(&f.start)
        .unwrap()
        .is_none());
    f.execute(f.start.clone(), 1001).unwrap();
    f.assert_live_durable();
}
#[test]
fn production_unknown_file_publication_freezes_while_file_retains_original_receipt() {
    let mut f = Fixture::new("unknown");
    f.config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(matches!(
        f.execute(f.start.clone(), 1000),
        Err(ProductionDurableError::Unknown(_))
    ));
    assert!(f.session.query_personal_production(&f.start).is_err());
    assert!(f.session.save_active_character().is_err());
    // Reopening a shared cache cannot silently clear an ambiguous publication.
    let config = SimulationConfig::default().with_account_store_path(&f.path);
    assert!(config.ensure_account_store_writable().is_err());
    let durable = f.durable();
    let owner = owner_for("demo", &durable).unwrap();
    let recovered = ledger_for(&durable, &owner)
        .unwrap()
        .query(&owner, &f.start)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.status, JobStatus::Running);
    assert_eq!(f.durable().gold, 990);
    assert!(snapshot_active_character_save(f.session.app.world())
        .unwrap()
        .personal_production
        .is_none());
}
#[test]
fn production_known_postcommit_failure_keeps_receipt_and_never_reissues() {
    let mut f = Fixture::new("postcommit");
    let job = f.execute(f.start.clone(), 1000).unwrap().receipt.job_id;
    let collect = f.finish_command(&job, false);
    assert!(matches!(
        f.session
            .run_personal_production(collect.clone(), 61_000, |_| Err(
                "controlled publication aftermath".into()
            )),
        Err(ProductionDurableError::PostCommit { .. })
    ));
    let disk = fs::read(&f.path).unwrap();
    let issued = f.cfg.global_uid_allocator.issued_through().unwrap();
    assert!(f.execute(collect, 61_001).unwrap().replayed);
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    assert_eq!(f.cfg.global_uid_allocator.issued_through().unwrap(), issued);
    f.assert_live_durable();
}
#[test]
fn production_ordinary_save_and_restore_cannot_manufacture_or_rewind_jobs() {
    let mut f = Fixture::new("history");
    let original = f.durable();
    f.execute(f.start.clone(), 1000).unwrap();
    let durable = f.durable();
    let mut forged = original.clone();
    forged.personal_production = durable.personal_production.clone();
    assert!(protect_checkpoint(&mut forged, &original).is_err());
    assert!(f
        .session
        .restore_active_character_checkpoint(&original)
        .is_err());
    let mut stale = original.clone();
    assert!(protect_checkpoint(&mut stale, &durable).unwrap());
    assert_eq!(stale.personal_production, durable.personal_production);
    let mut wrong_actor = durable.clone();
    wrong_actor
        .personal_production
        .as_mut()
        .unwrap()
        .owner
        .character_id = "other-incarnation".into();
    assert!(validate_checkpoint(&wrong_actor).is_err());
    assert!(f
        .session
        .restore_active_character_checkpoint(&wrong_actor)
        .is_err());
    f.session.save_active_character().unwrap();
    f.assert_live_durable();
}
#[test]
fn production_authoritative_range_and_disabled_gates_do_not_debit() {
    let mut f = Fixture::new("range");
    let disk = fs::read(&f.path).unwrap();
    f.cfg.facilities[0].map_file_name = "not-current-map".into();
    f.session
        .configure_personal_production(f.cfg.clone())
        .unwrap();
    assert!(matches!(
        f.execute(f.start.clone(), 1000),
        Err(ProductionDurableError::BeforeExecution(_))
    ));
    f.cfg.facilities[0].map_file_name = f.durable().map_file_name;
    f.cfg.availability.enabled = false;
    f.session
        .configure_personal_production(f.cfg.clone())
        .unwrap();
    assert!(matches!(
        f.execute(f.start.clone(), 1001),
        Err(ProductionDurableError::BeforeExecution(_))
    ));
    assert_eq!(fs::read(&f.path).unwrap(), disk);
    assert!(matches!(
        f.session
            .app
            .world()
            .resource::<SessionResource>()
            .selected_character
            .as_ref()
            .unwrap()
            .class,
        MirClass::Warrior
            | MirClass::Wizard
            | MirClass::Taoist
            | MirClass::Assassin
            | MirClass::Archer
    ));
}

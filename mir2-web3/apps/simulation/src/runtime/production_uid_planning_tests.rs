//! Real Session/File production with an observable Store adapter. No PostgreSQL
//! or deployment is involved; the adapter asserts the account mutex is free.
use super::*;
use crate::{
    AccountStoreTransactionFault, SimulationConfig, UserItemUid, UserItemUidError, UserItemUidStore,
};
use mir2_production::{JobStatus, ProductionAction};
use mir2_protocol::ClientPacket;
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
type AfterIssue = Box<dyn FnOnce(u64) + Send>;
#[derive(Default)]
struct ObservedStore {
    high: AtomicU64,
    issued: Mutex<Vec<u64>>,
    account: Mutex<Option<Arc<Mutex<AccountStore>>>>,
    after_issue: Mutex<Option<AfterIssue>>,
    unlocked_checks: AtomicUsize,
    ensures: AtomicUsize,
    starting_character: AtomicBool,
}
impl ObservedStore {
    fn assert_account_unlocked(&self) {
        if let Some(account) = &*self.account.lock().unwrap() {
            let _guard = account
                .try_lock()
                .expect("UID backend must run before account locks");
            self.unlocked_checks.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn issued_ids(&self) -> Vec<u64> {
        self.issued.lock().unwrap().clone()
    }
}
impl UserItemUidStore for ObservedStore {
    fn issue_one(&self, reason: UserItemUidReason) -> Result<UserItemUid, UserItemUidError> {
        self.assert_account_unlocked();
        assert_eq!(
            reason,
            if self.starting_character.load(Ordering::SeqCst) {
                UserItemUidReason::CharacterStartItem
            } else {
                UserItemUidReason::Crafting
            }
        );
        let uid = crate::player_drop::player_drop_next_ordinary_item_uid(
            self.high.load(Ordering::SeqCst),
        )
        .unwrap();
        self.high.store(uid, Ordering::SeqCst);
        self.issued.lock().unwrap().push(uid);
        let callback = self.after_issue.lock().unwrap().take();
        if let Some(callback) = callback {
            callback(uid);
        }
        UserItemUid::new(uid)
    }
    fn issued_through(&self) -> Result<u64, UserItemUidError> {
        self.assert_account_unlocked();
        Ok(self.high.load(Ordering::SeqCst))
    }
    fn ensure_issued_through_at_least(&self, floor: u64) -> Result<u64, UserItemUidError> {
        self.assert_account_unlocked();
        self.ensures.fetch_add(1, Ordering::SeqCst);
        Ok(self.high.fetch_max(floor, Ordering::SeqCst).max(floor))
    }
}

struct Fixture {
    config: SimulationConfig,
    path: PathBuf,
    session: SimulationSession,
    store: Arc<ObservedStore>,
    start: ProductionCommand,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let directory = root.join(format!(
            "mir2-production-uid-plan-{label}-{}-{}-{}",
            std::process::id(),
            now_ms().unwrap(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(directory.starts_with(root));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("accounts.json");
        let store = Arc::new(ObservedStore::default());
        let allocator = UserItemUidAllocator::from_store(store.clone());
        let config = SimulationConfig::default()
            .with_account_store_path(&path)
            .with_item_uid_allocator(allocator.clone())
            .unwrap();
        *store.account.lock().unwrap() = Some(config.account_store.clone());
        let mut session = SimulationSession::new(config.clone());
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into()
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        // Starter migration is a distinct legitimate issuer phase. After a
        // successful start every issuance remains strictly a crafting action.
        store.starting_character.store(true, Ordering::SeqCst);
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(session.active_identity().is_some());
        store.starting_character.store(false, Ordering::SeqCst);
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
        {
            let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
            inventory.inventory_capacity = 46;
            inventory.inventory_items.clear();
            inventory.belt_items.clear();
            inventory.storage_items.clear();
            inventory.equipment_items.clear();
            inventory.reserved_item_unique_ids.clear();
            let mut item = embedded_item_state_from_template(&choices[0], ItemContainer::Bag1, 0);
            item.unique_id = 700_001;
            item.quantity = 5;
            inventory.inventory_items.push(item);
        }
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 1000;
        session.save_active_character().unwrap();
        session.begin_personal_production().unwrap();
        let active = snapshot_active_character_save(session.app.world()).unwrap();
        let catalog=Catalog::from_json(r#"{
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
        session
            .configure_personal_production(PersonalProductionConfig {
                catalog,
                availability: ProductionAvailability {
                    enabled: true,
                    accepted_gates: ["gathering".into()].into(),
                    accepted_facilities: ["bench".into()].into(),
                    material_templates: [
                        ("raw".into(), choices[0].item_index),
                        ("processed".into(), choices[1].item_index),
                    ]
                    .into(),
                },
                facilities: vec![PersonalProductionFacility {
                    id: "fixture-bench".into(),
                    kind: "bench".into(),
                    map_file_name: active.map_file_name,
                    position: active.position,
                    radius: 3,
                }],
                ordinary_ore_purity_caps: BTreeMap::new(),
                global_uid_allocator: allocator,
            })
            .unwrap();
        Self {
            config,
            path,
            session,
            store,
            start,
        }
    }
    fn start_job(&mut self) -> ProductionCommand {
        let job = self
            .session
            .run_personal_production(self.start.clone(), 1000, |_| Ok(()))
            .unwrap()
            .receipt
            .job_id;
        ProductionCommand {
            request_id: "collect-1".into(),
            facility_id: self.start.facility_id.clone(),
            action: ProductionAction::Collect { job },
        }
    }
    fn durable(&self) -> CharacterSaveRecord {
        let store: AccountStore = serde_json::from_slice(&fs::read(&self.path).unwrap()).unwrap();
        store.accounts["demo"].saves[&0].clone()
    }
    fn assert_source_unchanged(&self, before: &CharacterSaveRecord, bytes: &[u8]) {
        assert_eq!(fs::read(&self.path).unwrap(), bytes);
        let active = snapshot_active_character_save(self.session.app.world()).unwrap();
        assert_eq!(active.gold, before.gold);
        assert_eq!(active.inventory_items_json, before.inventory_items_json);
        assert_eq!(active.belt_items_json, before.belt_items_json);
        assert_eq!(active.personal_production, before.personal_production);
        assert_eq!(active.revision, before.revision);
    }
}
fn gained_uid(execution: &ProductionDurableExecution) -> u64 {
    execution
        .packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::GainedItem { item } => Some(item.unique_id),
            _ => None,
        })
        .unwrap()
}

#[test]
fn production_uid_backend_runs_without_account_lock_and_duplicate_query_never_mints() {
    let mut fixture = Fixture::new("unlocked");
    let collect = fixture.start_job();
    let previous = fixture.store.issued_ids();
    let checks = fixture.store.unlocked_checks.load(Ordering::SeqCst);
    let execution = fixture
        .session
        .run_personal_production(collect.clone(), 61_000, |_| Ok(()))
        .unwrap();
    assert!(!execution.replayed);
    assert_eq!(fixture.store.issued_ids().len(), previous.len() + 1);
    assert!(fixture.store.unlocked_checks.load(Ordering::SeqCst) >= checks + 2);
    assert_eq!(
        gained_uid(&execution),
        *fixture.store.issued_ids().last().unwrap()
    );
    let ids = fixture.store.issued_ids();
    let ensures = fixture.store.ensures.load(Ordering::SeqCst);
    assert_eq!(
        fixture.session.query_personal_production(&collect).unwrap(),
        Some(execution.receipt.clone())
    );
    let duplicate = fixture
        .session
        .run_personal_production(collect, 61_001, |_| {
            panic!("query replay must not postprocess")
        })
        .unwrap();
    assert!(duplicate.replayed && duplicate.packets.is_empty());
    assert_eq!(fixture.store.issued_ids(), ids);
    assert_eq!(fixture.store.ensures.load(Ordering::SeqCst), ensures);
}

#[test]
fn production_uid_history_growth_after_reservation_rejects_then_retires_the_colliding_uid() {
    let mut fixture = Fixture::new("history-growth");
    let collect = fixture.start_job();
    let before = fixture.durable();
    let bytes = fs::read(&fixture.path).unwrap();
    let account_store = fixture.config.account_store.clone();
    let mut external_save = before.clone();
    external_save.character.index = 71;
    external_save.character.name = "ExternalUID".into();
    external_save.npc_purchase_journal = None;
    external_save.personal_production = None;
    external_save.belt_items_json.clear();
    external_save.storage_items_json.clear();
    external_save.equipment_items_json.clear();
    let mut external_item: ItemState =
        serde_json::from_str(&external_save.inventory_items_json[0]).unwrap();
    // Simulate a separately refreshed account appearing after the reservation,
    // including an exact collision. This is a controlled cache race, not PG.
    *fixture.store.after_issue.lock().unwrap() = Some(Box::new(move |uid| {
        external_item.unique_id = uid;
        external_save.inventory_items_json = vec![serde_json::to_string(&external_item).unwrap()];
        validate_character_save_record(&external_save).unwrap();
        let mut account = crate::config::AccountRecord::new(external_save.character.clone());
        account
            .saves
            .insert(external_save.character.index, external_save);
        account_store
            .lock()
            .unwrap()
            .accounts
            .insert("external-uid".into(), account);
    }));
    assert!(
        matches!(fixture.session.run_personal_production(collect.clone(),61_000,|_|Ok(())),
        Err(ProductionDurableError::BeforeExecution(detail)) if detail.contains("UID history changed"))
    );
    fixture.assert_source_unchanged(&before, &bytes);
    let rejected = *fixture.store.issued_ids().last().unwrap();
    assert!(fixture
        .session
        .query_personal_production(&collect)
        .unwrap()
        .is_none());
    let execution = fixture
        .session
        .run_personal_production(collect, 61_001, |_| Ok(()))
        .unwrap();
    assert!(gained_uid(&execution) > rejected);
    assert_eq!(
        fixture
            .store
            .issued_ids()
            .iter()
            .filter(|uid| **uid == rejected)
            .count(),
        1
    );
}

#[test]
fn production_uid_failed_source_publication_burns_reservation_without_asset_debit() {
    let mut fixture = Fixture::new("persist-failure");
    let collect = fixture.start_job();
    let before = fixture.durable();
    let bytes = fs::read(&fixture.path).unwrap();
    fixture
        .config
        .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(matches!(
        fixture
            .session
            .run_personal_production(collect.clone(), 61_000, |_| Ok(())),
        Err(ProductionDurableError::Unknown(_))
    ));
    fixture.assert_source_unchanged(&before, &bytes);
    let rejected = *fixture.store.issued_ids().last().unwrap();
    assert_eq!(
        fixture.session.personal_production_jobs().unwrap()[0].status,
        JobStatus::Running
    );
    let execution = fixture
        .session
        .run_personal_production(collect, 61_001, |_| Ok(()))
        .unwrap();
    assert!(gained_uid(&execution) > rejected);
    assert_eq!(
        fixture
            .store
            .issued_ids()
            .iter()
            .filter(|uid| **uid == rejected)
            .count(),
        1
    );
}

#[test]
fn production_uid_failed_capacity_plan_does_not_mint_and_postcommit_replay_never_reissues() {
    let mut fixture = Fixture::new("capacity-postcommit");
    let padding = mir2_game_data::crystal_item_by_name("DemonLeather").unwrap();
    assert_eq!(padding.weight, 0);
    {
        let mut inventory = fixture
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>();
        for slot in 1..40 {
            let mut item = embedded_item_state_from_template(&padding, ItemContainer::Bag1, slot);
            item.unique_id = 800_000 + u64::from(slot);
            inventory.inventory_items.push(item);
        }
    }
    fixture.session.save_active_character().unwrap();
    let collect = fixture.start_job();
    let before = fixture.durable();
    let bytes = fs::read(&fixture.path).unwrap();
    let issued = fixture.store.issued_ids();
    let ensures = fixture.store.ensures.load(Ordering::SeqCst);
    assert!(
        matches!(fixture.session.run_personal_production(collect.clone(),61_000,|_|Ok(())),
        Err(ProductionDurableError::BeforeExecution(detail)) if detail.contains("bag full"))
    );
    fixture.assert_source_unchanged(&before, &bytes);
    assert_eq!(fixture.store.issued_ids(), issued);
    assert_eq!(fixture.store.ensures.load(Ordering::SeqCst), ensures);
    fixture
        .session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .pop()
        .unwrap();
    fixture.session.save_active_character().unwrap();
    assert!(matches!(
        fixture
            .session
            .run_personal_production(collect.clone(), 61_001, |_| Err(
                "controlled aftermath".into()
            )),
        Err(ProductionDurableError::PostCommit { .. })
    ));
    let after = fixture.store.issued_ids();
    assert_eq!(after.len(), issued.len() + 1);
    let bytes = fs::read(&fixture.path).unwrap();
    assert!(
        fixture
            .session
            .run_personal_production(collect, 61_002, |_| Ok(()))
            .unwrap()
            .replayed
    );
    assert_eq!(fixture.store.issued_ids(), after);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn production_uid_history_preserves_running_collected_and_cancelled_debits() {
    for status in [
        JobStatus::Running,
        JobStatus::Collected,
        JobStatus::Cancelled,
    ] {
        let mut fixture = Fixture::new("typed-retired-history");
        let collect = fixture.start_job();
        if status != JobStatus::Running {
            let terminal = if status == JobStatus::Cancelled {
                let ProductionAction::Collect { job } = collect.action else {
                    unreachable!()
                };
                ProductionCommand {
                    request_id: "cancel-1".into(),
                    facility_id: collect.facility_id,
                    action: ProductionAction::Cancel { job },
                }
            } else {
                collect
            };
            fixture
                .session
                .run_personal_production(
                    terminal,
                    if status == JobStatus::Cancelled {
                        1001
                    } else {
                        61_000
                    },
                    |_| Ok(()),
                )
                .unwrap();
        }
        let mut save = fixture.durable();
        assert_eq!(
            save.personal_production
                .as_ref()
                .unwrap()
                .ledger()
                .unwrap()
                .jobs_for(&owner_for("demo", &save).unwrap())
                .next()
                .unwrap()
                .status,
            status
        );
        // This history-only copy has no live owner for the spent input. Its
        // persisted ledger must still protect the original debit identity.
        save.inventory_items_json.clear();
        save.belt_items_json.clear();
        save.storage_items_json.clear();
        save.equipment_items_json.clear();
        save.equipment_items_explicit_empty = true;
        let before = fixture.store.issued_ids();
        let mut ids = Vec::new();
        visit_historical_item_uids("demo", &save, &mut |uid| ids.push(uid)).unwrap();
        assert!(!ids.is_empty());
        assert!(ids.iter().all(|uid| *uid == 700_001));
        assert_eq!(
            super::super::item_uid_issuance::historical_uid_floor(
                super::super::item_uid_issuance::UidHistoryRoot::Save {
                    account_id: "demo",
                    save: &save,
                }
            )
            .unwrap(),
            700_001
        );
        assert_eq!(fixture.store.issued_ids(), before);
    }
}

#[test]
fn production_uid_history_requires_actual_owner_and_valid_ledger_before_visiting() {
    let mut fixture = Fixture::new("typed-history-binding");
    fixture.start_job();
    let save = fixture.durable();
    let mut ids = Vec::new();
    assert!(
        visit_historical_item_uids("another-account", &save, &mut |uid| ids.push(uid)).is_err()
    );
    assert!(ids.is_empty());
    let mut invalid = save.clone();
    let state = invalid.personal_production.as_mut().unwrap();
    let mut checkpoint: serde_json::Value = serde_json::from_str(&state.ledger_json).unwrap();
    for job in checkpoint["jobs"].as_object_mut().unwrap().values_mut() {
        job["catalog_version"] = serde_json::json!("unsupported-production-version");
    }
    state.ledger_json = serde_json::to_string(&checkpoint).unwrap();
    assert!(visit_historical_item_uids("demo", &invalid, &mut |uid| ids.push(uid)).is_err());
    assert!(ids.is_empty());
    let mut absent = save;
    absent.personal_production = None;
    visit_historical_item_uids("another-account", &absent, &mut |uid| ids.push(uid)).unwrap();
    assert!(ids.is_empty(), "an absent ledger cannot synthesize history");
}

#[test]
fn production_uid_opaque_json_mail_does_not_exhaust_output_authority() {
    let mut fixture = Fixture::new("typed-opaque-text");
    let mut collect = fixture.start_job();
    let active = fixture.durable();
    crate::deliver_stage5_system_mail(
        &fixture.config,
        crate::Stage5MailDelivery {
            target_kind: crate::Stage5MailTargetKind::Character,
            target_id: active.character.name,
            from: "ordinary-text-fixture".into(),
            subject: format!("{{\"uid\":{}}}", u64::MAX),
            body: format!(
                "{{\"unique_id\":{},\"issuedThrough\":{}}}",
                u64::MAX,
                u64::MAX
            ),
            gold: 0,
            items: Vec::new(),
        },
    )
    .unwrap();
    let before = fixture.store.issued_ids();
    let high = fixture.store.high.load(Ordering::SeqCst);
    collect.request_id = u64::MAX.to_string();
    let execution = fixture
        .session
        .run_personal_production(collect, 61_000, |_| Ok(()))
        .unwrap();
    assert_eq!(fixture.store.issued_ids().len(), before.len() + 1);
    assert!(gained_uid(&execution) > high);
    assert!(gained_uid(&execution) < u64::MAX);
    let systems: crate::Stage5SystemsState =
        serde_json::from_str(fixture.durable().stage5_systems_json.as_deref().unwrap()).unwrap();
    assert!(systems.mail[0].body.contains(&u64::MAX.to_string()));
}

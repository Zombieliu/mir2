use super::*;
use crate::{UserItemUid, UserItemUidError, UserItemUidStore};
use mir2_protocol::{MirClass, MirGender};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};

fn character(name: &str) -> CharacterRecord {
    CharacterRecord {
        index: 0,
        name: name.into(),
        level: 1,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    }
}

struct LockProbe {
    source: SimulationConfig,
    floor: AtomicU64,
    issued: AtomicUsize,
    ensures: AtomicUsize,
    fail_after: Option<usize>,
    change_save_on_issue: bool,
}
impl UserItemUidStore for LockProbe {
    fn issue_one(&self, _: UserItemUidReason) -> Result<UserItemUid, UserItemUidError> {
        assert!(
            self.source.account_store.try_lock().is_ok(),
            "issuance under account lock"
        );
        let n = self.issued.fetch_add(1, Ordering::SeqCst);
        if self.fail_after.is_some_and(|limit| n >= limit) {
            return Err(UserItemUidError::DatabaseAuthorityUnavailable {
                operation: "starter test refusal",
            });
        }
        if self.change_save_on_issue && n == 0 {
            self.source
                .account_store
                .lock()
                .unwrap()
                .accounts
                .get_mut("starter-a")
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap()
                .gold = 777;
        }
        UserItemUid::new(self.floor.fetch_add(1, Ordering::SeqCst) + 1)
    }
    fn issued_through(&self) -> Result<u64, UserItemUidError> {
        Ok(self.floor.load(Ordering::SeqCst))
    }
    fn ensure_issued_through_at_least(&self, floor: u64) -> Result<u64, UserItemUidError> {
        self.ensures.fetch_add(1, Ordering::SeqCst);
        assert!(
            self.source.account_store.try_lock().is_ok(),
            "floor adjustment under account lock"
        );
        self.floor.fetch_max(floor, Ordering::SeqCst);
        self.issued_through()
    }
}

fn mock(fail_after: Option<usize>, change_save: bool) -> (SimulationConfig, Arc<LockProbe>) {
    let mut config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        store
            .accounts
            .insert("starter-a".into(), AccountRecord::empty());
        store
            .accounts
            .insert("starter-b".into(), AccountRecord::empty());
    }
    let probe = Arc::new(LockProbe {
        source: config.clone(),
        floor: AtomicU64::new(900_000),
        issued: AtomicUsize::new(0),
        ensures: AtomicUsize::new(0),
        fail_after,
        change_save_on_issue: change_save,
    });
    config.item_uid_issuance =
        ItemUidIssuance::Durable(UserItemUidAllocator::from_store(probe.clone()));
    (config, probe)
}

fn ids(save: &CharacterSaveRecord) -> BTreeSet<u64> {
    let decoded = decode_and_validate_character_save(save).unwrap();
    let mut ids = BTreeSet::new();
    for item in decoded
        .inventory_items
        .iter()
        .chain(&decoded.belt_items)
        .chain(&decoded.storage_items)
    {
        require_ids(&try_user_item_from_item_state(item).unwrap(), &mut ids).unwrap();
    }
    for item in decoded.equipment_items {
        require_ids(
            &try_user_item_from_item_state(&item_state_from_equipment_state(
                item.clone(),
                ItemContainer::Bag1,
                item.slot as u8,
            ))
            .unwrap(),
            &mut ids,
        )
        .unwrap();
    }
    ids
}

#[test]
fn starter_uid_planning_preallocates_outside_account_lock_and_cross_account_unique() {
    let (config, probe) = mock(None, false);
    let first = add_character_to_account(&config, "starter-a", character("StarterOne")).unwrap();
    let second = add_character_to_account(&config, "starter-b", character("StarterTwo")).unwrap();
    let store = config.account_store.lock().unwrap();
    let first_ids = ids(&store.accounts["starter-a"].saves[&first.index]);
    let second_ids = ids(&store.accounts["starter-b"].saves[&second.index]);
    assert!(!first_ids.is_empty());
    assert!(!second_ids.is_empty());
    assert!(first_ids.is_disjoint(&second_ids));
    assert_eq!(
        first_ids.len() + second_ids.len(),
        probe.issued.load(Ordering::SeqCst)
    );
}

#[test]
fn starter_uid_planning_failed_issuance_does_not_publish_character_or_restart_uid() {
    let (config, probe) = mock(Some(1), false);
    let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    assert!(add_character_to_account(&config, "starter-a", character("RejectedStarter")).is_err());
    assert_eq!(
        before,
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
    );
    assert!(
        probe.floor.load(Ordering::SeqCst) > 900_000,
        "the first issued identity is retired"
    );
}

#[test]
fn starter_uid_planning_start_migration_is_source_cas_and_idempotent() {
    let (config, probe) = mock(None, false);
    let mut old = crystal_new_character_save(&config, character("LegacyStarter"));
    old.character.index = 0;
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("starter-a").unwrap();
        account.characters.push(old.character.clone());
        account.saves.insert(0, old);
    }
    let first = character_save_for_start(&config, "starter-a", 0)
        .unwrap()
        .unwrap();
    validate_save_identities(&first).unwrap();
    let count = probe.issued.load(Ordering::SeqCst);
    assert!(count > 0);
    let second = character_save_for_start(&config, "starter-a", 0)
        .unwrap()
        .unwrap();
    assert!(same_character_save(&first, &second).unwrap());
    assert_eq!(count, probe.issued.load(Ordering::SeqCst));
}

#[test]
fn starter_uid_planning_start_cas_rejects_source_change_after_reservation() {
    let (config, probe) = mock(None, true);
    let old = crystal_new_character_save(&config, character("ChangingStarter"));
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("starter-a").unwrap();
        account.characters.push(old.character.clone());
        account.saves.insert(0, old);
    }
    assert!(character_save_for_start(&config, "starter-a", 0)
        .unwrap_err()
        .contains("changed after UID planning"));
    let store = config.account_store.lock().unwrap();
    assert_eq!(store.accounts["starter-a"].saves[&0].gold, 777);
    assert!(probe.issued.load(Ordering::SeqCst) > 0);
}

#[test]
fn starter_uid_planning_file_source_failure_burns_ids_and_reload_keeps_exact_save() {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "mir2-starter-uids-{}-{}",
        std::process::id(),
        unix_now_ms(),
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("accounts.json");
    let uid_path = root.join("uids.json");
    let allocator = UserItemUidAllocator::initialize_file(&uid_path, 1_000_000).unwrap();
    let config = SimulationConfig::default()
        .with_account_store_path(&path)
        .with_item_uid_allocator(allocator.clone())
        .unwrap();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert("starter-a".into(), AccountRecord::empty());
    config.save_account_store().unwrap();
    let before = allocator.issued_through().unwrap();
    config
        .inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(add_character_to_account(&config, "starter-a", character("FailFileStarter")).is_err());
    let burned = allocator.issued_through().unwrap();
    assert!(burned > before);
    assert!(config.account_store.lock().unwrap().accounts["starter-a"]
        .characters
        .is_empty());
    let added =
        add_character_to_account(&config, "starter-a", character("SavedFileStarter")).unwrap();
    let saved =
        config.account_store.lock().unwrap().accounts["starter-a"].saves[&added.index].clone();
    assert!(ids(&saved).iter().all(|uid| *uid > burned));
    let reopened = UserItemUidAllocator::open_file(&uid_path).unwrap();
    assert_eq!(
        reopened.issued_through().unwrap(),
        allocator.issued_through().unwrap()
    );
    let persisted: crate::AccountStore =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(
        same_character_save(&saved, &persisted.accounts["starter-a"].saves[&added.index]).unwrap()
    );
}

#[test]
fn starter_uid_planning_nested_fresh_tree_and_equipment_preserve_attributes() {
    let (config, _) = mock(None, false);
    let allocator = config.server_item_uid_allocator().unwrap();
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let child_template = crystal_item_by_name("BronzeBell").unwrap();
    let mut root = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    let mut child = embedded_item_state_from_template(&child_template, ItemContainer::Bag1, 1);
    child.unique_id = 123;
    child.attack = 19;
    child.description = "retained child".into();
    root.unique_id = 122;
    root.socket_slots = 2;
    root.socketed.push(child);
    root.attack = 42;
    root.description = "retained starter attributes".into();
    let before = complete_item_tree(&root).unwrap();
    assign_item(&allocator, &mut root).unwrap();
    let after = try_user_item_from_item_state(&root).unwrap();
    assert_ne!(before.unique_id, after.unique_id);
    assert_ne!(
        before.slots[1].as_ref().unwrap().unique_id,
        after.slots[1].as_ref().unwrap().unique_id
    );
    assert_eq!(root.attack, 42);
    assert_eq!(root.socketed[0].attack, 19);
    assert_eq!(root.socketed[0].description, "retained child");
    let mut normalized = after.clone();
    normalized.unique_id = before.unique_id;
    normalized.slots[1].as_mut().unwrap().unique_id = before.slots[1].as_ref().unwrap().unique_id;
    assert_eq!(normalized, before);

    let mut equipment = super::super::super::equipment::equipment_state_from_item_state(
        &root,
        crate::EquipmentSlot::Helmet,
    );
    equipment.shape = Some(999);
    equipment.attack = 314;
    equipment.added_luck = 11;
    let old_uid = equipment.user_item_unique_id;
    let mut save = crystal_new_character_save(&config, character("NestedEquippedStarter"));
    save.equipment_items_json = encode_state_vec(&[equipment]);
    prepare_new_character(&config, "starter-a", &mut save).unwrap();
    let equipped = decode_saved_equipment_states("equipment", &save.equipment_items_json).unwrap();
    assert_ne!(equipped[0].user_item_unique_id, old_uid);
    assert_eq!(equipped[0].shape, Some(999));
    assert_eq!(equipped[0].attack, 314);
    assert_eq!(equipped[0].added_luck, 11);
    assert_eq!(equipped[0].socketed[0].attack, 19);
}

#[test]
fn starter_uid_planning_same_template_zero_socket_children_each_receive_one_identity() {
    let (config, probe) = mock(None, false);
    let allocator = config.server_item_uid_allocator().unwrap();
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let child_template = crystal_item_by_name("BronzeBell").unwrap();
    let mut root = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    root.socket_slots = 3;
    root.socketed = vec![
        embedded_item_state_from_template(&child_template, ItemContainer::Bag1, 0),
        embedded_item_state_from_template(&child_template, ItemContainer::Bag1, 2),
    ];
    assert!(entirely_unassigned_legacy_tree(&root));
    assign_item(&allocator, &mut root).unwrap();
    let wire = try_user_item_from_item_state(&root).unwrap();
    assert_eq!(wire.slots.len(), 3);
    assert!(wire.slots[1].is_none());
    let first = wire.slots[0].as_ref().unwrap();
    let second = wire.slots[2].as_ref().unwrap();
    assert_eq!(first.item_index, child_template.item_index);
    assert_eq!(second.item_index, child_template.item_index);
    assert_eq!(root.socketed.len(), 2);
    assert_eq!(root.socketed[0].unique_id, first.unique_id);
    assert_eq!(root.socketed[1].unique_id, second.unique_id);
    assert_eq!(
        root.socketed[0]
            .user_item_metadata
            .as_ref()
            .unwrap()
            .captured_socket_position,
        Some(0)
    );
    assert_eq!(
        root.socketed[1]
            .user_item_metadata
            .as_ref()
            .unwrap()
            .captured_socket_position,
        Some(2)
    );
    assert!(root.user_item_metadata.as_ref().unwrap().slots.is_empty());
    let mut ids = BTreeSet::new();
    require_ids(&wire, &mut ids).unwrap();
    assert_eq!(ids.len(), 3);
    assert_eq!(probe.issued.load(Ordering::SeqCst), 3);
}

#[test]
fn starter_uid_planning_keeps_class_gender_equipment_loadouts() {
    let (config, _) = mock(None, false);
    for class in [
        MirClass::Warrior,
        MirClass::Wizard,
        MirClass::Taoist,
        MirClass::Assassin,
        MirClass::Archer,
    ] {
        for gender in [MirGender::Male, MirGender::Female] {
            let mut person = character("ClassGenderStarter");
            person.class = class;
            person.gender = gender;
            let mut save = crystal_new_character_save(&config, person);
            let before =
                decode_saved_equipment_states("equipment", &save.equipment_items_json).unwrap();
            prepare_new_character(&config, "starter-a", &mut save).unwrap();
            let after =
                decode_saved_equipment_states("equipment", &save.equipment_items_json).unwrap();
            assert_eq!(before.len(), after.len());
            for (old, new) in before.iter().zip(&after) {
                assert_eq!(old.key, new.key);
                assert_eq!(old.slot, new.slot);
                assert_eq!(old.shape, new.shape);
                assert_eq!(old.attack, new.attack);
                assert_eq!(old.defence, new.defence);
                assert_eq!(old.durability_current, new.durability_current);
                assert!(new.user_item_unique_id.is_some_and(|uid| uid > 0));
            }
        }
    }
}

#[test]
fn starter_uid_planning_restore_gate_rejects_zero_duplicate_and_missing_equipment_id() {
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let mut item = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
    inventory.inventory_items.push(item.clone());
    assert!(validate_inventory_identities(&inventory).is_err());
    item.unique_id = 55;
    inventory.inventory_items = vec![item.clone(), item.clone()];
    assert!(validate_inventory_identities(&inventory).is_err());
    inventory.inventory_items = vec![item];
    inventory.reserved_item_unique_ids.insert(0); // exact held zero is not rewritten
    validate_inventory_identities(&inventory).unwrap();
    inventory.equipment_items =
        seed_equipment_items_for_character(MirClass::Warrior, MirGender::Male);
    assert!(validate_inventory_identities(&inventory).is_err());
}

#[test]
fn starter_uid_planning_mixed_zero_parent_with_assigned_child_is_rejected() {
    let (config, probe) = mock(None, false);
    let mut save = crystal_new_character_save(&config, character("MixedStarter"));
    save.equipment_items_json.clear();
    save.equipment_items_explicit_empty = true;
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let child_template = crystal_item_by_name("BronzeBell").unwrap();
    let mut root = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    let mut child = embedded_item_state_from_template(&child_template, ItemContainer::Bag1, 1);
    child.unique_id = 998;
    root.socket_slots = 2;
    root.socketed.push(child);
    save.inventory_items_json = encode_state_vec(&[root]);
    assert!(prepare_character_start(&config, "starter-a", &mut save)
        .unwrap_err()
        .contains("already assigned/exact child"));
    assert_eq!(probe.issued.load(Ordering::SeqCst), 0);
}

#[test]
fn starter_uid_planning_exact_zero_is_not_reidentified_by_controlled_legacy_start() {
    let (config, probe) = mock(None, false);
    let mut save = crystal_new_character_save(&config, character("ExactZeroStarter"));
    save.equipment_items_json.clear();
    save.equipment_items_explicit_empty = true;
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let plain = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    let mut wire = try_user_item_from_item_state(&plain).unwrap();
    wire.unique_id = 0;
    let exact = try_item_state_from_user_item(plain, &wire).unwrap();
    save.inventory_items_json = encode_state_vec(&[exact]);
    assert!(prepare_character_start(&config, "starter-a", &mut save).is_err());
    assert_eq!(probe.issued.load(Ordering::SeqCst), 0);
}

#[test]
fn starter_uid_planning_unavailable_and_fenced_cannot_create_or_repair() {
    for policy in [ItemUidIssuance::Unavailable, ItemUidIssuance::Fenced] {
        let mut config = SimulationConfig::default();
        config.item_uid_issuance = policy;
        let mut save = crystal_new_character_save(&config, character("NoMintStarter"));
        assert!(prepare_new_character(&config, "starter-a", &mut save).is_err());
        assert!(prepare_character_start(&config, "starter-a", &mut save).is_err());
    }
}

fn empty_owned_save(config: &SimulationConfig, label: &str) -> CharacterSaveRecord {
    let mut save = crystal_new_character_save(config, character(label));
    normalize_legacy_default_vitals(&mut save);
    save.equipment_items_json.clear();
    save.equipment_items_explicit_empty = true;
    save
}

fn assert_no_allocator_writes(probe: &LockProbe) {
    assert_eq!(
        probe.ensures.load(Ordering::SeqCst),
        0,
        "bad input must not raise the real floor"
    );
    assert_eq!(
        probe.issued.load(Ordering::SeqCst),
        0,
        "bad input must not issue real identities"
    );
    assert_eq!(probe.floor.load(Ordering::SeqCst), 900_000);
}

#[test]
fn starter_uid_planning_duplicate_max_history_rejected_without_real_floor_update() {
    let (config, probe) = mock(None, false);
    let mut save = empty_owned_save(&config, "DuplicateMaxStarter");
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let mut first = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    first.unique_id = u64::MAX;
    let mut duplicate = first.clone();
    duplicate.slot = 1;
    save.inventory_items_json = encode_state_vec(&[first, duplicate]);
    let before = serde_json::to_value(&save).unwrap();
    assert!(prepare_character_start(&config, "starter-a", &mut save).is_err());
    assert_eq!(before, serde_json::to_value(&save).unwrap());
    assert_no_allocator_writes(&probe);
}

#[test]
fn starter_uid_planning_invalid_max_template_rejected_without_real_floor_update() {
    let (config, probe) = mock(None, false);
    let mut save = empty_owned_save(&config, "InvalidMaxStarter");
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let mut invalid = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    invalid.unique_id = u64::MAX;
    invalid.key = "unknown-starter-template".into();
    save.inventory_items_json = encode_state_vec(&[invalid]);
    assert!(prepare_character_start(&config, "starter-a", &mut save).is_err());
    assert_no_allocator_writes(&probe);
}

#[test]
fn starter_uid_planning_late_invalid_fresh_tree_prevents_all_real_reservations() {
    let (config, probe) = mock(None, false);
    let mut save = empty_owned_save(&config, "LateInvalidStarter");
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let child_template = crystal_item_by_name("BronzeBell").unwrap();
    let first = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    let mut second = first.clone();
    second.slot = 1;
    second.socket_slots = 1;
    let child = embedded_item_state_from_template(&child_template, ItemContainer::Bag1, 2);
    second.socketed.push(child); // a later fresh root has an out-of-width child
    save.inventory_items_json = encode_state_vec(&[first, second]);
    assert!(prepare_character_start(&config, "starter-a", &mut save).is_err());
    assert_no_allocator_writes(&probe);
}

#[test]
fn starter_uid_planning_private_preflight_never_installs_or_issues_temporary_ids() {
    let (config, probe) = mock(None, false);
    let save = crystal_new_character_save(&config, character("PrivatePreflightStarter"));
    let original = serde_json::to_value(&save).unwrap();
    let fresh = preflight_roots(&save, FreshRoots::all(&save), false).unwrap();
    assert_eq!(fresh.equipment.len(), save.equipment_items_json.len());
    assert_eq!(original, serde_json::to_value(&save).unwrap());
    assert_no_allocator_writes(&probe);
}

#[test]
fn starter_uid_planning_opaque_max_mail_cannot_raise_store_or_candidate_floor() {
    let (config, probe) = mock(None, false);
    let mut old = crystal_new_character_save(&config, character("OpaqueMailStarter"));
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let mut retained = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    retained.unique_id = 925_000;
    old.inventory_items_json = encode_state_vec(&[retained]);
    let text = format!(
        "{{\"unique_id\":{},\"nested\":\"{{\\\"issuedThrough\\\":{}}}\"}}",
        u64::MAX,
        u64::MAX
    );
    let mut systems = crate::Stage5SystemsState::default();
    systems.mail.push(crate::Stage5MailMessage {
        id: 1,
        delivery_nonce: "".into(),
        from: "ordinary-player".into(),
        to: old.character.name.clone(),
        subject: text.clone(),
        body: text.clone(),
        gold: 0,
        items: Vec::new(),
        item_states_json: Vec::new(),
        attachment_origin: crate::Stage5MailAttachmentOrigin::LegacyUnknown,
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    });
    old.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("starter-a").unwrap();
        account.characters.push(old.character.clone());
        account.saves.insert(0, old);
    }
    // New-character planning scans the existing Store; controlled Start also
    // scans the candidate Save containing the same opaque text.
    add_character_to_account(&config, "starter-b", character("AfterOpaqueMail")).unwrap();
    let started = character_save_for_start(&config, "starter-a", 0)
        .unwrap()
        .unwrap();
    let decoded = decode_and_validate_character_save(&started).unwrap();
    assert_eq!(decoded.inventory_items[0].unique_id, 925_000);
    assert_eq!(decoded.stage5_systems.mail[0].body, text);
    assert!(probe.issued.load(Ordering::SeqCst) > 0);
    let high = probe.floor.load(Ordering::SeqCst);
    assert!(high > 925_000 && high < 926_000);
    assert_eq!(
        high,
        historical_uid_floor(UidHistoryRoot::Store(&config.account_store.lock().unwrap())).unwrap()
    );
    assert!(same_character_save(
        &started,
        &character_save_for_start(&config, "starter-a", 0)
            .unwrap()
            .unwrap(),
    )
    .unwrap());
    assert_eq!(probe.floor.load(Ordering::SeqCst), high);
}

#[test]
fn starter_uid_planning_fresh_candidate_counts_original_ids_without_provisional_ids() {
    let (config, _) = mock(None, false);
    let mut save = empty_owned_save(&config, "FreshHistoryOnly");
    let template = crystal_item_by_name("BronzeHelmet").unwrap();
    let child_template = crystal_item_by_name("BronzeBell").unwrap();
    let mut root = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    root.socket_slots = 2;
    for slot in 0..2 {
        root.socketed.push(embedded_item_state_from_template(
            &child_template,
            ItemContainer::Bag1,
            slot,
        ));
    }
    save.inventory_items_json = encode_state_vec(&[root.clone()]);
    let fresh = preflight_roots(&save, FreshRoots::all(&save), false).unwrap();
    assert_eq!(
        candidate_history_floor("starter-a", &save, &fresh).unwrap(),
        0
    );
    root.socketed[1].unique_id = 999_999;
    save.inventory_items_json = encode_state_vec(&[root]);
    let fresh = preflight_roots(&save, FreshRoots::all(&save), false).unwrap();
    assert_eq!(
        candidate_history_floor("starter-a", &save, &fresh).unwrap(),
        999_999
    );
}

#[test]
fn starter_uid_planning_fresh_removed_child_reference_is_retired_before_real_issuance() {
    for equipped in [false, true] {
        let (config, probe) = mock(None, false);
        let mut save = empty_owned_save(&config, "RemovedSocketStarter");
        let template = crystal_item_by_name("BronzeHelmet").unwrap();
        let child_template = crystal_item_by_name("BronzeBell").unwrap();
        let mut root = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
        root.unique_id = 100;
        root.socket_slots = 1;
        let mut child = embedded_item_state_from_template(&child_template, ItemContainer::Bag1, 0);
        child.unique_id = 990_001;
        let mut wire = try_user_item_from_item_state(&root).unwrap();
        wire.slots[0] = Some(try_user_item_from_item_state(&child).unwrap());
        let mut root = try_item_state_from_user_item(root, &wire).unwrap();
        root.socketed.clear();
        assert_eq!(
            root.user_item_metadata
                .as_ref()
                .unwrap()
                .captured_socket_positions
                .as_ref()
                .unwrap()[0]
                .unwrap()
                .unique_id,
            990_001
        );
        validate_committed_item_state_carrier(&root).unwrap();
        if equipped {
            let equipment = super::super::super::equipment::equipment_state_from_item_state(
                &root,
                crate::EquipmentSlot::Helmet,
            );
            save.equipment_items_json = encode_state_vec(&[equipment]);
        } else {
            save.inventory_items_json = encode_state_vec(&[root]);
        }
        let before = serde_json::to_value(&save).unwrap();
        let fresh = preflight_roots(&save, FreshRoots::all(&save), false).unwrap();
        assert_eq!(
            candidate_history_floor("starter-a", &save, &fresh).unwrap(),
            990_001
        );
        assert_eq!(serde_json::to_value(&save).unwrap(), before);
        assert_no_allocator_writes(&probe);
        // The candidate is absent from the Store: its retained map alone must
        // raise the real source floor before fresh rehydration clears that map.
        prepare_new_character(&config, "starter-a", &mut save).unwrap();
        assert_eq!(ids(&save), BTreeSet::from([990_002]));
        assert_eq!(probe.issued.load(Ordering::SeqCst), 1);
        assert_eq!(probe.floor.load(Ordering::SeqCst), 990_002);
    }
}

fn exact_parent_with_legacy_child_and_grandchild() -> ItemState {
    let root_template = crystal_item_by_name("BronzeHelmet").unwrap();
    let child_template = crystal_item_by_name("DemonicBells").unwrap();
    let grandchild_template = crystal_item_by_name("BronzeBell").unwrap();
    let mut root = embedded_item_state_from_template(&root_template, ItemContainer::Bag1, 0);
    root.unique_id = 100;
    root.socket_slots = 3;
    root.attack = 101;
    let mut child = embedded_item_state_from_template(&child_template, ItemContainer::Bag1, 2);
    child.unique_id = 200;
    child.socket_slots = 2;
    let mut wire = try_user_item_from_item_state(&root).unwrap();
    wire.slots[2] = Some(try_user_item_from_item_state(&child).unwrap());
    let mut root = try_item_state_from_user_item(root, &wire).unwrap();
    let mut grandchild =
        embedded_item_state_from_template(&grandchild_template, ItemContainer::Bag1, 1);
    grandchild.unique_id = 300;
    grandchild.heal_hp = 303;
    let child = &mut root.socketed[0];
    child.user_item_metadata = None;
    child.slot = 7; // parent exact identity map, rather than this local slot, owns position 2
    child.defence = 202;
    child.socketed = vec![grandchild];
    validate_committed_item_state_carrier(&root)
        .expect("the mixed historical carrier is structurally legal");
    root
}

#[test]
fn starter_uid_planning_exact_parent_restores_every_legacy_descendant_without_remint() {
    let root = exact_parent_with_legacy_child_and_grandchild();
    let before = serde_json::to_value(&root).unwrap();
    let wire = complete_item_tree(&root).unwrap();
    let child = wire.slots[2].as_ref().unwrap();
    assert_eq!(wire.unique_id, 100);
    assert_eq!(child.unique_id, 200);
    assert_eq!(child.slots[1].as_ref().unwrap().unique_id, 300);
    assert!(wire.slots[0].is_none() && wire.slots[1].is_none());
    assert!(child.slots[0].is_none());
    let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
    inventory.inventory_items = vec![root.clone()];
    validate_inventory_identities(&inventory).unwrap();
    assert_eq!(serde_json::to_value(root).unwrap(), before);
}

#[test]
fn starter_uid_planning_fresh_exact_parent_preserves_legacy_descendants_and_issues_all_nodes() {
    let (config, probe) = mock(None, false);
    let allocator = config.server_item_uid_allocator().unwrap();
    let mut root = exact_parent_with_legacy_child_and_grandchild();
    let before = complete_item_tree(&root).unwrap();
    assign_item(&allocator, &mut root).unwrap();
    let after = try_user_item_from_item_state(&root).unwrap();
    let child = after.slots[2].as_ref().unwrap();
    let grandchild = child.slots[1].as_ref().unwrap();
    assert!(
        after.unique_id > 900_000
            && child.unique_id > after.unique_id
            && grandchild.unique_id > child.unique_id
    );
    assert_eq!(probe.issued.load(Ordering::SeqCst), 3);
    assert_eq!(root.socketed.len(), 1);
    assert_eq!(root.socketed[0].socketed.len(), 1);
    assert_eq!(root.attack, 101);
    assert_eq!(root.socketed[0].defence, 202);
    assert_eq!(root.socketed[0].socketed[0].heal_hp, 303);
    assert_eq!(root.socketed[0].slot, 7);
    assert_eq!(
        root.socketed[0]
            .user_item_metadata
            .as_ref()
            .unwrap()
            .captured_socket_position,
        Some(2)
    );
    assert_eq!(
        root.socketed[0].socketed[0]
            .user_item_metadata
            .as_ref()
            .unwrap()
            .captured_socket_position,
        Some(1)
    );
    let mut normalized = after;
    normalized.unique_id = 100;
    normalized.slots[2].as_mut().unwrap().unique_id = 200;
    normalized.slots[2].as_mut().unwrap().slots[1]
        .as_mut()
        .unwrap()
        .unique_id = 300;
    assert_eq!(normalized, before);
}

#[test]
fn starter_uid_planning_exact_parent_hidden_grandchild_conflict_rejects_before_real_issuer() {
    let (config, probe) = mock(None, false);
    let root = exact_parent_with_legacy_child_and_grandchild();
    let template = crystal_item_by_name("BronzeBell").unwrap();
    let mut other = embedded_item_state_from_template(&template, ItemContainer::Bag1, 1);
    other.unique_id = 300;
    let mut save = empty_owned_save(&config, "HiddenGrandchildConflict");
    save.inventory_items_json = encode_state_vec(&[root, other]);
    assert!(prepare_character_start(&config, "starter-a", &mut save)
        .unwrap_err()
        .contains("duplicate item UID"));
    assert_no_allocator_writes(&probe);
}

#[test]
fn starter_uid_planning_legacy_fishing_rod_equipment_migration_persists_canonical_slot_width() {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "mir2-starter-rod-uids-{}-{}",
        std::process::id(),
        unix_now_ms(),
    ));
    std::fs::create_dir(&root).unwrap();
    let account_path = root.join("accounts.json");
    let allocator =
        UserItemUidAllocator::initialize_file(root.join("uids.json"), 2_000_000).unwrap();
    let config = SimulationConfig::default()
        .with_account_store_path(&account_path)
        .with_item_uid_allocator(allocator.clone())
        .unwrap();
    let template = crystal_item_by_name("BlueFishingRod").unwrap();
    let mut base = embedded_item_state_from_template(&template, ItemContainer::Bag1, 0);
    base.socket_slots = 0;
    base.user_item_metadata = None;
    assert_eq!(try_user_item_from_item_state(&base).unwrap().slots.len(), 5);
    let mut rod = super::super::super::equipment::equipment_state_from_item_state(
        &base,
        crate::EquipmentSlot::Weapon,
    );
    rod.user_item_unique_id = None;
    rod.user_item_metadata = None;
    rod.socket_slots = 0;
    let mut save = empty_owned_save(&config, "LegacyRodStarter");
    save.character.index = 9007;
    save.equipment_items_json = encode_state_vec(&[rod]);
    save.equipment_items_explicit_empty = false;
    validate_character_save_record(&save).unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        let mut account = AccountRecord::new(save.character.clone());
        account.saves.insert(9007, save);
        store.accounts.insert("starter-rod-owner".into(), account);
    }
    config.save_account_store().unwrap();
    let migrated = character_save_for_start(&config, "starter-rod-owner", 9007)
        .unwrap()
        .unwrap();
    let equipment = decode_saved_equipment_states("rod", &migrated.equipment_items_json).unwrap();
    assert_eq!(equipment[0].socket_slots, 5);
    assert_eq!(
        equipment[0]
            .user_item_metadata
            .as_ref()
            .unwrap()
            .captured_socket_positions
            .as_ref()
            .unwrap()
            .len(),
        5
    );
    assert!(equipment[0]
        .user_item_unique_id
        .is_some_and(|id| id > 2_000_000));
    validate_save_identities(&migrated).unwrap();
    let persisted: crate::AccountStore =
        serde_json::from_slice(&std::fs::read(&account_path).unwrap()).unwrap();
    assert!(same_character_save(
        &migrated,
        &persisted.accounts["starter-rod-owner"].saves[&9007]
    )
    .unwrap());
    let high = allocator.issued_through().unwrap();
    let repeated = character_save_for_start(&config, "starter-rod-owner", 9007)
        .unwrap()
        .unwrap();
    assert!(same_character_save(&migrated, &repeated).unwrap());
    assert_eq!(allocator.issued_through().unwrap(), high);
}

#[test]
fn starter_uid_planning_mount_bells_keep_original_protocol_position_and_exact_attributes() {
    for captured_hint in [false, true] {
        let (config, probe) = mock(None, false);
        let allocator = config.server_item_uid_allocator().unwrap();
        let mount_template = crystal_item_by_name("RedTiger").unwrap();
        let bells_template = crystal_item_by_name("BronzeBell").unwrap();
        let mut mount = embedded_item_state_from_template(&mount_template, ItemContainer::Bag1, 0);
        mount.unique_id = 400;
        mount.socket_slots = mount.socket_slots.max(2);
        let mut bells = embedded_item_state_from_template(&bells_template, ItemContainer::Bag1, 0);
        bells.unique_id = 401;
        bells.description = "retained Bells attributes".into();
        bells.attack = 123;
        if captured_hint {
            let wire = try_user_item_from_item_state(&bells).unwrap();
            bells = try_item_state_from_user_item(bells, &wire).unwrap();
            bells
                .user_item_metadata
                .as_mut()
                .unwrap()
                .captured_socket_position = Some(0);
        }
        mount.socketed = vec![bells];
        validate_committed_item_state_carrier(&mount).unwrap();
        let original = try_user_item_from_item_state(&mount).unwrap();
        assert_eq!(original.slots[1].as_ref().unwrap().unique_id, 401);
        assert!(original.slots[0].is_none());
        assert_eq!(complete_item_tree(&mount).unwrap(), original);
        assign_item(&allocator, &mut mount).unwrap();
        let after = try_user_item_from_item_state(&mount).unwrap();
        assert!(after.slots[0].is_none());
        assert!(after.unique_id > 900_000);
        assert!(after.slots[1].as_ref().unwrap().unique_id > after.unique_id);
        assert_eq!(mount.socketed[0].slot, 0);
        assert_eq!(
            mount.socketed[0]
                .user_item_metadata
                .as_ref()
                .unwrap()
                .captured_socket_position,
            Some(1)
        );
        assert_eq!(mount.socketed[0].description, "retained Bells attributes");
        assert_eq!(mount.socketed[0].attack, 123);
        assert_eq!(probe.issued.load(Ordering::SeqCst), 2);
        let mut normalized = after.clone();
        normalized.unique_id = 400;
        normalized.slots[1].as_mut().unwrap().unique_id = 401;
        assert_eq!(normalized, original);
        let encoded = serde_json::to_string(&mount).unwrap();
        let restored: ItemState = serde_json::from_str(&encoded).unwrap();
        validate_committed_item_state_carrier(&restored).unwrap();
        assert_eq!(complete_item_tree(&restored).unwrap(), after);
        let mut inventory = InventoryResource::new(BASE_STORAGE_SLOTS);
        inventory.inventory_items = vec![restored];
        validate_inventory_identities(&inventory).unwrap();
    }
}

//! Real File-source Session claims with an observable server UID store.
use super::*;
use crate::{
    SimulationConfig, UserItemUid, UserItemUidAllocator, UserItemUidError, UserItemUidStore,
};
use mir2_protocol::ClientPacket;
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
type Callback = Box<dyn FnOnce(u64) + Send>;
#[derive(Default)]
struct Store {
    high: AtomicU64,
    initializing: AtomicBool,
    attempts: AtomicU64,
    ensures: AtomicU64,
    fail_at: AtomicU64,
    unknown: AtomicBool,
    issued: Mutex<Vec<u64>>,
    account: Mutex<Option<Arc<Mutex<AccountStore>>>>,
    callback: Mutex<Option<Callback>>,
}
impl Store {
    fn unlocked(&self) {
        if let Some(account) = &*self.account.lock().unwrap() {
            let _guard = account
                .try_lock()
                .expect("mail UID issuer must not re-enter account lock");
        }
    }
    fn ids(&self) -> Vec<u64> {
        self.issued.lock().unwrap().clone()
    }
}
impl UserItemUidStore for Store {
    fn issue_one(&self, reason: UserItemUidReason) -> Result<UserItemUid, UserItemUidError> {
        self.unlocked();
        if self.initializing.load(Ordering::SeqCst) {
            assert_eq!(reason, UserItemUidReason::CharacterStartItem);
        } else {
            assert_eq!(reason, UserItemUidReason::SystemMailGrant);
        }
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_at.load(Ordering::SeqCst) == attempt {
            return Err(UserItemUidError::InvalidUid { value: 0 });
        }
        let uid = crate::player_drop::player_drop_next_ordinary_item_uid(
            self.high.load(Ordering::SeqCst),
        )
        .unwrap();
        self.high.store(uid, Ordering::SeqCst);
        self.issued.lock().unwrap().push(uid);
        let callback = self.callback.lock().unwrap().take();
        if let Some(callback) = callback {
            callback(uid);
        }
        if self.unknown.swap(false, Ordering::SeqCst) {
            return Err(UserItemUidError::CommitOutcomeUnknown {
                path: PathBuf::from("owned-mail-uid-fixture"),
                reason: "injected committed response loss".into(),
            });
        }
        UserItemUid::new(uid)
    }
    fn issued_through(&self) -> Result<u64, UserItemUidError> {
        self.unlocked();
        Ok(self.high.load(Ordering::SeqCst))
    }
    fn ensure_issued_through_at_least(&self, floor: u64) -> Result<u64, UserItemUidError> {
        self.unlocked();
        self.ensures.fetch_add(1, Ordering::SeqCst);
        Ok(self.high.fetch_max(floor, Ordering::SeqCst).max(floor))
    }
}
struct Fixture {
    config: SimulationConfig,
    session: SimulationSession,
    store: Arc<Store>,
    path: PathBuf,
}
impl Fixture {
    fn new(label: &str, mail: Vec<Stage5MailMessage>) -> Self {
        Self::with_policy(label, mail, true)
    }
    fn with_policy(label: &str, mail: Vec<Stage5MailMessage>, durable: bool) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let directory = root.join(format!(
            "mir2-mail-uid-{label}-{}-{}-{}",
            std::process::id(),
            crate::monthly_card::monthly_card_now_ms(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(directory.starts_with(root));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("accounts.json");
        let store = Arc::new(Store::default());
        store.high.store(1_000_000, Ordering::SeqCst);
        let config = SimulationConfig::default().with_account_store_path(&path);
        let config = if durable {
            config
                .with_item_uid_allocator(UserItemUidAllocator::from_store(store.clone()))
                .unwrap()
        } else {
            config
        };
        *store.account.lock().unwrap() = Some(config.account_store.clone());
        let mut session = SimulationSession::new(config.clone());
        store.initializing.store(true, Ordering::SeqCst);
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(session.active_identity().is_some());
        store.initializing.store(false, Ordering::SeqCst);
        // Startup consumed real identities. Retain their high watermark while
        // exposing only the claim under test to fault/issuance observations.
        store.issued.lock().unwrap().clear();
        store.attempts.store(0, Ordering::SeqCst);
        store.ensures.store(0, Ordering::SeqCst);
        store.fail_at.store(0, Ordering::SeqCst);
        store.unknown.store(false, Ordering::SeqCst);
        store.callback.lock().unwrap().take();
        {
            let mut inventory = session.app.world_mut().resource_mut::<InventoryResource>();
            inventory.inventory_items.clear();
            inventory.belt_items.clear();
            inventory.storage_items.clear();
            inventory.equipment_items.clear();
            inventory.reserved_item_unique_ids.clear();
            inventory.inventory_capacity = 46;
        }
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 100;
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .credit = 333;
        session
            .app
            .world_mut()
            .resource_mut::<Stage5SystemsResource>()
            .stage5_systems
            .mail = mail;
        session.save_active_character().unwrap();
        Self {
            config,
            session,
            store,
            path,
        }
    }
    fn state(&self) -> serde_json::Value {
        serde_json::json!({"inventory":encode_state_vec(&self.session.app.world().resource::<InventoryResource>().inventory_items),
        "belt":encode_state_vec(&self.session.app.world().resource::<InventoryResource>().belt_items),"gold":self.session.world_snapshot().gold,
        "credit":self.session.app.world().resource::<PlayerRuntimeResource>().credit,"mail":self.session.world_snapshot().stage5_systems.mail})
    }
}
fn mail(id: u32, items: Vec<ItemState>, gold: u32) -> Stage5MailMessage {
    Stage5MailMessage {
        id,
        delivery_nonce: crate::config::new_stage5_mail_delivery_nonce(),
        from: "Fixture".into(),
        to: "Scout".into(),
        subject: format!("Mail {id}"),
        body: "owned fixture".into(),
        gold,
        items: vec!["red-potion".into()],
        item_states_json: items
            .iter()
            .map(|item| serde_json::to_string(item).unwrap())
            .collect(),
        attachment_origin: Stage5MailAttachmentOrigin::FreshGrant,
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    }
}
fn nested() -> ItemState {
    let template = mir2_game_data::crystal_item_by_name("(HP)DrugSmall").unwrap();
    let mut item = super::super::super::items::embedded_item_state_from_template(
        &template,
        ItemContainer::Bag1,
        0,
    );
    item.socket_slots = 2;
    let mut raw = try_user_item_from_item_state(&item).unwrap();
    raw.unique_id = 765_001;
    raw.count = 2;
    raw.current_dura = 123;
    raw.max_dura = 456;
    raw.cursed = true;
    raw.added_stats = vec![mir2_protocol::UserItemStat { stat: 17, value: 6 }];
    raw.awake_type = 2;
    raw.awake_values = vec![4, 5];
    raw.gem_count = 3;
    raw.refined_value = 6;
    raw.refine_added = 7;
    raw.refine_success_chance = 88;
    raw.wedding_ring = 23;
    raw.gm_made = true;
    let child = super::super::super::items::embedded_item_state_from_template(
        &mir2_game_data::crystal_item_by_name("BronzeBell").unwrap(),
        ItemContainer::Bag1,
        0,
    );
    let mut child = try_user_item_from_item_state(&child).unwrap();
    child.unique_id = 765_002;
    raw.slots = vec![None, Some(child)];
    let item = try_item_state_from_user_item(item, &raw).unwrap();
    validate_exact_stage5_item_state(&item).unwrap();
    item
}

fn sidecarless_socket_tree() -> ItemState {
    let mut root = super::super::super::items::embedded_item_state_from_template(
        &mir2_game_data::crystal_item_by_name("BronzeHelmet").unwrap(),
        ItemContainer::Bag1,
        17,
    );
    root.unique_id = 765_101;
    root.socket_slots = 4;
    root.description = "retained mail helmet".into();
    root.attack = 42;
    root.defence = 43;
    root.heal_hp = 44;
    root.heal_mp = 45;
    root.weight = 46;
    root.durability_current = Some(123);
    root.durability_max = Some(456);
    root.cursed = true;
    let template = mir2_game_data::crystal_item_by_name("BronzeBell").unwrap();
    for (position, uid, power) in [(0, 765_102, 51), (2, 765_103, 61)] {
        let mut child = super::super::super::items::embedded_item_state_from_template(
            &template,
            ItemContainer::Bag1,
            position,
        );
        child.unique_id = uid;
        child.description = format!("retained mail child {position}");
        child.attack = power;
        child.defence = power + 1;
        child.heal_hp = power + 2;
        child.heal_mp = power + 3;
        child.weight = power as u16 + 4;
        child.added_stats = vec![mir2_protocol::UserItemStat { stat: 17, value: 6 }];
        root.socketed.push(child);
    }
    validate_exact_stage5_item_state(&root).unwrap();
    // This is the precise accepted legacy carrier the protocol compatibility
    // converter omits; mail planning must preserve its complete live tree.
    assert!(try_user_item_from_item_state(&root)
        .unwrap()
        .slots
        .iter()
        .all(Option::is_none));
    root
}

fn live_functional_fields(item: &ItemState) -> serde_json::Value {
    fn normalize(value: &mut serde_json::Value) {
        let object = value.as_object_mut().unwrap();
        object.remove("unique_id");
        object.remove("user_item_metadata");
        for child in object.get_mut("socketed").unwrap().as_array_mut().unwrap() {
            normalize(child);
        }
    }
    let mut value = serde_json::to_value(item).unwrap();
    normalize(&mut value);
    value
}

fn require_socket_tree_preserved(original: &ItemState, claimed: &ItemState) {
    let mut original = original.clone();
    original.slot = claimed.slot;
    original.container = claimed.container;
    assert_eq!(
        live_functional_fields(&original),
        live_functional_fields(claimed)
    );
    let raw = try_user_item_from_item_state(claimed).unwrap();
    assert_eq!(raw.slots.len(), 4);
    assert!(raw.slots[1].is_none());
    assert!(raw.slots[3].is_none());
    let mut identities = BTreeSet::from([raw.unique_id]);
    for position in [0, 2] {
        let child = raw.slots[position].as_ref().unwrap();
        let live = claimed
            .socketed
            .iter()
            .find(|item| item.slot as usize == position)
            .unwrap();
        assert!(identities.insert(child.unique_id));
        assert_eq!(child.unique_id, live.unique_id);
        assert_eq!(
            live.user_item_metadata
                .as_ref()
                .unwrap()
                .captured_socket_position,
            Some(position as u8)
        );
        let captured = claimed
            .user_item_metadata
            .as_ref()
            .unwrap()
            .captured_socket_positions
            .as_ref()
            .unwrap()[position]
            .unwrap();
        assert_eq!(captured.unique_id, child.unique_id);
        assert_eq!(captured.item_index, child.item_index);
    }
    assert_eq!(identities.len(), 3);
    assert!(identities.iter().all(|uid| *uid > 1_000_000));
}

fn custody_mail(item: ItemState) -> Stage5MailMessage {
    let mut mail = mail(1, vec![item], 5);
    mail.attachment_origin = Stage5MailAttachmentOrigin::ExistingCustody;
    mail
}

#[test]
fn mail_uid_legacy_mount_bells_resolved_protocol_position_outranks_local_and_captured_zero() {
    for captured_hint in [false, true] {
        for origin in [
            Stage5MailAttachmentOrigin::FreshGrant,
            Stage5MailAttachmentOrigin::ExistingCustody,
        ] {
            let mut root = super::super::super::items::embedded_item_state_from_template(
                &mir2_game_data::crystal_item_by_name("RedTiger").unwrap(),
                ItemContainer::Bag1,
                17,
            );
            root.unique_id = 765_201;
            root.socket_slots = 2;
            root.description = "retained mail mount".into();
            root.attack = 42;
            let mut child = super::super::super::items::embedded_item_state_from_template(
                &mir2_game_data::crystal_item_by_name("BronzeBell").unwrap(),
                ItemContainer::Bag1,
                0,
            );
            child.unique_id = 765_202;
            if captured_hint {
                let raw = try_user_item_from_item_state(&child).unwrap();
                child = try_item_state_from_user_item(child, &raw).unwrap();
                child
                    .user_item_metadata
                    .as_mut()
                    .unwrap()
                    .captured_socket_position = Some(0);
            }
            child.attack = 51;
            child.defence = 52;
            child.description = "retained mail bells".into();
            root.socketed.push(child);
            validate_exact_stage5_item_state(&root).unwrap();
            let before_wire = try_user_item_from_item_state(&root).unwrap();
            assert!(before_wire.slots[0].is_none());
            assert_eq!(before_wire.slots[1].as_ref().unwrap().unique_id, 765_202);
            let mut message = mail(1, vec![root.clone()], 5);
            message.attachment_origin = origin;
            let mut fixture = Fixture::new("mount-bells-position", vec![message]);
            let issued = fixture.store.ids().len();
            let plan = commit_claims(fixture.session.app.world_mut(), &[1]).unwrap();
            let claimed = &plan.claim.items[0];
            let after_wire = try_user_item_from_item_state(claimed).unwrap();
            let mut expected = root;
            expected.slot = claimed.slot;
            expected.container = claimed.container;
            assert_eq!(
                live_functional_fields(&expected),
                live_functional_fields(claimed)
            );
            assert_eq!(claimed.socketed.len(), 1);
            assert_eq!(claimed.socketed[0].slot, 0);
            assert_eq!(
                claimed.socketed[0]
                    .user_item_metadata
                    .as_ref()
                    .unwrap()
                    .captured_socket_position,
                Some(1)
            );
            assert!(after_wire.slots[0].is_none());
            let mut normalized = after_wire.clone();
            normalized.unique_id = before_wire.unique_id;
            normalized.slots[1].as_mut().unwrap().unique_id = 765_202;
            assert_eq!(normalized, before_wire);
            if origin == Stage5MailAttachmentOrigin::ExistingCustody {
                assert_eq!(after_wire, before_wire);
                assert_eq!(fixture.store.ids().len(), issued);
            } else {
                assert_ne!(after_wire.unique_id, before_wire.unique_id);
                assert_ne!(after_wire.slots[1].as_ref().unwrap().unique_id, 765_202);
                assert_eq!(fixture.store.ids().len(), issued + 2);
            }
            fixture.session.save_active_character().unwrap();
            let durable: AccountStore =
                serde_json::from_slice(&fs::read(&fixture.path).unwrap()).unwrap();
            let saved: ItemState =
                serde_json::from_str(&durable.accounts["demo"].saves[&0].inventory_items_json[0])
                    .unwrap();
            assert_eq!(try_user_item_from_item_state(&saved).unwrap(), after_wire);
            assert_eq!(
                live_functional_fields(&saved),
                live_functional_fields(claimed)
            );
        }
    }
}

#[test]
fn mail_uid_existing_custody_transfers_complete_identity_without_issuer_or_receipt_conflict() {
    let original = sidecarless_socket_tree();
    let mut fixture = Fixture::new("custody", vec![custody_mail(original.clone())]);
    // An immutable old projection receipt may mention this UID; it is not a
    // second physical owner and must not be scanned as one.
    {
        let mut store = fixture.config.account_store.lock().unwrap();
        store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap()
            .player_kill_receipts
            .insert(
                "historical".into(),
                format!("{{\"unique_id\":{}}}", original.unique_id),
            );
    }
    // Keep the receipt on a separate canonical actor so this source's exact
    // save CAS remains unchanged while the physical-ownership scan sees it.
    {
        let mut store = fixture.config.account_store.lock().unwrap();
        let source = store.accounts["demo"].saves[&0].clone();
        let mut historical = source.clone();
        historical.character.index = 9;
        historical.character.name = "Historical".into();
        historical.stage5_systems_json = None;
        let account = store.accounts.get_mut("demo").unwrap();
        account
            .saves
            .get_mut(&0)
            .unwrap()
            .player_kill_receipts
            .clear();
        account.characters.push(historical.character.clone());
        account.saves.insert(9, historical);
    }
    let issued = fixture.store.ids();
    let ensures = fixture.store.ensures.load(Ordering::SeqCst);
    let plan = commit_claims(fixture.session.app.world_mut(), &[1]).unwrap();
    assert_eq!(fixture.store.ids(), issued);
    assert_eq!(fixture.store.ensures.load(Ordering::SeqCst), ensures);
    let claimed = &plan.claim.items[0];
    let mut expected = original.clone();
    expected.container = claimed.container;
    expected.slot = claimed.slot;
    assert_eq!(
        live_functional_fields(&expected),
        live_functional_fields(claimed)
    );
    assert_eq!(
        complete_item_tree(&expected).unwrap(),
        complete_item_tree(claimed).unwrap()
    );
    assert_eq!(
        plan.custody_ids,
        BTreeSet::from([765_101, 765_102, 765_103])
    );
    assert!(plan.issued.is_empty());
    assert_eq!(plan.inventory.reserved_item_unique_ids, BTreeSet::new());
    assert!(plan.systems.mail[0].claimed && plan.systems.mail[0].item_states_json.is_empty());
    fixture.session.save_active_character().unwrap();
    let durable: AccountStore = serde_json::from_slice(&fs::read(&fixture.path).unwrap()).unwrap();
    let save = &durable.accounts["demo"].saves[&0];
    let stored: ItemState = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
    assert_eq!(
        complete_item_tree(&stored).unwrap(),
        complete_item_tree(claimed).unwrap()
    );
    assert!(systems(save).unwrap().mail[0].item_states_json.is_empty());
}

#[test]
fn mail_uid_existing_custody_rejects_live_reserved_and_other_source_child_owners() {
    for conflict in ["live", "reserved", "other-mail", "other-account"] {
        let original = sidecarless_socket_tree();
        let mut fixture = Fixture::new(conflict, vec![custody_mail(original.clone())]);
        let child = original.socketed[0].clone();
        match conflict {
            "live" => fixture
                .session
                .app
                .world_mut()
                .resource_mut::<InventoryResource>()
                .inventory_items
                .push(child),
            "reserved" => {
                fixture
                    .session
                    .app
                    .world_mut()
                    .resource_mut::<InventoryResource>()
                    .reserved_item_unique_ids
                    .insert(child.unique_id);
            }
            "other-mail" => {
                let mut store = fixture.config.account_store.lock().unwrap();
                let save = store
                    .accounts
                    .get_mut("demo")
                    .unwrap()
                    .saves
                    .get_mut(&0)
                    .unwrap();
                let mut state = systems(save).unwrap();
                state.mail.push(mail(2, vec![child], 0));
                save.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
            }
            "other-account" => {
                let mut store = fixture.config.account_store.lock().unwrap();
                let mut other = store.accounts["demo"].clone();
                let save = other.saves.get_mut(&0).unwrap();
                save.stage5_systems_json = None;
                save.inventory_items_json = encode_state_vec(&[child]);
                store.accounts.insert("other".into(), other);
            }
            _ => unreachable!(),
        }
        let before = fixture.state();
        let issued = fixture.store.ids();
        let ensures = fixture.store.ensures.load(Ordering::SeqCst);
        assert!(
            commit_claims(fixture.session.app.world_mut(), &[1]).is_err(),
            "{conflict}"
        );
        assert_eq!(fixture.state(), before, "{conflict}");
        assert_eq!(fixture.store.ids(), issued, "{conflict}");
        assert_eq!(
            fixture.store.ensures.load(Ordering::SeqCst),
            ensures,
            "{conflict}"
        );
        let save = fixture.config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
        assert!(!systems(&save).unwrap().mail[0].claimed);
    }
}

#[test]
fn mail_uid_unknown_origin_and_live_forged_origin_never_authorize_durable_attachments() {
    for (label, items, forged) in [
        ("unknown-exact", vec![sidecarless_socket_tree()], false),
        ("unknown-keyed", Vec::new(), false),
        ("forged-origin", vec![sidecarless_socket_tree()], true),
    ] {
        let mut message = mail(1, items, 5);
        message.attachment_origin = Stage5MailAttachmentOrigin::LegacyUnknown;
        message.from = "Gameshop".into();
        message.subject = "Trusted reward".into();
        let mut fixture = Fixture::new(label, vec![message]);
        if forged {
            fixture
                .session
                .app
                .world_mut()
                .resource_mut::<Stage5SystemsResource>()
                .stage5_systems
                .mail[0]
                .attachment_origin = Stage5MailAttachmentOrigin::FreshGrant;
        }
        let before = fixture.state();
        let bytes = fs::read(&fixture.path).unwrap();
        let issued = fixture.store.ids();
        let ensures = fixture.store.ensures.load(Ordering::SeqCst);
        assert!(commit_claims(fixture.session.app.world_mut(), &[1]).is_err());
        assert_eq!(fixture.state(), before);
        assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
        assert_eq!(fixture.store.ids(), issued);
        assert_eq!(fixture.store.ensures.load(Ordering::SeqCst), ensures);
    }
}

#[test]
fn mail_uid_existing_custody_rejects_zero_duplicate_and_key_only_instances_without_mint() {
    for kind in ["zero", "duplicate", "key-only"] {
        let mut original = sidecarless_socket_tree();
        if kind == "zero" {
            original.socketed[0].unique_id = 0;
        }
        if kind == "duplicate" {
            original.socketed[1].unique_id = original.socketed[0].unique_id;
        }
        let mut message = if kind == "key-only" {
            mail(1, Vec::new(), 5)
        } else {
            custody_mail(original)
        };
        message.attachment_origin = Stage5MailAttachmentOrigin::ExistingCustody;
        let mut fixture = Fixture::new(kind, vec![message]);
        let before = fixture.state();
        let issued = fixture.store.ids();
        let ensures = fixture.store.ensures.load(Ordering::SeqCst);
        assert!(
            commit_claims(fixture.session.app.world_mut(), &[1]).is_err(),
            "{kind}"
        );
        assert_eq!(fixture.state(), before);
        assert_eq!(fixture.store.ids(), issued);
        assert_eq!(fixture.store.ensures.load(Ordering::SeqCst), ensures);
    }
}

#[test]
fn mail_uid_keyed_fresh_grant_never_merges_into_bound_or_cursed_stack() {
    for bound in [true, false] {
        let mut fixture = Fixture::new("keyed-identity", vec![mail(1, Vec::new(), 5)]);
        let mut existing = item_grants::FreshItemGrant::plain(
            ItemContainer::Bag1,
            "red-potion",
            &stage5_item_name("red-potion"),
            "Stage 5 mail attachment.",
            20,
            1,
            1,
            UserItemUidReason::SystemMailGrant,
        )
        .unwrap()
        .prototype;
        existing.unique_id = 800_001;
        existing.slot = 10;
        if bound {
            existing.soul_bound_id = Some(12);
        } else {
            existing.cursed = true;
        }
        fixture
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(existing.clone());
        fixture.session.save_active_character().unwrap();
        let issued = fixture.store.ids().len();
        let plan = commit_claims(fixture.session.app.world_mut(), &[1]).unwrap();
        assert_eq!(
            plan.inventory.inventory_items.len() + plan.inventory.belt_items.len(),
            2
        );
        assert_eq!(
            plan.inventory
                .inventory_items
                .iter()
                .find(|item| item.unique_id == existing.unique_id)
                .unwrap()
                .quantity,
            1
        );
        assert_ne!(plan.claim.items[0].unique_id, existing.unique_id);
        assert_eq!(fixture.store.ids().len(), issued + 1);
    }
}

#[test]
fn mail_uid_maximum_private_planning_floor_rejects_before_issuer_without_escaping() {
    let mut original = sidecarless_socket_tree();
    original.unique_id = u64::MAX;
    let mut fixture = Fixture::new("max-planning-floor", vec![mail(1, vec![original], 5)]);
    let before = fixture.state();
    let issued = fixture.store.ids();
    let ensures = fixture.store.ensures.load(Ordering::SeqCst);
    assert!(commit_claims(fixture.session.app.world_mut(), &[1]).is_err());
    assert_eq!(fixture.state(), before);
    assert_eq!(fixture.store.ids(), issued);
    assert_eq!(fixture.store.ensures.load(Ordering::SeqCst), ensures);
    assert!(fixture
        .session
        .app
        .world()
        .resource::<InventoryResource>()
        .reserved_item_unique_ids
        .is_empty());
}

#[test]
fn mail_uid_initially_stale_claim_uses_canonical_assets_and_retains_stale_revision_gate() {
    for fail_publication in [false, true] {
        let mut fixture = Fixture::new("stale-source", vec![mail(1, vec![nested()], 5)]);
        let mut removed = nested();
        removed.unique_id = 800_101;
        removed.socketed.clear();
        removed.socket_slots = 0;
        removed.user_item_metadata = None;
        removed.slot = 12;
        fixture
            .session
            .app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items
            .push(removed.clone());
        fixture.session.save_active_character().unwrap();
        let session = fixture
            .session
            .app
            .world()
            .resource::<SessionResource>()
            .clone();
        let stale_revision = session.active_save_revision().unwrap();
        fixture
            .config
            .commit_account_store_transaction(&["demo".into()], |store| {
                let save = store
                    .accounts
                    .get_mut("demo")
                    .unwrap()
                    .saves
                    .get_mut(&0)
                    .unwrap();
                save.inventory_items_json.clear();
                save.inventory_capacity = 54;
                save.gold = 30;
                save.credit = 222;
                save.pk_points = 123;
                save.revision += 1;
                Ok(())
            })
            .unwrap();
        let live_before = fixture.state();
        if fail_publication {
            let session = session.clone();
            *fixture.store.callback.lock().unwrap() = Some(Box::new(move |_| {
                session.bind_active_save_revision(stale_revision + 10);
            }));
        }
        let result = commit_claims(fixture.session.app.world_mut(), &[1]);
        if fail_publication {
            assert!(result
                .unwrap_err()
                .starts_with("SOURCE_COMMITTED_LIVE_PUBLICATION_FAILED"));
            assert!(fixture.config.ensure_account_store_writable().is_err());
            assert_eq!(fixture.state(), live_before);
        } else {
            let plan = result.unwrap();
            assert_eq!(plan.gold, 35);
            assert_eq!(plan.credit, 222);
            assert_eq!(plan.inventory.inventory_capacity, 54);
            assert!(!plan
                .inventory
                .inventory_items
                .iter()
                .any(|item| item.unique_id == removed.unique_id));
            assert_eq!(session.active_save_revision(), Some(stale_revision));
            // Other private business state still predates the canonical save.
            // An ordinary save must never publish it under the newer revision.
            assert_ne!(
                fixture
                    .session
                    .app
                    .world()
                    .resource::<PlayerRuntimeResource>()
                    .pk_points,
                123
            );
            let _ = fixture.session.save_active_character();
        }
        let save = fixture.config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
        assert_eq!((save.gold, save.credit, save.pk_points), (35, 222, 123));
        assert!(!save.inventory_items_json.iter().any(
            |encoded| serde_json::from_str::<ItemState>(encoded)
                .unwrap()
                .unique_id
                == removed.unique_id
        ));
        assert!(systems(&save).unwrap().mail[0].claimed);
        let issued = fixture.store.ids();
        assert!(commit_claims(fixture.session.app.world_mut(), &[1]).is_err());
        assert_eq!(fixture.store.ids(), issued);
    }
}

#[test]
fn mail_uid_synthetic_keyed_attachment_is_legacy_only_and_never_enters_durable_issuer() {
    for durable in [false, true] {
        let mut message = mail(1, Vec::new(), 5);
        message.items = vec!["mail-credit-item".into()];
        let mut fixture = Fixture::with_policy("synthetic-key", vec![message], durable);
        let before = fixture.state();
        let issued = fixture.store.ids();
        let ensures = fixture.store.ensures.load(Ordering::SeqCst);
        let outcome =
            super::super::stage5_claim_mail_authoritative(fixture.session.app.world_mut(), 1);
        if durable {
            assert!(outcome.is_err());
            assert_eq!(fixture.state(), before);
        } else {
            let outcome = outcome.unwrap();
            assert_eq!(outcome.items[0].key, "mail-credit-item");
            assert!(fixture.session.world_snapshot().stage5_systems.mail[0].claimed);
        }
        assert_eq!(fixture.store.ids(), issued);
        assert_eq!(fixture.store.ensures.load(Ordering::SeqCst), ensures);
    }
}

#[test]
fn mail_uid_sidecarless_same_template_sockets_keep_positions_and_all_live_fields() {
    let original = sidecarless_socket_tree();
    let mut fixture = Fixture::new("sidecarless", vec![mail(1, vec![original.clone()], 5)]);
    let plan = commit_claims(fixture.session.app.world_mut(), &[1]).unwrap();
    assert_eq!(fixture.store.ids().len(), 3);
    require_socket_tree_preserved(&original, &plan.claim.items[0]);
    assert_eq!(
        serde_json::to_value(&plan.claim.items[0]).unwrap(),
        serde_json::to_value(&plan.inventory.inventory_items[0]).unwrap()
    );
    fixture.session.save_active_character().unwrap();
    let durable: AccountStore = serde_json::from_slice(&fs::read(&fixture.path).unwrap()).unwrap();
    let saved: ItemState =
        serde_json::from_str(&durable.accounts["demo"].saves[&0].inventory_items_json[0]).unwrap();
    require_socket_tree_preserved(&original, &saved);
}

#[test]
fn mail_uid_sidecarless_child_reservation_failure_preserves_unclaimed_complete_tree() {
    let original = sidecarless_socket_tree();
    let mut fixture = Fixture::new(
        "sidecarless-failure",
        vec![mail(1, vec![original.clone()], 5)],
    );
    let before = fixture.state();
    let bytes = fs::read(&fixture.path).unwrap();
    fixture.store.fail_at.store(
        fixture.store.attempts.load(Ordering::SeqCst) + 3,
        Ordering::SeqCst,
    );
    assert!(commit_claims(fixture.session.app.world_mut(), &[1]).is_err());
    assert_eq!(fixture.state(), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    let burned = fixture.store.ids();
    assert_eq!(burned.len(), 2);
    fixture.store.fail_at.store(0, Ordering::SeqCst);
    let plan = commit_claims(fixture.session.app.world_mut(), &[1]).unwrap();
    require_socket_tree_preserved(&original, &plan.claim.items[0]);
    assert!(plan.issued.iter().all(|uid| *uid > burned[1]));
    assert_eq!(fixture.store.ids().len(), 5);
}

#[test]
fn mail_uid_sidecarless_ambiguous_socket_positions_reject_before_real_issuer() {
    let mut fixture = Fixture::new("sidecarless-position", Vec::new());
    let mut invalid = sidecarless_socket_tree();
    invalid.socketed[1].slot = invalid.socketed[0].slot;
    // Existing exact validation accepts this legacy shape. The complete fresh
    // planner must reject ambiguous positions rather than delete a child.
    validate_exact_stage5_item_state(&invalid).unwrap();
    fixture
        .session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .mail = vec![mail(1, vec![invalid], 5)];
    let before = fixture.state();
    let issued = fixture.store.ids();
    assert!(matches!(
        prepare_claims(fixture.session.app.world(), &[1]),
        Err(Stage5MailClaimError::InvalidExactItemState)
    ));
    assert_eq!(fixture.state(), before);
    assert_eq!(fixture.store.ids(), issued);
}

#[test]
fn mail_uid_whole_batch_rejects_bad_later_exact_json_before_real_issuance() {
    let mut invalid = mail(2, Vec::new(), 7);
    invalid.item_states_json = vec!["{bad exact carrier".into()];
    // Install malformed live input after saving a valid source, so the planner
    // itself must reject it; it cannot repair it using the canonical mailbox.
    let mut fixture = Fixture::new("invalid-later", vec![mail(1, vec![nested()], 5)]);
    fixture
        .session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .mail
        .push(invalid);
    let before = fixture.state();
    let ids = fixture.store.ids();
    let bytes = fs::read(&fixture.path).unwrap();
    assert!(matches!(
        prepare_claims(fixture.session.app.world(), &[1, 2]),
        Err(Stage5MailClaimError::InvalidExactItemState)
    ));
    assert_eq!(fixture.state(), before);
    assert_eq!(fixture.store.ids(), ids);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn mail_uid_partial_reservation_failure_discards_batch_then_retry_uses_new_complete_tree() {
    let original = nested();
    let mut fixture = Fixture::new(
        "partial",
        vec![mail(1, vec![original.clone()], 5), mail(2, Vec::new(), 7)],
    );
    let before = fixture.state();
    let bytes = fs::read(&fixture.path).unwrap();
    fixture.store.fail_at.store(
        fixture.store.attempts.load(Ordering::SeqCst) + 2,
        Ordering::SeqCst,
    );
    assert!(commit_claims(fixture.session.app.world_mut(), &[1, 2]).is_err());
    assert_eq!(fixture.state(), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    let burned = fixture.store.ids();
    assert_eq!(burned.len(), 1);
    fixture.store.fail_at.store(0, Ordering::SeqCst);
    let plan = commit_claims(fixture.session.app.world_mut(), &[1, 2]).unwrap();
    assert_eq!(plan.claim.gold, 12);
    assert_eq!(plan.gold, 112);
    assert_eq!(plan.credit, 333);
    let mut expected = try_user_item_from_item_state(&original).unwrap();
    let assigned = try_user_item_from_item_state(&plan.claim.items[0]).unwrap();
    expected.unique_id = assigned.unique_id;
    expected.slots[1].as_mut().unwrap().unique_id = assigned.slots[1].as_ref().unwrap().unique_id;
    assert_eq!(assigned, expected);
    assert!(assigned.unique_id > burned[0]);
    assert!(assigned.slots[1].as_ref().unwrap().unique_id > assigned.unique_id);
    let issued = fixture.store.ids();
    assert!(commit_claims(fixture.session.app.world_mut(), &[1, 2]).is_err());
    assert_eq!(fixture.store.ids(), issued);
    fixture.session.save_active_character().unwrap();
    let durable: AccountStore = serde_json::from_slice(&fs::read(&fixture.path).unwrap()).unwrap();
    assert_eq!(durable.accounts["demo"].saves[&0].gold, 112);
    assert!(systems(&durable.accounts["demo"].saves[&0])
        .unwrap()
        .mail
        .iter()
        .all(|mail| mail.claimed));
}

#[test]
fn mail_uid_commit_merges_unrelated_new_mail_and_advances_private_revision() {
    let mut fixture = Fixture::new("external", vec![mail(1, vec![nested()], 5)]);
    let config = fixture.config.clone();
    let mut external = nested();
    *fixture.store.callback.lock().unwrap() = Some(Box::new(move |uid| {
        external.unique_id = uid + 100;
        external.user_item_metadata = None;
        external.socketed.clear();
        external.socket_slots = 0;
        external.quantity = 1;
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        let mut state = systems(save).unwrap();
        state.mail.push(mail(7, vec![external], 9));
        save.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
        save.revision += 1;
    }));
    let plan = commit_claims(fixture.session.app.world_mut(), &[1]).unwrap();
    assert!(plan
        .systems
        .mail
        .iter()
        .any(|mail| mail.id == 1 && mail.claimed));
    assert!(plan
        .systems
        .mail
        .iter()
        .any(|mail| mail.id == 7 && !mail.claimed && mail.gold == 9));
    fixture.session.save_active_character().unwrap();
    let stored = fixture.config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert!(systems(&stored)
        .unwrap()
        .mail
        .iter()
        .any(|mail| mail.id == 7 && !mail.claimed));
}

#[test]
fn mail_uid_changed_touched_delivery_rejects_without_publishing_reserved_assets() {
    let mut fixture = Fixture::new("changed-delivery", vec![mail(1, vec![nested()], 5)]);
    let before = fixture.state();
    let bytes = fs::read(&fixture.path).unwrap();
    let config = fixture.config.clone();
    *fixture.store.callback.lock().unwrap() = Some(Box::new(move |_| {
        let mut store = config.account_store.lock().unwrap();
        let save = store
            .accounts
            .get_mut("demo")
            .unwrap()
            .saves
            .get_mut(&0)
            .unwrap();
        let mut state = systems(save).unwrap();
        state.mail[0].locked = true;
        save.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
        save.revision += 1;
    }));
    assert!(commit_claims(fixture.session.app.world_mut(), &[1])
        .unwrap_err()
        .contains("delivery changed"));
    assert!(!fixture.store.ids().is_empty());
    assert_eq!(fixture.state(), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
}

#[test]
fn mail_uid_monthly_card_authorization_keeps_deterministic_identity_without_ordinary_mint() {
    let mut fixture = Fixture::new("monthly", Vec::new());
    let active = snapshot_active_character_save(fixture.session.app.world()).unwrap();
    let items = fixture
        .config
        .commit_account_store_transaction(&["demo".into()], |store| {
            billing_monthly_card::issue_paid_items(
                store.accounts.get_mut("demo").unwrap(),
                &active,
                "demo",
                0,
                "owned-mail-card",
                1,
                1000,
                1,
                crate::monthly_card::MonthlyCardItemDelivery::Mail,
            )
            .map(|(items, _)| items)
        })
        .unwrap();
    let uid = items[0].unique_id;
    fixture
        .session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .mail = vec![{
        let mut mail = mail(1, items, 0);
        mail.attachment_origin = Stage5MailAttachmentOrigin::LegacyUnknown;
        mail
    }];
    fixture.session.save_active_character().unwrap();
    let before = fixture.store.ids();
    let plan = commit_claims(fixture.session.app.world_mut(), &[1]).unwrap();
    assert_eq!(plan.claim.items[0].unique_id, uid);
    assert_eq!(fixture.store.ids(), before);
    fixture.session.save_active_character().unwrap();
}

#[test]
fn mail_uid_mixed_ledger_card_and_fresh_tree_batch_preserves_card_carrier_and_consumes_once() {
    let original = nested();
    let mut fixture = Fixture::new("monthly-mixed", vec![mail(2, vec![original.clone()], 5)]);
    let active = snapshot_active_character_save(fixture.session.app.world()).unwrap();
    let cards = fixture
        .config
        .commit_account_store_transaction(&["demo".into()], |store| {
            billing_monthly_card::issue_paid_items(
                store.accounts.get_mut("demo").unwrap(),
                &active,
                "demo",
                0,
                "owned-mixed-mail-card",
                1,
                1000,
                1,
                crate::monthly_card::MonthlyCardItemDelivery::Mail,
            )
            .map(|(items, _)| items)
        })
        .unwrap();
    let card_uid = cards[0].unique_id;
    assert!(cards[0].user_item_metadata.is_none());
    let mut card_mail = mail(1, cards, 0);
    card_mail.attachment_origin = Stage5MailAttachmentOrigin::LegacyUnknown;
    fixture
        .session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .mail
        .insert(0, card_mail);
    fixture.session.save_active_character().unwrap();
    let issued = fixture.store.ids().len();
    let plan = commit_claims(fixture.session.app.world_mut(), &[1, 2]).unwrap();
    assert_eq!(plan.claim.items.len(), 2);
    let card = plan
        .claim
        .items
        .iter()
        .find(|item| billing_monthly_card::is_card(item))
        .unwrap();
    assert_eq!(card.unique_id, card_uid);
    assert!(card.user_item_metadata.is_none());
    assert_eq!(card.quantity, 1);
    let ordinary = plan
        .claim
        .items
        .iter()
        .find(|item| !billing_monthly_card::is_card(item))
        .unwrap();
    assert_ne!(ordinary.unique_id, original.unique_id);
    assert_ne!(
        ordinary.socketed[0].unique_id,
        original.socketed[0].unique_id
    );
    assert_eq!(fixture.store.ids().len(), issued + 2);
    assert!(plan
        .systems
        .mail
        .iter()
        .all(|mail| mail.claimed && mail.items.is_empty() && mail.item_states_json.is_empty()));
    let issued_once = fixture.store.ids();
    assert!(commit_claims(fixture.session.app.world_mut(), &[1, 2]).is_err());
    assert_eq!(fixture.store.ids(), issued_once);
    fixture.session.save_active_character().unwrap();
    let durable: AccountStore = serde_json::from_slice(&fs::read(&fixture.path).unwrap()).unwrap();
    let save = &durable.accounts["demo"].saves[&0];
    assert_eq!(save.gold, 105);
    assert!(systems(save).unwrap().mail.iter().all(|mail| mail.claimed));
    let stored: Vec<ItemState> = save
        .inventory_items_json
        .iter()
        .map(|encoded| serde_json::from_str(encoded).unwrap())
        .collect();
    let stored_card = stored
        .iter()
        .find(|item| billing_monthly_card::is_card(item))
        .unwrap();
    assert_eq!(stored_card.unique_id, card_uid);
    assert!(stored_card.user_item_metadata.is_none());
    assert_eq!(
        serde_json::to_value(stored_card).unwrap(),
        serde_json::to_value(card).unwrap()
    );
}

#[test]
fn mail_uid_unknown_freezes_source_clones_and_never_reissues_or_publishes_plan() {
    let mut fixture = Fixture::new("unknown", vec![mail(1, vec![nested()], 5)]);
    let before = fixture.state();
    let bytes = fs::read(&fixture.path).unwrap();
    let clone = fixture.config.clone();
    fixture.store.unknown.store(true, Ordering::SeqCst);
    assert!(commit_claims(fixture.session.app.world_mut(), &[1]).is_err());
    assert!(clone.ensure_account_store_writable().is_err());
    assert_eq!(fixture.state(), before);
    assert_eq!(fs::read(&fixture.path).unwrap(), bytes);
    let burned = fixture.store.ids();
    assert_eq!(burned.len(), 1);
    assert!(commit_claims(fixture.session.app.world_mut(), &[1]).is_err());
    assert_eq!(fixture.store.ids(), burned);
}

#[test]
fn mail_uid_committed_source_with_failed_private_revision_publication_freezes_without_retry() {
    let mut fixture = Fixture::new("publication", vec![mail(1, vec![nested()], 5)]);
    let before = fixture.state();
    let session = fixture
        .session
        .app
        .world()
        .resource::<SessionResource>()
        .clone();
    let revision = session.active_save_revision().unwrap();
    *fixture.store.callback.lock().unwrap() = Some(Box::new(move |_| {
        session.bind_active_save_revision(revision + 10);
    }));
    let clone = fixture.config.clone();
    let error = commit_claims(fixture.session.app.world_mut(), &[1]).unwrap_err();
    assert!(error.starts_with("SOURCE_COMMITTED_LIVE_PUBLICATION_FAILED"));
    assert!(clone.ensure_account_store_writable().is_err());
    assert_eq!(fixture.state(), before);
    let durable: AccountStore = serde_json::from_slice(&fs::read(&fixture.path).unwrap()).unwrap();
    let save = &durable.accounts["demo"].saves[&0];
    assert_eq!(save.gold, 105);
    assert_eq!(save.revision, revision + 1);
    assert!(systems(save).unwrap().mail[0].claimed);
    assert_ne!(save.inventory_items_json, Vec::<String>::new());
    let issued = fixture.store.ids();
    assert_eq!(issued.len(), 2);
    assert!(commit_claims(fixture.session.app.world_mut(), &[1]).is_err());
    assert_eq!(fixture.store.ids(), issued);
    assert_eq!(fixture.state(), before);
}

#[test]
fn mail_uid_direct_stage5_claim_uses_the_same_issuer_and_unavailable_never_falls_back() {
    let mut fixture = Fixture::new("direct", vec![mail(1, vec![nested()], 5)]);
    fixture
        .session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .item_uid_issuance = item_uid_issuance::ItemUidIssuance::Unavailable;
    let before = fixture.state();
    let issued = fixture.store.ids();
    assert!(matches!(
        super::super::stage5_claim_mail_authoritative(fixture.session.app.world_mut(), 1),
        Err(Stage5MailClaimError::AuthorityUnavailable)
    ));
    assert_eq!(fixture.state(), before);
    assert_eq!(fixture.store.ids(), issued);
    let allocator = fixture.config.server_item_uid_allocator().unwrap();
    fixture
        .session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .item_uid_issuance = item_uid_issuance::ItemUidIssuance::Durable(allocator);
    let claimed =
        super::super::stage5_claim_mail_authoritative(fixture.session.app.world_mut(), 1).unwrap();
    assert_eq!(claimed.gold, 5);
    assert_eq!(fixture.store.ids().len(), issued.len() + 2);
    assert!(claimed.items[0].unique_id > 1_000_000);
    fixture.session.save_active_character().unwrap();
}

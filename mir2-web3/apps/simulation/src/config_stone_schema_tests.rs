//! File-format compatibility for the stone registry and existing economy ledgers.
use super::{
    AccountStore, AccountStoreRepository, CharacterRecord, FileAccountStoreRepository,
    SimulationConfig, ACCOUNT_STORE_SCHEMA_VERSION,
};
use crate::activities::{
    ActivityClaimRecord, ActivityLedger, ActivityPhase, ActivityPhaseRecord, ActivityRewardKind,
};
use crate::billing::{
    BillingLedger, RechargeAppliedReceipt, RechargeOffer, RechargeOrder, RechargeOrderState,
};
use crate::monthly_card::{
    monthly_card_item_identity, MonthlyCardItemDelivery, MonthlyCardItemReceipt, MonthlyCardLedger,
    MONTHLY_CARD_DURATION_MS,
};
use mir2_production::{
    StoneBinding, StoneCatalog, StoneGrade, StoneMintPreparation, StoneMintRequest, StoneOwner,
    StoneRegistry, StoneSource, StoneSourceKind,
};
use mir2_protocol::{MirClass, MirGender};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct FileFixture {
    directory: PathBuf,
    path: PathBuf,
    bytes: Vec<u8>,
}

impl FileFixture {
    fn new(label: &str, value: &Value) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "mir2-stone-schema-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("accounts.json");
        let bytes = serde_json::to_vec_pretty(value).unwrap();
        fs::write(&path, &bytes).unwrap();
        Self {
            directory,
            path,
            bytes,
        }
    }

    fn assert_source_unchanged(&self) {
        assert_eq!(fs::read(&self.path).unwrap(), self.bytes);
    }
}

impl Drop for FileFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[derive(Clone, Copy)]
enum ExistingLedger {
    Billing,
    MonthlyCard,
    Activities,
    Combined,
}

fn character() -> CharacterRecord {
    CharacterRecord {
        index: 0,
        name: "SchemaKeeper".into(),
        level: 40,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    }
}

fn billing_history() -> BillingLedger {
    let id = format!("rch_{}", "a".repeat(64));
    let order = RechargeOrder {
        id: id.clone(),
        request_id: "schema-billing-request".into(),
        character_index: 0,
        offer: RechargeOffer {
            id: "schema-offer".into(),
            label: "Existing schema nine offer".into(),
            currency: "usd".into(),
            amount_minor: 100,
            credits: 100,
        },
        created_at_ms: 1_000,
        livemode: false,
        checkout_session_id: Some("cs_schema_existing".into()),
        payment_intent_id: Some("pi_schema_existing".into()),
        state: RechargeOrderState::Applied,
        paid_at_ms: Some(1_010),
        applied: Some(RechargeAppliedReceipt {
            character_index: 0,
            credits: 100,
            credit_before: 400,
            credit_after: 500,
            revision: 7,
            at_ms: 1_020,
        }),
        review_events: BTreeMap::new(),
    };
    let ledger = BillingLedger {
        orders: BTreeMap::from([(id, order)]),
    };
    ledger.validate().unwrap();
    ledger
}

fn monthly_card_history() -> MonthlyCardLedger {
    let request = "schema-monthly-item";
    let (key, unique_id) = monthly_card_item_identity("demo", 0, request, 0);
    let expires_at_ms = 1_100 + MONTHLY_CARD_DURATION_MS;
    let ledger = MonthlyCardLedger {
        expires_at_ms,
        codes: BTreeMap::new(),
        item_receipts: BTreeMap::from([(
            key,
            MonthlyCardItemReceipt {
                account_id: "demo".into(),
                character_index: 0,
                request_id: request.into(),
                ordinal: 0,
                purchase_quantity: 1,
                unique_id,
                credit_price: 2_500,
                delivery: MonthlyCardItemDelivery::Bag,
                issued_at_ms: 1_000,
                redeemed_at_ms: Some(1_100),
                credited_until_ms: Some(expires_at_ms),
            },
        )]),
    };
    ledger.validate().unwrap();
    ledger
}

fn activity_history() -> ActivityLedger {
    let ledger = ActivityLedger {
        phases: BTreeMap::from([(
            "closed-v1".into(),
            ActivityPhaseRecord {
                phase: ActivityPhase::Closed,
                start_ms: 1_000,
                end_ms: Some(10_000),
                max_level: 40,
                qualified_at_ms: 1_100,
            },
        )]),
        claims: BTreeMap::from([(
            "closed-v1-lv40".into(),
            ActivityClaimRecord {
                reward_id: "closed-v1-lv40".into(),
                phase: ActivityPhase::Closed,
                kind: ActivityRewardKind::Title,
                amount: 1,
                qualified_level: 40,
                character_index: 0,
                character_name: character().name,
                class: MirClass::Warrior,
                gender: MirGender::Male,
                claimed_at_ms: 1_200,
            },
        )]),
        recharge_records: BTreeMap::new(),
    };
    ledger.validate().unwrap();
    ledger
}

fn existing_store(kind: ExistingLedger) -> AccountStore {
    let mut store = AccountStore::new(character());
    store.schema_version = 9;
    let account = store.accounts.get_mut("demo").unwrap();
    let save = account.saves.get_mut(&0).unwrap();
    save.revision = 7;
    save.gold = 12_345;
    save.credit = 500;
    if matches!(kind, ExistingLedger::Billing | ExistingLedger::Combined) {
        account.billing = Some(billing_history());
    }
    if matches!(kind, ExistingLedger::MonthlyCard | ExistingLedger::Combined) {
        account.monthly_card = Some(monthly_card_history());
    }
    if matches!(kind, ExistingLedger::Activities | ExistingLedger::Combined) {
        account.activities = Some(activity_history());
    }
    crate::billing::validate_complete_store(&store).unwrap();
    store
}

fn assert_schema_nine_file_migration(label: &str, kind: ExistingLedger) {
    assert_eq!(crate::billing::BILLING_SCHEMA_VERSION, 9);
    assert_eq!(ACCOUNT_STORE_SCHEMA_VERSION, 10);
    let before = existing_store(kind);
    let mut legacy = serde_json::to_value(&before).unwrap();
    // A genuine pre-workshop schema nine file has no sealedStones field.
    legacy.as_object_mut().unwrap().remove("sealedStones");
    let fixture = FileFixture::new(label, &legacy);
    let loaded = FileAccountStoreRepository::new(&fixture.path)
        .load(character())
        .unwrap();
    fixture.assert_source_unchanged();

    let mut expected = serde_json::to_value(&before).unwrap();
    expected["schemaVersion"] = Value::from(10);
    assert_eq!(serde_json::to_value(&loaded).unwrap(), expected);
    assert_eq!(loaded.sealed_stones, StoneRegistry::default());

    // Persist through the normal shared File authority, then reopen it. Merely
    // decoding a JSON object would miss publication-time schema/ledger checks.
    let config = SimulationConfig::default().with_account_store_path(&fixture.path);
    config.ensure_account_store_writable().unwrap();
    fixture.assert_source_unchanged();
    config.save_account_store().unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&fixture.path).unwrap()).unwrap(),
        expected,
    );
    drop(config);
    let reopened = FileAccountStoreRepository::new(&fixture.path)
        .load(character())
        .unwrap();
    assert_eq!(serde_json::to_value(&reopened).unwrap(), expected);
}

#[test]
fn schema_nine_applied_billing_file_migrates_without_losing_payment_receipts() {
    assert_schema_nine_file_migration("billing", ExistingLedger::Billing);
}

#[test]
fn schema_nine_monthly_item_file_migrates_without_resetting_activation_history() {
    assert_schema_nine_file_migration("monthly", ExistingLedger::MonthlyCard);
}

#[test]
fn schema_nine_activity_file_migrates_without_reopening_claims() {
    assert_schema_nine_file_migration("activities", ExistingLedger::Activities);
}

#[test]
fn schema_nine_combined_ledgers_preserve_the_complete_file_image_after_migration() {
    assert_schema_nine_file_migration("combined", ExistingLedger::Combined);
}

fn nonempty_stone_registry() -> StoneRegistry {
    let request = StoneMintRequest {
        owner: StoneOwner {
            account_id: "demo".into(),
            character_incarnation: "schema-character-incarnation".into(),
        },
        request_id: "schema-stone-mint".into(),
        serial: 1,
        carrier_uid: 9_007_199_254_740_993,
        source_template_id: 824,
        raw_template_id: mir2_game_data::RAW_STONE_ROUGH,
        raw_weight: 4,
        grade: StoneGrade::Rough,
        binding: StoneBinding::default(),
        source: StoneSource {
            kind: StoneSourceKind::MiningWorkshop,
            source_version: "v1".into(),
            event_id: "schema-stone-source".into(),
        },
    };
    let registry = match StoneRegistry::default()
        .prepare_mint(&StoneCatalog::v1(), &request, 0)
        .unwrap()
    {
        StoneMintPreparation::Change(plan) => plan.next_registry,
        StoneMintPreparation::Replay(_) => panic!("a fresh registry cannot replay a mint"),
    };
    registry.validate().unwrap();
    assert_ne!(registry, StoneRegistry::default());
    registry
}

fn assert_file_load_rejected_without_source_changes(fixture: &FileFixture, expected: &str) {
    let error = FileAccountStoreRepository::new(&fixture.path)
        .load(character())
        .unwrap_err();
    assert!(error.contains(expected), "unexpected rejection: {error}");
    fixture.assert_source_unchanged();
    let config = SimulationConfig::default().with_account_store_path(&fixture.path);
    assert!(config.ensure_account_store_writable().is_err());
    assert!(config.file_authority.is_none());
    fixture.assert_source_unchanged();
}

#[test]
fn old_schema_six_or_nine_cannot_import_a_valid_nonempty_stone_registry() {
    for version in [6, 9] {
        let mut store = AccountStore::new(character());
        store.schema_version = version;
        store.sealed_stones = nonempty_stone_registry();
        let fixture = FileFixture::new(
            &format!("old-stones-{version}"),
            &serde_json::to_value(&store).unwrap(),
        );
        assert_file_load_rejected_without_source_changes(
            &fixture,
            "stone authority present in an older schema",
        );
    }
}

#[test]
fn future_schema_eleven_with_each_existing_ledger_is_rejected_without_rewriting() {
    for (label, kind, expected) in [
        ("billing", ExistingLedger::Billing, "billingSchemaInvalid"),
        (
            "monthly",
            ExistingLedger::MonthlyCard,
            "billingSchemaInvalid",
        ),
        (
            "activities",
            ExistingLedger::Activities,
            "activitiesSchemaInvalid",
        ),
    ] {
        let mut store = existing_store(kind);
        store.schema_version = 11;
        let fixture = FileFixture::new(
            &format!("future-{label}"),
            &serde_json::to_value(&store).unwrap(),
        );
        assert_file_load_rejected_without_source_changes(&fixture, expected);
    }
}

#[test]
fn future_schema_without_economy_ledgers_is_also_rejected() {
    let mut store = AccountStore::new(character());
    store.schema_version = 11;
    let fixture = FileFixture::new("future-empty", &serde_json::to_value(&store).unwrap());
    assert_file_load_rejected_without_source_changes(&fixture, "unsupported account store schema");
}

#[test]
fn schema_ten_still_rejects_cross_account_reuse_of_a_provider_payment() {
    let mut store = existing_store(ExistingLedger::Billing);
    store.schema_version = ACCOUNT_STORE_SCHEMA_VERSION;
    crate::billing::validate_complete_store(&store).unwrap();
    let mut second = store.accounts["demo"].clone();
    let mut order = second
        .billing
        .as_mut()
        .unwrap()
        .orders
        .pop_first()
        .unwrap()
        .1;
    order.id = format!("rch_{}", "b".repeat(64));
    order.request_id = "schema-second-request".into();
    order.checkout_session_id = Some("cs_schema_second".into());
    second
        .billing
        .as_mut()
        .unwrap()
        .orders
        .insert(order.id.clone(), order);
    // Each ledger is valid in isolation; only the complete-store provider
    // binding fence detects that the same payment was used by another account.
    second.billing.as_ref().unwrap().validate().unwrap();
    store
        .accounts
        .insert("schema-second-account".into(), second);
    let fixture = FileFixture::new("duplicate-provider", &serde_json::to_value(&store).unwrap());
    assert_file_load_rejected_without_source_changes(&fixture, "billingProviderBindingAlreadyUsed");
}

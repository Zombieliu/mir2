//! File boundary checks for the merged billing/activity and stone schema.
//! Ledger fixtures come from their real rules; these are not live realm tests.
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use mir2_production::{
    StoneBinding, StoneCatalog, StoneGrade, StoneMintPreparation, StoneMintRequest, StoneOwner,
    StoneRegistry, StoneSource, StoneSourceKind,
};

use super::{AccountStoreRepository, FileAccountStoreRepository, SimulationConfig};
use crate::activities::{ActivityPhase, ActivityPolicy};
use crate::billing::{RechargeOffer, VerifiedRechargePayment, BILLING_SCHEMA_VERSION};
use crate::monthly_card::MonthlyCardPolicy;

const NOW: u64 = 1_900_000_000_000;
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct StoreFile {
    directory: PathBuf,
    path: PathBuf,
}

impl StoreFile {
    fn new(label: &str, bytes: &[u8]) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "mir2-schema-integration-{label}-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("accounts.json");
        fs::write(&path, bytes).unwrap();
        Self { directory, path }
    }
}

impl Drop for StoreFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

fn existing_authorities() -> SimulationConfig {
    let config = SimulationConfig::default().with_monthly_card_policy(
        MonthlyCardPolicy::new(
            false,
            Some("test-only-schema-integration-monthly-key-never-deploy"),
        )
        .unwrap(),
    );
    let character_index = config.default_character.index;
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("demo").unwrap();
        account.characters[0].level = 40;
        let save = account.saves.get_mut(&character_index).unwrap();
        save.character.level = 40;
        save.gold = 321;
        save.credit = 87;
    }
    let order = config
        .prepare_recharge_order(
            "demo",
            character_index,
            "schema-integration-recharge",
            &RechargeOffer {
                id: "schema-usd-1000".into(),
                label: "Schema fixture USD10".into(),
                currency: "usd".into(),
                amount_minor: 1000,
                credits: 1000,
            },
            NOW,
            false,
        )
        .unwrap();
    config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &VerifiedRechargePayment {
                session_id: "cs_schema_integration".into(),
                payment_intent_id: "pi_schema_integration".into(),
                amount_minor: 1000,
                currency: "usd".into(),
                livemode: false,
            },
            NOW + 1,
        )
        .unwrap();
    let issued = config
        .issue_monthly_card("demo", "schema-monthly", NOW + 2)
        .unwrap();
    config
        .redeem_monthly_card("demo", &issued.code, NOW + 3)
        .unwrap();
    config
        .activity_claim(
            "demo",
            character_index,
            "closed-v1-lv40",
            &ActivityPolicy {
                phase: ActivityPhase::Closed,
                phase_start_ms: NOW - 100,
                phase_end_ms: None,
            },
            NOW + 4,
        )
        .unwrap();
    config
}

fn valid_sealed_stones() -> StoneRegistry {
    let request = StoneMintRequest {
        owner: StoneOwner {
            account_id: "demo".into(),
            character_incarnation: "schema-stone-incarnation".into(),
        },
        request_id: "schema-stone-mint".into(),
        serial: 1,
        carrier_uid: 120_000_001,
        source_template_id: 824,
        raw_template_id: mir2_game_data::RAW_STONE_ROUGH,
        raw_weight: 4,
        grade: StoneGrade::Rough,
        binding: StoneBinding::default(),
        source: StoneSource {
            kind: StoneSourceKind::MiningWorkshop,
            source_version: "schema-integration-v1".into(),
            event_id: "schema-stone-source".into(),
        },
    };
    let StoneMintPreparation::Change(prepared) = StoneRegistry::default()
        .prepare_mint(&StoneCatalog::v1(), &request, 9999)
        .unwrap()
    else {
        panic!("fresh fixture mint must prepare a change");
    };
    let registry = prepared.next_registry;
    registry.validate().unwrap();
    assert!(registry.record(1).is_some());
    registry
}

#[test]
fn schema9_migration_preserves_existing_billing_activities_monthly_card_and_stones() {
    let config = existing_authorities();
    let mut legacy = config.account_store.lock().unwrap().clone();
    crate::billing::validate_complete_store(&legacy).unwrap();
    let account = &legacy.accounts["demo"];
    assert!(!account.billing.as_ref().unwrap().orders.is_empty());
    assert!(!account.activities.as_ref().unwrap().claims.is_empty());
    let card = account.monthly_card.as_ref().unwrap();
    card.validate().unwrap();
    assert!(!card.codes.is_empty());
    assert!(card.expires_at_ms > NOW);
    legacy.sealed_stones = valid_sealed_stones();
    let before_stones = legacy.sealed_stones.clone();
    let before_accounts = serde_json::to_value(&legacy.accounts).unwrap();
    legacy.schema_version = 9;
    let bytes = serde_json::to_vec(&legacy).unwrap();
    let file = StoreFile::new("schema9", &bytes);

    let loaded = FileAccountStoreRepository::new(&file.path)
        .load(config.default_character.clone())
        .unwrap();

    assert_eq!(loaded.schema_version, BILLING_SCHEMA_VERSION);
    assert_eq!(
        serde_json::to_value(&loaded.accounts).unwrap(),
        before_accounts
    );
    assert_eq!(loaded.sealed_stones, before_stones);
    assert_eq!(
        fs::read(&file.path).unwrap(),
        bytes,
        "a read must not rewrite the source"
    );
}

#[test]
fn schema6_through12_stone_authority_survives_current_schema_upgrade() {
    for version in 6..=12 {
        let config = SimulationConfig::default();
        let mut tagged = config.account_store.lock().unwrap().clone();
        tagged.schema_version = version;
        tagged.sealed_stones = valid_sealed_stones();
        let before_accounts = serde_json::to_value(&tagged.accounts).unwrap();
        let before_stones = tagged.sealed_stones.clone();
        let bytes = serde_json::to_vec(&tagged).unwrap();
        let file = StoreFile::new("legacy-stones", &bytes);
        let repository = FileAccountStoreRepository::new(&file.path);
        let loaded = repository.load(config.default_character.clone()).unwrap();
        assert_eq!(loaded.schema_version, BILLING_SCHEMA_VERSION);
        assert_eq!(loaded.sealed_stones, before_stones);
        assert_eq!(serde_json::to_value(&loaded.accounts).unwrap(), before_accounts);
        assert_eq!(fs::read(&file.path).unwrap(), bytes, "read-only upgrade must not rewrite source");
        repository.save(&loaded).unwrap();
        let reopened = repository.load(config.default_character).unwrap();
        assert_eq!(reopened.sealed_stones, before_stones);
        assert_eq!(serde_json::to_value(&reopened.accounts).unwrap(), before_accounts);
    }
}

#[test]
fn schema5_sealed_stone_authority_is_rejected_on_load_and_restore() {
    let config = SimulationConfig::default();
    let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    let mut tagged = config.account_store.lock().unwrap().clone();
    tagged.sealed_stones = valid_sealed_stones();
    let valid_bytes = serde_json::to_vec(&tagged).unwrap();
    let file = StoreFile::new("stone-tag", &valid_bytes);
    let repository = FileAccountStoreRepository::new(&file.path);
    let valid = repository.load(config.default_character.clone()).unwrap();
    assert_eq!(
        valid.sealed_stones, tagged.sealed_stones,
        "the same nonempty registry must be valid under the current schema"
    );

    tagged.schema_version = 5;
    let migrated = tagged.clone().migrate_to_current_schema();
    assert_eq!(
        migrated.schema_version, 5,
        "an authority field cannot silently acquire a newer schema tag"
    );
    assert_eq!(migrated.sealed_stones, tagged.sealed_stones);
    let rejected_bytes = serde_json::to_vec(&tagged).unwrap();
    fs::write(&file.path, &rejected_bytes).unwrap();
    assert_eq!(
        repository
            .load(config.default_character.clone())
            .unwrap_err(),
        "stone authority present in an older schema"
    );
    assert_eq!(
        config
            .restore_account_store_from_backup(&file.path)
            .unwrap_err(),
        "stone authority present in an older schema"
    );
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        before
    );
    assert_eq!(fs::read(&file.path).unwrap(), rejected_bytes);
}

#[test]
fn legacy_store_without_schema_or_stone_fields_still_loads_without_account_loss() {
    let config = SimulationConfig::default();
    let mut encoded = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    let expected_accounts = encoded["accounts"].clone();
    let object = encoded.as_object_mut().unwrap();
    object.remove("schemaVersion");
    object.remove("sealedStones");
    let bytes = serde_json::to_vec(&encoded).unwrap();
    let file = StoreFile::new("legacy", &bytes);

    let loaded = FileAccountStoreRepository::new(&file.path)
        .load(config.default_character.clone())
        .unwrap();

    assert_eq!(loaded.schema_version, BILLING_SCHEMA_VERSION);
    assert_eq!(
        serde_json::to_value(&loaded.accounts).unwrap(),
        expected_accounts
    );
    assert_eq!(loaded.sealed_stones, StoneRegistry::default());
    assert_eq!(fs::read(&file.path).unwrap(), bytes);
}

//! Persisted files from both pre-integration branches retain their authorities.
use super::*;
use crate::billing::{BillingLedger, RechargeOffer, RechargeOrder, RechargeOrderState};
use mir2_production::{
    StoneBinding, StoneCatalog, StoneGrade, StoneMintPreparation, StoneMintRequest, StoneOwner,
    StoneRegistry, StoneSource, StoneSourceKind,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

struct FixtureFile(PathBuf);
impl FixtureFile {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "mir2-schema-union-{}-{}.json",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for FixtureFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn base_store(version: u16) -> (AccountStore, CharacterRecord) {
    let config = SimulationConfig::default();
    let mut store = config.account_store.lock().unwrap().clone();
    store.schema_version = version;
    (store, config.default_character.clone())
}

fn sealed_stone() -> StoneRegistry {
    let request = StoneMintRequest {
        owner: StoneOwner {
            account_id: "demo".into(),
            character_incarnation: "legacy-character".into(),
        },
        request_id: "legacy-mint".into(),
        serial: 41,
        carrier_uid: 123_456,
        source_template_id: 824,
        raw_template_id: 980_001,
        raw_weight: 4,
        grade: StoneGrade::Rough,
        binding: StoneBinding::default(),
        source: StoneSource {
            kind: StoneSourceKind::MiningWorkshop,
            source_version: "mining-workshop-v1".into(),
            event_id: "legacy-source-41".into(),
        },
    };
    match StoneRegistry::default()
        .prepare_mint(&StoneCatalog::v1(), &request, 8_500)
        .unwrap()
    {
        StoneMintPreparation::Change(plan) => plan.next_registry,
        StoneMintPreparation::Replay(_) => panic!("first mint cannot replay"),
    }
}

fn paid_ledger() -> BillingLedger {
    let id = format!("rch_{}", "a".repeat(64));
    let order = RechargeOrder {
        id: id.clone(),
        request_id: "legacy-paid-order".into(),
        character_index: 0,
        offer: RechargeOffer {
            id: "legacy-offer".into(),
            label: "Legacy offer".into(),
            currency: "usd".into(),
            amount_minor: 100,
            credits: 100,
        },
        created_at_ms: 1,
        livemode: false,
        checkout_session_id: Some("cs_legacy".into()),
        payment_intent_id: Some("pi_legacy".into()),
        state: RechargeOrderState::PaidPending,
        paid_at_ms: Some(2),
        applied: None,
        review_events: BTreeMap::new(),
    };
    BillingLedger {
        orders: BTreeMap::from([(id, order)]),
        ..Default::default()
    }
}

fn redeemed_monthly_card() -> crate::monthly_card::MonthlyCardLedger {
    let config = SimulationConfig::default().with_monthly_card_policy(
        crate::monthly_card::MonthlyCardPolicy::new(
            false,
            Some("schema-union-test-issuance-key-never-deploy"),
        )
        .unwrap(),
    );
    let issued = config
        .issue_monthly_card("demo", "schema-union-legacy-card", 100)
        .unwrap();
    config
        .redeem_monthly_card("demo", &issued.code, 101)
        .unwrap();
    let store = config.account_store.lock().unwrap();
    let card = store.accounts["demo"].monthly_card.clone().unwrap();
    card.validate().unwrap();
    assert!(!card.codes.is_empty());
    assert!(card.expires_at_ms > 101);
    card
}

fn assert_file_round_trip(store: AccountStore, character: CharacterRecord) {
    let file = FixtureFile::new();
    let mut expected = serde_json::to_value(&store).unwrap();
    fs::write(&file.0, serde_json::to_vec(&expected).unwrap()).unwrap();
    let repository = FileAccountStoreRepository::new(&file.0);
    let loaded = repository.load(character.clone()).unwrap();
    assert_eq!(loaded.schema_version, 10);
    expected["schemaVersion"] = 10.into();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), expected);
    repository.save(&loaded).unwrap();
    let reopened = repository.load(character).unwrap();
    assert_eq!(serde_json::to_value(reopened).unwrap(), expected);
}

#[test]
fn schema_union_preserves_both_version_six_authorities_and_legacy_five() {
    let (store, character) = base_store(5);
    assert_file_round_trip(store, character);

    let (mut stone_store, character) = base_store(6);
    stone_store.sealed_stones = sealed_stone();
    assert_file_round_trip(stone_store, character);

    let (mut conquest_store, character) = base_store(6);
    conquest_store
        .shared_conquests
        .insert(1, crate::conquest::SharedConquestRecord::new(1));
    assert_file_round_trip(conquest_store, character);
}

#[test]
fn schema_union_preserves_version_seven_eight_and_nine_ledgers() {
    for version in 7..=9 {
        let (mut store, character) = base_store(version);
        let account = store.accounts.get_mut("demo").unwrap();
        account.monthly_card = Some(redeemed_monthly_card());
        if version >= 8 {
            account.billing = Some(paid_ledger());
        }
        if version >= 9 {
            let mut ledger = crate::activities::ActivityLedger::default();
            ledger.phases.insert(
                "closed-v1".into(),
                crate::activities::ActivityPhaseRecord {
                    phase: crate::activities::ActivityPhase::Closed,
                    start_ms: 100,
                    end_ms: None,
                    max_level: 40,
                    qualified_at_ms: 101,
                },
            );
            account.activities = Some(ledger);
        }
        assert_file_round_trip(store, character);
    }
}

#[test]
fn schema_union_cannot_promote_authorities_from_pre_introduction_tags() {
    let (mut stone_store, _) = base_store(5);
    stone_store.sealed_stones = sealed_stone();
    let stone_store = stone_store.migrate_to_current_schema();
    assert_eq!(stone_store.schema_version, 5);
    assert!(stone_authority::validate_store(&stone_store).is_err());

    let (mut conquest_store, _) = base_store(5);
    conquest_store
        .shared_conquests
        .insert(1, crate::conquest::SharedConquestRecord::new(1));
    let conquest_store = conquest_store.migrate_to_current_schema();
    assert_eq!(conquest_store.schema_version, 5);
    assert!(shared_conquest_store::validate_complete_state(&conquest_store).is_err());

    let (mut monthly_store, _) = base_store(6);
    monthly_store.accounts.get_mut("demo").unwrap().monthly_card = Some(Default::default());
    let monthly_store = monthly_store.migrate_to_current_schema();
    assert_eq!(monthly_store.schema_version, 6);
    assert!(crate::billing::validate_complete_store(&monthly_store).is_err());

    let (mut billing_store, _) = base_store(7);
    billing_store.accounts.get_mut("demo").unwrap().billing = Some(paid_ledger());
    let billing_store = billing_store.migrate_to_current_schema();
    assert_eq!(billing_store.schema_version, 7);
    assert!(crate::billing::validate_complete_store(&billing_store).is_err());

    let (mut activity_store, _) = base_store(8);
    activity_store.accounts.get_mut("demo").unwrap().activities = Some(Default::default());
    let activity_store = activity_store.migrate_to_current_schema();
    assert_eq!(activity_store.schema_version, 8);
    assert!(crate::billing::validate_complete_store(&activity_store).is_err());
}

#[test]
fn schema_union_migration_and_source_merge_keep_both_domain_versions() {
    let (mut store, _) = base_store(6);
    store.sealed_stones = sealed_stone();
    store
        .shared_conquests
        .insert(1, crate::conquest::SharedConquestRecord::new(1));
    store.source_stone_version = Some(7);
    store.source_conquest_versions.insert(1, 11);
    let mut store = store.migrate_to_current_schema();
    assert_eq!(store.source_stone_version, Some(7));
    assert_eq!(store.source_conquest_versions.get(&1), Some(&11));
    store.merge_source_versions(AccountStoreSourceVersions {
        stones: Some(8),
        conquests: BTreeMap::from([(1, 12)]),
        ..Default::default()
    });
    assert_eq!(store.source_stone_version, Some(8));
    assert_eq!(store.source_conquest_versions.get(&1), Some(&12));
    assert!(store.sealed_stones.record(41).is_some());
}

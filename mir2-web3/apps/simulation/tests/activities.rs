//! Synthetic account/save fixtures test the campaign authority. They do not
//! modify any public realm, provision a production character or mint rewards.
use mir2_protocol::{MirClass, MirGender};
use mir2_simulation::activities::{
    ActivityPhase, ActivityPolicy, ActivitySettlement, ActivityStatus,
};
use mir2_simulation::billing::{RechargeOffer, RechargeOrder, VerifiedRechargePayment};
use mir2_simulation::{
    AccountRecord, AccountStoreRepository, CharacterRecord, CharacterSaveRecord,
    FileAccountStoreRepository, SimulationConfig,
};

const NOW: u64 = 1_900_000_000_000;

fn policy(phase: ActivityPhase, start: u64) -> ActivityPolicy {
    ActivityPolicy {
        phase,
        phase_start_ms: start,
        phase_end_ms: None,
    }
}
fn closed() -> ActivityPolicy {
    policy(ActivityPhase::Closed, NOW - 100)
}
fn beta() -> ActivityPolicy {
    policy(ActivityPhase::Beta, NOW - 100)
}
fn fixture(level: u16) -> SimulationConfig {
    let config = SimulationConfig::default();
    set_level(&config, level);
    config
}
fn set_level(config: &SimulationConfig, level: u16) {
    let mut store = config.account_store.lock().unwrap();
    let account = store.accounts.get_mut("demo").unwrap();
    account.characters[0].level = level;
    account
        .saves
        .get_mut(&config.default_character.index)
        .unwrap()
        .character
        .level = level;
}
fn reward<'a>(
    status: &'a ActivityStatus,
    id: &str,
) -> &'a mir2_simulation::activities::ActivityRewardStatus {
    status
        .rewards
        .iter()
        .find(|reward| reward.id == id)
        .unwrap()
}
fn snapshot(config: &SimulationConfig) -> serde_json::Value {
    serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
}
fn path(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mir2-activities-{label}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("accounts.json")
}
fn paid(
    config: &SimulationConfig,
    label: &str,
    live: bool,
    created: u64,
    confirmed: u64,
) -> RechargeOrder {
    let order = config
        .prepare_recharge_order(
            "demo",
            config.default_character.index,
            label,
            &RechargeOffer {
                id: "usd-1000".into(),
                label: "USD10".into(),
                currency: "usd".into(),
                amount_minor: 1000,
                credits: 1000,
            },
            created,
            live,
        )
        .unwrap();
    config
        .confirm_recharge_payment(
            "demo",
            &order.id,
            &VerifiedRechargePayment {
                session_id: format!("cs_activities_{label}"),
                payment_intent_id: format!("pi_activities_{label}"),
                amount_minor: 1000,
                currency: "usd".into(),
                livemode: live,
            },
            confirmed,
        )
        .unwrap()
}

#[test]
fn activities_policy_disabled_is_safe_and_enabled_requires_a_trusted_window() {
    assert!(ActivityPolicy::default().validate().is_ok());
    let config = fixture(50);
    let before = snapshot(&config);
    let status = config
        .activity_status("demo", &ActivityPolicy::default(), NOW)
        .unwrap();
    assert_eq!(status.phase, ActivityPhase::Disabled);
    assert!(status
        .rewards
        .iter()
        .all(|reward| !reward.eligible && !reward.claimed));
    assert_eq!(snapshot(&config), before);
    assert_eq!(
        config
            .activity_claim(
                "demo",
                config.default_character.index,
                "closed-v1-lv30",
                &ActivityPolicy::default(),
                NOW
            )
            .unwrap_err(),
        "activityDisabled"
    );
    assert!(policy(ActivityPhase::Closed, 0).validate().is_err());
    assert!(policy(ActivityPhase::Disabled, NOW).validate().is_err());
    assert!(ActivityPolicy {
        phase: ActivityPhase::Beta,
        phase_start_ms: NOW,
        phase_end_ms: Some(NOW)
    }
    .validate()
    .is_err());
}

#[test]
fn activities_disabled_save_hook_does_not_add_legacy_roster_requirements() {
    let config = fixture(1);
    let mut account = config.account_store.lock().unwrap().accounts["demo"].clone();
    account.saves.clear(); // Legacy transaction can temporarily lack a private save.
    let before = serde_json::to_value(&account).unwrap();
    mir2_simulation::activities::observe_account(&mut account, &ActivityPolicy::default(), NOW)
        .unwrap();
    assert_eq!(serde_json::to_value(&account).unwrap(), before);
    assert!(account.activities.is_none());
}

#[test]
fn activities_low_level_foreign_character_and_unknown_reward_do_not_register() {
    let config = fixture(29);
    let index = config.default_character.index;
    let before = snapshot(&config);
    assert_eq!(
        config
            .activity_claim("demo", index, "closed-v1-lv30", &closed(), NOW)
            .unwrap_err(),
        "activityLevelRequired"
    );
    assert_eq!(
        config
            .activity_claim("demo", index + 10, "closed-v1-lv30", &closed(), NOW)
            .unwrap_err(),
        "activityCharacterMissing"
    );
    assert_eq!(
        config
            .activity_claim("missing", index, "closed-v1-lv30", &closed(), NOW)
            .unwrap_err(),
        "activityAccountMissing"
    );
    assert_eq!(
        config
            .activity_claim("demo", index, "fake-free-gold", &closed(), NOW)
            .unwrap_err(),
        "activityRewardMissing"
    );
    assert_eq!(snapshot(&config), before);
}

#[test]
fn activities_progress_reads_real_owned_saved_level_and_ignores_orphan_saves() {
    let config = fixture(1);
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("demo").unwrap();
        account.characters[0].level = 50; // Stale/select-roster value is not qualification authority.
        let mut orphan = account.saves[&config.default_character.index].clone();
        orphan.character.index = 99;
        orphan.character.level = 60;
        account.saves.insert(99, orphan);
    }
    let status = config.activity_status("demo", &closed(), NOW).unwrap();
    assert_eq!(status.max_level, 1);
    assert_eq!(status.characters.len(), 1);
    assert_eq!(status.characters[0].level, 1);
    assert!(!reward(&status, "closed-v1-lv30").eligible);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&config.default_character.index)
        .unwrap()
        .character
        .class = MirClass::Archer;
    assert_eq!(
        config.activity_status("demo", &closed(), NOW).unwrap_err(),
        "activityCharacterIdentityMismatch"
    );
}

#[test]
fn activities_claim_is_account_reserved_and_replay_does_not_mint_items_or_time() {
    let config = fixture(50);
    let index = config.default_character.index;
    let save_before = config.account_store.lock().unwrap().accounts["demo"].saves[&index].clone();
    let status = config
        .activity_claim("demo", index, "closed-v1-lv30", &closed(), NOW)
        .unwrap();
    let row = reward(&status, "closed-v1-lv30");
    assert!(row.claimed && !row.eligible);
    assert_eq!(row.amount, 30);
    assert_eq!(row.claimed_at_ms, Some(NOW));
    let durable = snapshot(&config);
    let repeated = config
        .activity_claim("demo", index, "closed-v1-lv30", &closed(), NOW + 1)
        .unwrap();
    assert_eq!(reward(&repeated, "closed-v1-lv30").claimed_at_ms, Some(NOW));
    assert_eq!(snapshot(&config), durable);
    let store = config.account_store.lock().unwrap();
    let account = &store.accounts["demo"];
    assert_eq!(account.activities.as_ref().unwrap().claims.len(), 1);
    assert_eq!(
        serde_json::to_value(&account.saves[&index]).unwrap(),
        serde_json::to_value(save_before).unwrap()
    );
    assert!(account.monthly_card.is_none());
}

#[test]
fn activities_cosmetic_recipient_class_and_first_claim_are_immutable() {
    let config = fixture(50);
    let alt = CharacterRecord {
        index: 77,
        name: "AltArcher".into(),
        level: 1,
        class: MirClass::Archer,
        gender: MirGender::Female,
    };
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("demo").unwrap();
        account.characters.push(alt.clone());
        account
            .saves
            .insert(alt.index, CharacterSaveRecord::new(alt));
    }
    let status = config
        .activity_claim("demo", 77, "closed-v1-lv50", &closed(), NOW)
        .unwrap();
    assert_eq!(
        reward(&status, "closed-v1-lv50").cosmetic_class,
        Some(MirClass::Archer)
    );
    let before = snapshot(&config);
    assert_eq!(
        config
            .activity_claim(
                "demo",
                config.default_character.index,
                "closed-v1-lv50",
                &closed(),
                NOW + 1
            )
            .unwrap_err(),
        "activityClaimRecipientChanged"
    );
    assert_eq!(snapshot(&config), before);
}

#[test]
fn activities_duplicate_concurrent_claims_have_one_durable_account_receipt() {
    let config = fixture(40);
    let index = config.default_character.index;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles = (0..8)
        .map(|ordinal| {
            let config = config.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                config
                    .activity_claim("demo", index, "closed-v1-lv40", &closed(), NOW + ordinal)
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let responses = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    let first = reward(&responses[0], "closed-v1-lv40").claimed_at_ms;
    assert!(responses
        .iter()
        .all(|status| reward(status, "closed-v1-lv40").claimed_at_ms == first));
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"]
            .activities
            .as_ref()
            .unwrap()
            .claims
            .len(),
        1
    );
}

#[test]
fn activities_file_commit_preserves_claim_across_reload_without_save_mutation() {
    let file = path("durable");
    let config = fixture(50);
    config
        .account_store
        .lock()
        .unwrap()
        .save_to_path(&file)
        .unwrap();
    let config = config.with_account_store_path(file.clone());
    let index = config.default_character.index;
    config
        .activity_claim("demo", index, "closed-v1-lv50", &closed(), NOW)
        .unwrap();
    let loaded = FileAccountStoreRepository::new(&file)
        .load(config.default_character.clone())
        .unwrap();
    let account = &loaded.accounts["demo"];
    assert_eq!(loaded.schema_version, 9);
    assert_eq!(
        account.activities.as_ref().unwrap().claims["closed-v1-lv50"].claimed_at_ms,
        NOW
    );
    assert_eq!(account.saves[&index].revision, 0);
    assert!(account.saves[&index].inventory_items_json.is_empty());
    account.activities.as_ref().unwrap().validate().unwrap();
}

#[test]
fn activities_schema8_without_new_authority_fields_migrates_without_data_loss() {
    let config = fixture(40);
    let mut legacy = snapshot(&config);
    legacy["schemaVersion"] = serde_json::json!(8);
    let file = path("legacy8");
    std::fs::write(&file, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let loaded = FileAccountStoreRepository::new(&file)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(loaded.schema_version, 9);
    assert_eq!(
        loaded.accounts["demo"].saves[&config.default_character.index]
            .character
            .level,
        40
    );
    assert!(loaded.accounts["demo"].activities.is_none());
    assert_eq!(
        serde_json::to_value(&loaded.accounts["demo"]).unwrap(),
        legacy["accounts"]["demo"]
    );
}

#[test]
fn activities_authority_fields_cannot_be_promoted_from_an_older_schema_tag() {
    let config = fixture(40);
    config
        .activity_claim(
            "demo",
            config.default_character.index,
            "closed-v1-lv40",
            &closed(),
            NOW,
        )
        .unwrap();
    let mut forged = snapshot(&config);
    forged["schemaVersion"] = serde_json::json!(8);
    let file = path("forged8");
    std::fs::write(&file, serde_json::to_vec(&forged).unwrap()).unwrap();
    assert_eq!(
        FileAccountStoreRepository::new(&file)
            .load(config.default_character.clone())
            .unwrap_err(),
        "activitiesSchemaInvalid"
    );
}

#[test]
fn activities_qualification_and_claims_survive_character_save_deletion() {
    let config = fixture(50);
    let index = config.default_character.index;
    config
        .activity_claim("demo", index, "closed-v1-lv40", &closed(), NOW)
        .unwrap();
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("demo").unwrap();
        account.characters.clear();
        account.saves.clear();
    }
    let status = config.activity_status("demo", &closed(), NOW + 1).unwrap();
    assert_eq!(status.max_level, 0);
    assert!(status.characters.is_empty());
    assert!(reward(&status, "closed-v1-lv40").claimed);
    assert_eq!(reward(&status, "closed-v1-lv50").qualified_level, 50);
    assert_eq!(
        config
            .activity_claim("demo", index, "closed-v1-lv50", &closed(), NOW + 1)
            .unwrap_err(),
        "activityCharacterMissing"
    );
}

#[test]
fn activities_later_phase_levels_cannot_qualify_an_earlier_campaign() {
    let config = fixture(29);
    config.activity_status("demo", &closed(), NOW).unwrap();
    set_level(&config, 50);
    let beta_policy = policy(ActivityPhase::Beta, NOW + 10);
    let status = config
        .activity_status("demo", &beta_policy, NOW + 11)
        .unwrap();
    assert_eq!(reward(&status, "closed-v1-lv30").qualified_level, 29);
    assert!(!reward(&status, "closed-v1-lv30").eligible);
    assert!(reward(&status, "beta-v1-lv40").eligible);
    assert!(reward(&status, "beta-v1-lv50").eligible);
    assert_eq!(
        config
            .activity_claim(
                "demo",
                config.default_character.index,
                "closed-v1-lv30",
                &beta_policy,
                NOW + 12
            )
            .unwrap_err(),
        "activityLevelRequired"
    );
    let before = snapshot(&config);
    assert_eq!(
        config
            .activity_status("demo", &closed(), NOW + 13)
            .unwrap_err(),
        "activityPhaseSealed"
    );
    assert_eq!(snapshot(&config), before);
}

#[test]
fn activities_campaign_key_cannot_be_reused_with_new_start_or_window() {
    let config = fixture(40);
    config.activity_status("demo", &closed(), NOW).unwrap();
    let before = snapshot(&config);
    assert_eq!(
        config
            .activity_status("demo", &policy(ActivityPhase::Closed, NOW - 50), NOW)
            .unwrap_err(),
        "activityPhaseSealed"
    );
    let mut changed_end = closed();
    changed_end.phase_end_ms = Some(NOW + 50);
    assert_eq!(
        config
            .activity_status("demo", &changed_end, NOW)
            .unwrap_err(),
        "activityPhaseSealed"
    );
    assert_eq!(snapshot(&config), before);
}

#[test]
fn activities_public_progress_preserves_past_rewards_without_new_qualification() {
    let config = fixture(40);
    config
        .activity_claim(
            "demo",
            config.default_character.index,
            "closed-v1-lv40",
            &closed(),
            NOW,
        )
        .unwrap();
    set_level(&config, 50);
    let public = policy(ActivityPhase::Public, NOW + 10);
    let status = config.activity_status("demo", &public, NOW + 11).unwrap();
    assert!(reward(&status, "closed-v1-lv40").claimed);
    assert!(reward(&status, "closed-v1-lv30").eligible);
    assert!(!reward(&status, "closed-v1-lv50").eligible);
    assert_eq!(reward(&status, "closed-v1-lv50").qualified_level, 40);
}

#[test]
fn activities_existing_claim_remains_valid_when_same_phase_progress_increases() {
    let config = fixture(40);
    config
        .activity_claim(
            "demo",
            config.default_character.index,
            "closed-v1-lv40",
            &closed(),
            NOW,
        )
        .unwrap();
    set_level(&config, 50);
    let status = config.activity_status("demo", &closed(), NOW + 1).unwrap();
    assert!(reward(&status, "closed-v1-lv40").claimed);
    assert!(reward(&status, "closed-v1-lv50").eligible);
    config.account_store.lock().unwrap().accounts["demo"]
        .activities
        .as_ref()
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn activities_expired_phase_does_not_observe_new_levels_or_accept_new_claims() {
    let config = fixture(29);
    let mut ended = closed();
    ended.phase_end_ms = Some(NOW + 10);
    config.activity_status("demo", &ended, NOW).unwrap();
    set_level(&config, 50);
    let status = config.activity_status("demo", &ended, NOW + 10).unwrap();
    assert_eq!(reward(&status, "closed-v1-lv30").qualified_level, 29);
    assert!(!reward(&status, "closed-v1-lv30").eligible);
    assert_eq!(
        config
            .activity_claim(
                "demo",
                config.default_character.index,
                "closed-v1-lv30",
                &ended,
                NOW + 10
            )
            .unwrap_err(),
        "activityDisabled"
    );
}

#[test]
fn activities_test_paid_checkout_never_counts_as_real_beta_carryover() {
    let config = fixture(1);
    paid(&config, "sandbox", false, NOW, NOW + 1);
    let before_credit = config
        .recharge_credit_status("demo", config.default_character.index)
        .unwrap();
    let status = config.activity_status("demo", &beta(), NOW + 2).unwrap();
    assert_eq!(status.carryover.real_verified_usd_minor, 0);
    assert_eq!(status.carryover.verified_credits, 0);
    assert_eq!(status.carryover.monthly_days, 0);
    assert_eq!(status.carryover.test_orders_excluded, 1);
    assert!(status.carryover.records.is_empty());
    assert_eq!(
        config
            .recharge_credit_status("demo", config.default_character.index)
            .unwrap(),
        before_credit
    );
}

#[test]
fn activities_real_payment_is_recorded_without_level_gate_or_new_wallet_award() {
    let config = fixture(1);
    let order = paid(&config, "real", true, NOW, NOW + 1);
    let index = config.default_character.index;
    let credit_before = config.recharge_credit_status("demo", index).unwrap();
    let status = config.activity_status("demo", &beta(), NOW + 2).unwrap();
    assert_eq!(status.max_level, 1);
    assert_eq!(
        (
            status.carryover.real_verified_usd_minor,
            status.carryover.verified_credits
        ),
        (1000, 1000)
    );
    assert_eq!(status.carryover.monthly_days, 0);
    assert_eq!(status.carryover.settlement, ActivitySettlement::Recorded);
    assert_eq!(status.carryover.records.len(), 1);
    assert_eq!(status.carryover.records[0].order_id, order.id);
    let status = config.activity_status("demo", &beta(), NOW + 3).unwrap();
    assert_eq!(status.carryover.records_total, 1);
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        credit_before
    );
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains("paymentIntentId") && !json.contains("checkoutSessionId"));
}

#[test]
fn activities_applied_funds_require_reconciliation_and_never_duplicate_monthly_time() {
    let mut config = fixture(1);
    paid(&config, "applied", true, NOW, NOW + 1);
    let index = config.default_character.index;
    config
        .consume_inactive_pending_recharge_credits("demo", index, NOW + 2)
        .unwrap();
    let status = config.activity_status("demo", &beta(), NOW + 3).unwrap();
    assert_eq!(status.carryover.verified_credits, 1000);
    assert_eq!(
        status.carryover.settlement,
        ActivitySettlement::ReconciliationRequired
    );
    config.billing_monthly_card_credit_price = Some(1000);
    config
        .buy_monthly_card_for_character("demo", index, "monthly-test", NOW + 4)
        .unwrap();
    let before = snapshot(&config);
    let status = config.activity_status("demo", &beta(), NOW + 5).unwrap();
    assert_eq!(status.carryover.verified_credits, 1000);
    assert_eq!(status.carryover.monthly_days, 0);
    assert_eq!(
        status.carryover.settlement,
        ActivitySettlement::ReconciliationRequired
    );
    assert_eq!(snapshot(&config), before);
    assert_eq!(
        config.recharge_credit_status("demo", index).unwrap(),
        (0, 0)
    );
}

#[test]
fn activities_refund_or_dispute_excludes_payment_and_keeps_review_evidence() {
    let config = fixture(1);
    let order = paid(&config, "review", true, NOW, NOW + 1);
    config.activity_status("demo", &beta(), NOW + 2).unwrap();
    config
        .mark_recharge_review(
            "demo",
            &order.id,
            "refund",
            "evt_activities_refund",
            NOW + 3,
        )
        .unwrap();
    let status = config.activity_status("demo", &beta(), NOW + 4).unwrap();
    assert_eq!(
        (
            status.carryover.real_verified_usd_minor,
            status.carryover.verified_credits
        ),
        (0, 0)
    );
    assert_eq!(status.carryover.review_orders, 1);
    assert_eq!(status.carryover.records[0].state, "review");
    assert_eq!(
        status.carryover.settlement,
        ActivitySettlement::ReconciliationRequired
    );
}

#[test]
fn activities_missing_provider_ledger_keeps_historical_proof_but_requires_reconciliation() {
    let config = fixture(1);
    paid(&config, "provider-ledger-wipe", true, NOW, NOW + 1);
    config.activity_status("demo", &beta(), NOW + 2).unwrap();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .billing = None;
    let status = config.activity_status("demo", &beta(), NOW + 3).unwrap();
    assert_eq!(status.carryover.verified_credits, 1000);
    assert_eq!(status.carryover.records_total, 1);
    assert_eq!(
        status.carryover.settlement,
        ActivitySettlement::ReconciliationRequired
    );
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"].saves
            [&config.default_character.index]
            .credit,
        0
    );
}

#[test]
fn activities_beta_payment_window_excludes_old_orders_and_after_launch_payments() {
    let config = fixture(1);
    paid(&config, "old-created", true, NOW - 200, NOW + 1);
    paid(&config, "within-beta", true, NOW, NOW + 1);
    paid(&config, "future-paid", true, NOW + 21, NOW + 22);
    let mut window = beta();
    window.phase_end_ms = Some(NOW + 20);
    let status = config.activity_status("demo", &window, NOW + 5).unwrap();
    assert_eq!(status.carryover.records_total, 1);
    assert_eq!(status.carryover.verified_credits, 1000);
    let public = policy(ActivityPhase::Public, NOW + 20);
    let status = config.activity_status("demo", &public, NOW + 23).unwrap();
    assert_eq!(status.carryover.records_total, 1);
    assert_eq!(status.carryover.verified_credits, 1000);
}

#[test]
fn activities_ended_beta_imports_eligible_historical_payment_after_refresh() {
    let config = fixture(1);
    let mut window = beta();
    window.phase_end_ms = Some(NOW + 20);
    config.activity_status("demo", &window, NOW).unwrap();
    paid(&config, "late-visible-historical", true, NOW + 10, NOW + 11);
    let status = config.activity_status("demo", &window, NOW + 30).unwrap();
    assert_eq!(status.carryover.records_total, 1);
    assert_eq!(status.carryover.verified_credits, 1000);
    assert!(!reward(&status, "beta-v1-lv40").eligible);
    let repeat = config
        .activity_status("demo", &ActivityPolicy::default(), NOW + 31)
        .unwrap();
    assert_eq!(repeat.carryover.records_total, 1);
    assert_eq!(repeat.carryover.verified_credits, 1000);
}

#[test]
fn activities_disabled_phase_imports_only_sealed_beta_payments_without_new_level_progress() {
    let config = fixture(1);
    let mut window = beta();
    window.phase_end_ms = Some(NOW + 20);
    config.activity_status("demo", &window, NOW).unwrap();
    paid(&config, "disabled-historical", true, NOW + 10, NOW + 11);
    paid(&config, "disabled-after-window", true, NOW + 21, NOW + 22);
    set_level(&config, 50);
    let status = config
        .activity_status("demo", &ActivityPolicy::default(), NOW + 30)
        .unwrap();
    assert_eq!(status.carryover.records_total, 1);
    assert_eq!(status.carryover.verified_credits, 1000);
    assert_eq!(reward(&status, "beta-v1-lv50").qualified_level, 1);
    assert!(!reward(&status, "beta-v1-lv50").eligible);
    assert_eq!(
        config
            .recharge_credit_status("demo", config.default_character.index)
            .unwrap(),
        (0, 2000)
    );
}

#[test]
fn activities_unrelated_account_and_private_save_are_not_modified_by_claim() {
    let config = fixture(50);
    {
        let mut store = config.account_store.lock().unwrap();
        let mut other = AccountRecord::new(config.default_character.clone());
        other.characters[0].name = "Other".into();
        other
            .saves
            .get_mut(&config.default_character.index)
            .unwrap()
            .character
            .name = "Other".into();
        store.accounts.insert("other".into(), other);
    }
    let before =
        serde_json::to_value(&config.account_store.lock().unwrap().accounts["other"]).unwrap();
    config
        .activity_claim(
            "demo",
            config.default_character.index,
            "closed-v1-lv40",
            &closed(),
            NOW,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&config.account_store.lock().unwrap().accounts["other"]).unwrap(),
        before
    );
}

#[test]
fn activities_banned_account_cannot_register_campaign_entitlement() {
    let config = fixture(50);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .is_banned = true;
    let before = snapshot(&config);
    assert_eq!(
        config
            .activity_claim(
                "demo",
                config.default_character.index,
                "closed-v1-lv30",
                &closed(),
                NOW
            )
            .unwrap_err(),
        "activityAccountBanned"
    );
    assert_eq!(
        config.activity_status("demo", &closed(), NOW).unwrap_err(),
        "activityAccountBanned"
    );
    assert_eq!(snapshot(&config), before);
}

#[test]
fn activities_carryover_public_rows_are_bounded_but_full_ledger_remains_durable() {
    let config = fixture(1);
    for ordinal in 0..51 {
        paid(&config, &format!("bounded-{ordinal}"), true, NOW, NOW + 1);
    }
    let status = config.activity_status("demo", &beta(), NOW + 2).unwrap();
    assert_eq!(status.carryover.records_total, 51);
    assert_eq!(status.carryover.records.len(), 50);
    assert_eq!(status.carryover.verified_credits, 51_000);
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"]
            .activities
            .as_ref()
            .unwrap()
            .recharge_records
            .len(),
        51
    );
    assert!(serde_json::to_vec(&status).unwrap().len() < 32_768);
}

#[cfg(feature = "test-support")]
#[test]
fn activities_known_failed_publication_has_no_claim_or_qualification_receipt() {
    let config = fixture(50);
    let before = snapshot(&config);
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
    );
    assert!(config
        .activity_claim(
            "demo",
            config.default_character.index,
            "closed-v1-lv50",
            &closed(),
            NOW
        )
        .unwrap_err()
        .contains("before persist"));
    assert_eq!(snapshot(&config), before);
    assert!(
        reward(
            &config
                .activity_claim(
                    "demo",
                    config.default_character.index,
                    "closed-v1-lv50",
                    &closed(),
                    NOW
                )
                .unwrap(),
            "closed-v1-lv50"
        )
        .claimed
    );
}

#[cfg(feature = "test-support")]
#[test]
fn activities_uncertain_file_publication_freezes_and_reload_finds_one_reservation() {
    let file = path("uncertain");
    let config = fixture(50);
    config
        .account_store
        .lock()
        .unwrap()
        .save_to_path(&file)
        .unwrap();
    let config = config.with_account_store_path(file.clone());
    let index = config.default_character.index;
    let qualified = config.activity_status("demo", &closed(), NOW - 1).unwrap();
    assert!(reward(&qualified, "closed-v1-lv30").eligible);
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    assert!(config
        .activity_claim("demo", index, "closed-v1-lv30", &closed(), NOW)
        .unwrap_err()
        .contains("OUTCOME_UNKNOWN"));
    assert!(config
        .activity_claim("demo", index, "closed-v1-lv30", &closed(), NOW + 1)
        .is_err());
    assert!(
        config.activity_status("demo", &closed(), NOW + 1).is_err(),
        "frozen authority must not publish obsolete fast-path qualification"
    );
    let loaded = FileAccountStoreRepository::new(&file)
        .load(config.default_character.clone())
        .unwrap();
    assert_eq!(
        loaded.accounts["demo"]
            .activities
            .as_ref()
            .unwrap()
            .claims
            .len(),
        1
    );
    assert_eq!(
        loaded.accounts["demo"].activities.as_ref().unwrap().claims["closed-v1-lv30"].claimed_at_ms,
        NOW
    );
    assert_eq!(loaded.accounts["demo"].saves[&index].revision, 0);
}

//! Dedicated native PostgreSQL fixtures. Provider evidence is synthetic and
//! explicitly trusted by the fixture; no live provider, wallet or chain is used.
//! These tests require the isolated local database and never silently skip.
use std::collections::BTreeMap;

use mir2_simulation::billing::{RechargeOffer, VerifiedRechargePayment};
use mir2_simulation::billing_provenance::VerifiedPaymentRevenue;
use mir2_simulation::promotion::{
    calculate_commission, CommissionInput, MemberInput, PromotionRules, PromotionStore,
    RevenueInput, TrustedGameFact, ALGORITHM_VERSION, FULL_MONTHLY_ALGORITHM_VERSION,
    TARGET_CAMPAIGN_ID,
};
use mir2_simulation::{apply_migrations, AccountRecord, SimulationConfig};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};

const AT: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;
const ACTIVATED: u64 = AT + 2 * DAY;
const NOW: u64 = ACTIVATED + 1000;
const END: u64 = AT + 7 * DAY;
const TERMS: &str = "full-monthly-native-fixture-v1";
const EVIDENCE: &str = "owner-authorized-isolated-fixture:full-monthly-policy";
const LEGACY_CAMPAIGN: &str = "locked-legacy-policy-fixture";

type Snapshot = BTreeMap<String, Value>;

struct Fixture {
    db: Client,
    store: PromotionStore,
    scoped: String,
}

impl Fixture {
    fn new(label: &str) -> Self {
        assert_eq!(
            std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref(),
            Ok("1"),
            "full-monthly policy tests require explicit isolated native PG opt-in"
        );
        let url = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL")
            .expect("dedicated local promotion test database URL is required");
        let parsed: Config = url.parse().expect("valid dedicated database URL");
        assert!(parsed.get_dbname().unwrap_or("").contains("promotion"));
        assert!(parsed.get_options().is_none());
        assert!(parsed.get_hosts().iter().all(|host| {
            matches!(host, postgres::config::Host::Tcp(name) if name == "127.0.0.1" || name == "localhost")
        }));
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!(
            "promotion_full_monthly_{label}_{}_{nonce}",
            std::process::id()
        );
        let mut base = Client::connect(&url, NoTls).unwrap();
        base.batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let separator = if url.contains('?') { '&' } else { '?' };
        let scoped = format!("{url}{separator}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000");
        let mut db = Client::connect(&scoped, NoTls).unwrap();
        apply_migrations(&mut db).unwrap();
        apply_migrations(&mut db).unwrap();
        let config = SimulationConfig::default()
            .with_postgres_account_store(scoped.clone())
            .unwrap()
            .with_billing_monthly_card_credit_price(Some(1000))
            .unwrap();
        for (account, name) in [
            ("operator", "Operator"),
            ("finance", "Finance"),
            ("leader", "Leader"),
            ("member", "Member"),
            ("outsider", "Outsider"),
        ] {
            let mut character = config.default_character.clone();
            character.name = name.into();
            character.level = 30;
            config
                .account_store
                .lock()
                .unwrap()
                .accounts
                .insert(account.into(), AccountRecord::new(character));
            config.save_account_store_account(account).unwrap();
            db.execute("UPDATE accounts SET created_at=to_timestamp($2::bigint::double precision/1000.0) WHERE account_id=$1", &[&account,&((AT+100) as i64)]).unwrap();
        }
        for (actor, role) in [
            ("operator", "operations"),
            ("operator", "finance"),
            ("finance", "finance"),
        ] {
            db.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'isolated-native-fixture',1)", &[&actor,&role]).unwrap();
        }
        let mut rules = PromotionRules::default();
        rules.active_utc_days = 1;
        rules.reserve_bps = 0;
        rules.maximum_coverage_ms = 30 * DAY;
        let rules = serde_json::to_value(rules).unwrap();
        db.execute("INSERT INTO promotion_campaigns(environment,campaign_id,terms_version,status,starts_at_ms,ends_at_ms,rules_json,rules_approved,recruitment_enabled,commission_enabled,payout_enabled,budget_micro_usd,reserved_micro_usd,member_limit,leader_limit,per_leader_member_limit,per_member_revenue_cap_micro_usd,configured_by,approved_by,created_at_ms,funding_mode) VALUES('test',$1,$2,'active',$3,$4,$5,TRUE,TRUE,TRUE,FALSE,0,0,0,0,0,0,'operator','finance',$3,'revenue_share')", &[&TARGET_CAMPAIGN_ID,&TERMS,&(AT as i64),&(END as i64),&rules]).unwrap();
        db.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES('test',$1,'leader','fixture-leader','fixture-invite',0,'Leader','approved',$2,$3)", &[&TARGET_CAMPAIGN_ID,&TERMS,&((AT+200) as i64)]).unwrap();
        db.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES('test',$1,'fixture-candidate','member','fixture-leader',$2,$3,'manual_code')", &[&TARGET_CAMPAIGN_ID,&((AT+300) as i64),&((AT+DAY) as i64)]).unwrap();
        db.execute("INSERT INTO promotion_attributions(environment,campaign_id,account_id,leader_id,candidate_id,first_clicked_at_ms,registered_at_ms,confirmed_at_ms,terms_version,source,reserved_micro_usd) VALUES('test',$1,'member','fixture-leader','fixture-candidate',$2,$3,$4,$5,'manual_code',0)", &[&TARGET_CAMPAIGN_ID,&((AT+300) as i64),&((AT+100) as i64),&((AT+500) as i64),&TERMS]).unwrap();

        fund_and_activate(&config);
        let store = PromotionStore::new(scoped.clone(), "test").unwrap();
        for (source, kind, id, at, level) in [
            (
                "level_commit",
                "levelCommitted",
                "fixture-level",
                AT + 1000,
                Some(30),
            ),
            (
                "combat_commit",
                "combatKill",
                "fixture-combat",
                AT + 2000,
                None,
            ),
        ] {
            store
                .record_game_fact(
                    &TrustedGameFact {
                        source: source.into(),
                        kind: kind.into(),
                        source_event_id: id.into(),
                        account_id: "member".into(),
                        character_index: 0,
                        occurred_at_ms: at,
                        level,
                        evidence_ref: "explicit-authoritative-game-fact-fixture".into(),
                    },
                    NOW,
                )
                .unwrap();
        }
        let imported = store.reconcile_live_monthly_revenue(NOW).unwrap();
        assert_eq!(imported["allocationsImported"], 1);
        assert_eq!(store.process_pending_work(100, NOW).unwrap()["failed"], 0);
        assert_eq!(
            store.me(TARGET_CAMPAIGN_ID, "leader", NOW).unwrap()["dashboard"]
                ["estimatedCommissionMicroUsd"],
            "166666"
        );
        let allocation = db.query_one("SELECT eligible_micro_usd,service_start_ms,service_end_ms,funding_status FROM promotion_revenue_allocations WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap();
        assert_eq!(allocation.get::<_, i64>(0), 10_000_000);
        assert_eq!(allocation.get::<_, i64>(1), ACTIVATED as i64);
        assert_eq!(allocation.get::<_, i64>(2), (ACTIVATED + 30 * DAY) as i64);
        assert_eq!(allocation.get::<_, String>(3), "verified_paid");
        Self { db, store, scoped }
    }

    fn snapshot(&mut self) -> Snapshot {
        let tables = self.db.query("SELECT table_name FROM information_schema.tables WHERE table_schema=current_schema() AND table_type='BASE TABLE' ORDER BY table_name", &[]).unwrap();
        let mut snapshot = BTreeMap::new();
        for table in tables {
            let table: String = table.get(0);
            assert!(table
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
            let value: Value = self.db.query_one(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]'::jsonb) FROM {table} t"), &[]).unwrap().get(0);
            snapshot.insert(table, value);
        }
        snapshot
    }

    fn failure(
        &mut self,
        label: &str,
        expected: &str,
        operation: impl FnOnce(&PromotionStore) -> Result<Value, String>,
    ) {
        let before = self.snapshot();
        assert_eq!(operation(&self.store).unwrap_err(), expected, "{label}");
        assert_eq!(
            self.snapshot(),
            before,
            "{label} must make zero durable writes"
        );
    }

    fn preview(&self) -> Value {
        self.store
            .preview_full_monthly_policy(TARGET_CAMPAIGN_ID, NOW)
            .unwrap()
    }

    fn apply(&self, expected: &str) -> Result<Value, String> {
        self.store.adopt_full_monthly_policy(
            TARGET_CAMPAIGN_ID,
            "operator",
            "finance",
            expected,
            EVIDENCE,
            NOW,
        )
    }
}

fn fund_and_activate(config: &SimulationConfig) {
    fund_and_activate_at(config, "full-monthly-fixture", ACTIVATED);
}

fn fund_and_activate_at(config: &SimulationConfig, key: &str, activated: u64) -> u64 {
    let offer = RechargeOffer {
        id: "monthly-funded-ten-usd".into(),
        label: "Native PG synthetic provider evidence".into(),
        currency: "usd".into(),
        amount_minor: 1000,
        credits: 1000,
    };
    let order = config
        .prepare_recharge_order("member", 0, key, &offer, activated - 5, false)
        .unwrap();
    let payment = VerifiedRechargePayment {
        session_id: format!("cs_{key}"),
        payment_intent_id: format!("pi_{key}"),
        amount_minor: 1000,
        currency: "usd".into(),
        livemode: false,
    };
    config
        .confirm_recharge_payment("member", &order.id, &payment, activated - 4)
        .unwrap();
    config
        .record_verified_payment_revenue(
            "member",
            &VerifiedPaymentRevenue {
                provider: "stripe".into(),
                source_event_id: format!("evt_{key}"),
                order_id: order.id,
                checkout_session_id: payment.session_id,
                payment_intent_id: payment.payment_intent_id,
                livemode: false,
                currency: "usd".into(),
                gross_amount_minor: 1000,
                indirect_tax_minor: Some(0),
                discount_minor: Some(0),
                paid_at_ms: activated - 4,
                observed_at_ms: activated - 3,
                evidence_ref: "explicit-provider-fixture:not-a-provider-acceptance".into(),
            },
        )
        .unwrap();
    config
        .consume_inactive_pending_recharge_credits("member", 0, activated - 2)
        .unwrap();
    let card = config
        .buy_monthly_card_for_character("member", 0, &format!("monthly-{key}"), activated - 1)
        .unwrap();
    config
        .activate_monthly_card_for_character("member", 0, card.unique_id, activated)
        .unwrap();
    card.unique_id
}

fn first_difference(before: &Value, after: &Value, path: &str) -> Option<String> {
    if before == after {
        return None;
    }
    match (before, after) {
        (Value::Object(left), Value::Object(right)) => {
            for key in left.keys().chain(right.keys()) {
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(diff) = first_difference(left, right, &format!("{path}.{key}"))
                        {
                            return Some(diff);
                        }
                    }
                    _ => return Some(format!("{path}.{key}: field added or removed")),
                }
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!(
                    "{path}: row count {} -> {}",
                    left.len(),
                    right.len()
                ));
            }
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                if let Some(diff) = first_difference(left, right, &format!("{path}[{index}]")) {
                    return Some(diff);
                }
            }
        }
        _ => return Some(format!("{path}: {before} -> {after}")),
    }
    Some(format!("{path}: values differ"))
}

fn assert_business_preserved(before: Snapshot, after: Snapshot) {
    let before = preserved_business(before);
    let after = preserved_business(after);
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    for (table, old) in before {
        if let Some(diff) = first_difference(&old, &after[&table], &table) {
            panic!("business preservation failed at {diff}");
        }
    }
}

fn preserved_business(mut snapshot: Snapshot) -> Snapshot {
    snapshot.remove("promotion_audit_log");
    snapshot.remove("promotion_commission_runs");
    for campaign in snapshot
        .get_mut("promotion_campaigns")
        .unwrap()
        .as_array_mut()
        .unwrap()
    {
        if campaign["campaign_id"] == TARGET_CAMPAIGN_ID {
            campaign.as_object_mut().unwrap().remove("rules_json");
        }
    }
    for attribution in snapshot
        .get_mut("promotion_attributions")
        .unwrap()
        .as_array_mut()
        .unwrap()
    {
        if attribution["campaign_id"] == TARGET_CAMPAIGN_ID {
            attribution
                .as_object_mut()
                .unwrap()
                .remove("qualification_algorithm_version");
        }
    }
    snapshot
}

fn assert_append_only(before: &Snapshot, after: &Snapshot, table: &str, added: usize) {
    let old = before[table].as_array().unwrap();
    let new = after[table].as_array().unwrap();
    assert_eq!(new.len(), old.len() + added, "{table} exact appended rows");
    for row in old {
        assert!(
            new.contains(row),
            "existing {table} row must be byte-equivalent JSON"
        );
    }
}

#[test]
fn native_full_monthly_preview_adoption_and_replay_preserve_paid_game_state() {
    let mut fixture = Fixture::new("adopt");
    insert_frozen_legacy_settlement(&mut fixture.db);
    let original = fixture.snapshot();
    let preview = fixture.preview();
    assert_eq!(fixture.snapshot(), original, "preview must be read-only");
    assert_eq!(preview["currentRecognition"], "service_period_prorated");
    assert_eq!(preview["payoutEnabled"], false);
    assert_eq!(preview["leaders"].as_array().unwrap().len(), 1);
    assert_eq!(preview["leaders"][0]["leaderId"], "fixture-leader");
    assert_eq!(
        preview["leaders"][0]["before"]["algorithmVersion"],
        ALGORITHM_VERSION
    );
    assert_eq!(
        preview["leaders"][0]["before"]["eligibleRevenueMicroUsd"],
        1_666_666
    );
    assert_eq!(
        preview["leaders"][0]["before"]["commissionMicroUsd"],
        166_666
    );
    assert_eq!(
        preview["leaders"][0]["after"]["algorithmVersion"],
        FULL_MONTHLY_ALGORITHM_VERSION
    );
    assert_eq!(
        preview["leaders"][0]["after"]["eligibleRevenueMicroUsd"],
        10_000_000
    );
    assert_eq!(
        preview["leaders"][0]["after"]["commissionMicroUsd"],
        1_000_000
    );
    let hash = preview["rulesSha256"].as_str().unwrap().to_owned();
    assert_eq!(hash.len(), 64);

    fixture.failure(
        "wrong campaign",
        "promotionFullMonthlyPolicyScopeInvalid",
        |store| {
            store.adopt_full_monthly_policy(
                LEGACY_CAMPAIGN,
                "operator",
                "finance",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    for environment in ["production", "development"] {
        let wrong_environment = PromotionStore::new(fixture.scoped.clone(), environment).unwrap();
        fixture.failure(
            "wrong environment",
            "promotionFullMonthlyPolicyScopeInvalid",
            |_| {
                wrong_environment.adopt_full_monthly_policy(
                    TARGET_CAMPAIGN_ID,
                    "operator",
                    "finance",
                    &hash,
                    EVIDENCE,
                    NOW,
                )
            },
        );
    }
    fixture.failure(
        "stale rule hash",
        "promotionFullMonthlyPolicyChanged",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &"a".repeat(64),
                EVIDENCE,
                NOW,
            )
        },
    );
    for evidence in ["", "   "] {
        fixture.failure(
            "missing evidence",
            "promotionFullMonthlyPolicyEvidenceInvalid",
            |store| {
                store.adopt_full_monthly_policy(
                    TARGET_CAMPAIGN_ID,
                    "operator",
                    "finance",
                    &hash,
                    evidence,
                    NOW,
                )
            },
        );
    }
    fixture.failure(
        "oversized evidence",
        "promotionFullMonthlyPolicyEvidenceInvalid",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &hash,
                &"e".repeat(513),
                NOW,
            )
        },
    );
    fixture.failure(
        "malformed expected hash",
        "promotionFullMonthlyPolicyEvidenceInvalid",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                "bad-hash",
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.failure(
        "operations missing role",
        "promotionPermissionDenied",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "outsider",
                "finance",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.failure(
        "finance missing role",
        "promotionPermissionDenied",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "outsider",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.failure(
        "distinct actors required even with both roles",
        "promotionDistinctReviewerRequired",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "operator",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    for time in [AT - 1, END] {
        fixture.failure(
            "outside active season",
            "promotionFullMonthlyPolicyStateInvalid",
            |store| {
                store.adopt_full_monthly_policy(
                    TARGET_CAMPAIGN_ID,
                    "operator",
                    "finance",
                    &hash,
                    EVIDENCE,
                    time,
                )
            },
        );
    }
    fixture.db.execute("UPDATE promotion_operator_roles SET revoked_at_ms=$1 WHERE environment='test' AND account_id='finance' AND role='finance'", &[&(NOW as i64)]).unwrap();
    fixture.failure(
        "revoked finance role",
        "promotionPermissionDenied",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.db.execute("UPDATE promotion_operator_roles SET revoked_at_ms=NULL WHERE environment='test' AND account_id='finance' AND role='finance'", &[]).unwrap();
    fixture.db.execute("UPDATE promotion_campaigns SET rules_approved=FALSE,commission_enabled=FALSE,recruitment_enabled=FALSE WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap();
    fixture.failure(
        "unapproved rules",
        "promotionFullMonthlyPolicyStateInvalid",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.db.execute("UPDATE promotion_campaigns SET rules_approved=TRUE,commission_enabled=TRUE,recruitment_enabled=TRUE WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap();
    fixture.db.execute("UPDATE promotion_campaigns SET status='closed',recruitment_enabled=FALSE WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap();
    fixture.failure(
        "closed campaign",
        "promotionFullMonthlyPolicyStateInvalid",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.db.execute("UPDATE promotion_campaigns SET status='active',recruitment_enabled=TRUE WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap();

    // Existing reviewed launch evidence only enables a guard fixture; this test
    // creates no payout, intent, signed transaction or external operation.
    fixture.db.execute("INSERT INTO promotion_production_approvals(environment,campaign_id,configuration_sha256,manifest_sha256,manifest_json,proposed_by,proposed_at_ms,approved_by,approved_at_ms) VALUES('test',$1,$2,$2,'{}','operator',1,'finance',2)", &[&TARGET_CAMPAIGN_ID,&"b".repeat(64)]).unwrap();
    fixture.db.execute("UPDATE promotion_campaigns SET payout_enabled=TRUE WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap();
    fixture.failure(
        "payment switch must stay off",
        "promotionFullMonthlyPolicyStateInvalid",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.db.execute("UPDATE promotion_campaigns SET payout_enabled=FALSE WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap();

    let before = fixture.snapshot();
    let applied = fixture.apply(&hash).unwrap();
    assert_eq!(applied["status"], "applied");
    assert_eq!(applied["newRecognition"], "monthly_purchase_full");
    assert_eq!(applied["payoutEnabled"], false);
    assert_eq!(
        applied["leaders"][0]["after"]["commissionMicroUsd"],
        1_000_000
    );
    let after = fixture.snapshot();
    assert_business_preserved(before.clone(), after.clone());
    assert_append_only(&before, &after, "promotion_commission_runs", 1);
    assert_append_only(&before, &after, "promotion_audit_log", 2);
    let new_rules: Value = fixture.db.query_one("SELECT rules_json FROM promotion_campaigns WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap().get(0);
    let mut old_without_policy = new_rules.clone();
    assert_eq!(
        old_without_policy
            .as_object_mut()
            .unwrap()
            .remove("revenueRecognition"),
        Some(json!("monthly_purchase_full"))
    );
    assert_eq!(
        old_without_policy,
        before["promotion_campaigns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["campaign_id"] == TARGET_CAMPAIGN_ID)
            .unwrap()["rules_json"]
    );
    let audit = fixture.db.query_one("SELECT actor_id,evidence_ref,summary_json FROM promotion_audit_log WHERE action='rules.full_monthly_adopted' AND environment='test'", &[]).unwrap();
    assert_eq!(audit.get::<_, String>(0), "operator");
    assert_eq!(audit.get::<_, String>(1), EVIDENCE);
    let summary: Value = audit.get(2);
    assert_eq!(summary["operationsActor"], "operator");
    assert_eq!(summary["financeActor"], "finance");
    assert_eq!(summary["beforeRules"], old_without_policy);
    assert_eq!(summary["afterRules"], new_rules);
    let live = fixture.store.me(TARGET_CAMPAIGN_ID, "leader", NOW).unwrap();
    assert_eq!(live["dashboard"]["estimatedCommissionMicroUsd"], "1000000");
    assert_eq!(live["payoutExecutionEnabled"], false);
    assert!(live["settlements"].as_array().unwrap().is_empty());
    assert_eq!(fixture.db.query_one("SELECT qualification_algorithm_version FROM promotion_attributions WHERE environment='test' AND campaign_id=$1 AND account_id='member'", &[&TARGET_CAMPAIGN_ID]).unwrap().get::<_, String>(0), FULL_MONTHLY_ALGORITHM_VERSION);
    let replay_before = fixture.snapshot();
    assert_eq!(fixture.apply(&hash).unwrap()["status"], "already_applied");
    assert_eq!(
        fixture.snapshot(),
        replay_before,
        "exact replay adds no audit/run or other write"
    );
    fixture.failure(
        "changed evidence cannot replay old hash",
        "promotionFullMonthlyPolicyChanged",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &hash,
                "different-review-evidence",
                NOW,
            )
        },
    );
    fixture.db.execute("UPDATE promotion_operator_roles SET revoked_at_ms=$1 WHERE environment='test' AND account_id='finance' AND role='finance'", &[&(NOW as i64)]).unwrap();
    fixture.failure(
        "replay still requires current roles",
        "promotionPermissionDenied",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.db.execute("UPDATE promotion_operator_roles SET revoked_at_ms=NULL WHERE environment='test' AND account_id='finance' AND role='finance'", &[]).unwrap();
    let full_preview_before = fixture.snapshot();
    let full_preview = fixture.preview();
    assert_eq!(full_preview["currentRecognition"], "monthly_purchase_full");
    assert_eq!(full_preview["rulesSha256"], applied["newRulesSha256"]);
    assert_eq!(
        full_preview["leaders"][0]["before"]["commissionMicroUsd"],
        1_000_000
    );
    assert_eq!(
        full_preview["leaders"][0]["after"]["commissionMicroUsd"],
        1_000_000
    );
    assert_eq!(fixture.snapshot(), full_preview_before);
    let legacy = fixture.store.me(LEGACY_CAMPAIGN, "leader", NOW).unwrap();
    assert_eq!(legacy["dashboard"]["algorithmVersion"], ALGORITHM_VERSION);
    assert_eq!(legacy["dashboard"]["estimatedCommissionMicroUsd"], "166666");
    assert_eq!(legacy["settlements"][0]["totalMicroUsd"], "166666");
}

#[test]
fn native_full_monthly_any_existing_settlement_blocks_adoption_without_writes() {
    let mut fixture = Fixture::new("settled");
    let hash = fixture.preview()["rulesSha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let result = legacy_result();
    let snapshot = json!({"result":result});
    fixture.db.execute("INSERT INTO promotion_settlements(environment,campaign_id,settlement_id,leader_id,status,algorithm_version,snapshot_json,snapshot_sha256,total_micro_usd,payable_cents,reserve_micro_usd,created_by,created_at_ms) VALUES('test',$1,'fixture-locked-target','fixture-leader','review',$2,$3,$4,166666,17,0,'finance',$5)", &[&TARGET_CAMPAIGN_ID,&ALGORITHM_VERSION,&snapshot,&"c".repeat(64),&(NOW as i64)]).unwrap();
    fixture.failure(
        "existing review settlement",
        "promotionSeasonLocked",
        |store| {
            store.adopt_full_monthly_policy(
                TARGET_CAMPAIGN_ID,
                "operator",
                "finance",
                &hash,
                EVIDENCE,
                NOW,
            )
        },
    );
    fixture.failure(
        "preview of settled target",
        "promotionSeasonLocked",
        |store| store.preview_full_monthly_policy(TARGET_CAMPAIGN_ID, NOW),
    );
}

#[test]
fn native_full_monthly_activated_renewal_is_imported_without_claiming_later_seasons() {
    let mut fixture = Fixture::new("renewal");
    let config = SimulationConfig::default()
        .with_postgres_account_store(fixture.scoped.clone())
        .unwrap()
        .with_billing_monthly_card_credit_price(Some(1000))
        .unwrap();
    let renewed_at = NOW + 20;
    let renewal = fund_and_activate_at(&config, "season-renewal", renewed_at);

    // Legacy recognition does not import a renewal whose future service period
    // lies entirely beyond this short season.
    assert_eq!(
        fixture
            .store
            .reconcile_live_monthly_revenue(renewed_at + 1)
            .unwrap()["allocationsImported"],
        0
    );
    let hash = fixture.preview()["rulesSha256"]
        .as_str()
        .unwrap()
        .to_owned();
    fixture.apply(&hash).unwrap();
    // A persisted redemption observed before its own event time is still held.
    assert_eq!(
        fixture
            .store
            .reconcile_live_monthly_revenue(renewed_at - 1)
            .unwrap()["allocationsImported"],
        0
    );
    assert_eq!(
        fixture
            .store
            .reconcile_live_monthly_revenue(renewed_at + 1)
            .unwrap()["allocationsImported"],
        1
    );
    assert_eq!(
        fixture
            .store
            .process_pending_work(100, renewed_at + 1)
            .unwrap()["failed"],
        0
    );
    let renewed = fixture.db.query_one("SELECT service_start_ms,service_end_ms,eligible_micro_usd,entitlement_at_ms FROM promotion_revenue_allocations WHERE environment='test' AND subscription_id=$1", &[&format!("monthly-item:{renewal}")]).unwrap();
    assert_eq!(renewed.get::<_, i64>(0), (ACTIVATED + 30 * DAY) as i64);
    assert!(renewed.get::<_, i64>(0) >= END as i64);
    assert_eq!(renewed.get::<_, i64>(1), (ACTIVATED + 60 * DAY) as i64);
    assert_eq!(renewed.get::<_, i64>(2), 10_000_000);
    assert_eq!(renewed.get::<_, i64>(3), renewed_at as i64);
    assert_eq!(
        fixture
            .store
            .me(TARGET_CAMPAIGN_ID, "leader", renewed_at + 1)
            .unwrap()["dashboard"]["estimatedCommissionMicroUsd"],
        "2000000"
    );

    // A later season uses the same explicit full-purchase rule. The earlier
    // season is scanned first, so omitting its payment/activation date gate
    // would consume the globally unique monthly UID and steal this purchase.
    const NEXT: &str = "z-next-full-monthly-fixture";
    let mut next_rules = PromotionRules::default();
    next_rules.active_utc_days = 1;
    next_rules.reserve_bps = 0;
    next_rules.revenue_recognition =
        mir2_simulation::promotion::RevenueRecognition::MonthlyPurchaseFull;
    let next_rules = serde_json::to_value(next_rules).unwrap();
    fixture.db.execute("INSERT INTO promotion_campaigns(environment,campaign_id,terms_version,status,starts_at_ms,ends_at_ms,rules_json,rules_approved,recruitment_enabled,commission_enabled,configured_by,approved_by,created_at_ms,funding_mode) VALUES('test',$1,$2,'active',$3,$4,$5,TRUE,TRUE,TRUE,'operator','finance',$3,'revenue_share')", &[&NEXT,&TERMS,&(END as i64),&((END+7*DAY) as i64),&next_rules]).unwrap();
    fixture.db.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES('test',$1,'leader','next-leader','next-invite',0,'Leader','approved',$2,$3)", &[&NEXT,&TERMS,&((END+1) as i64)]).unwrap();
    fixture.db.execute("INSERT INTO promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id,first_clicked_at_ms,expires_at_ms,source) VALUES('test',$1,'next-candidate','member','next-leader',$2,$3,'manual_code')", &[&NEXT,&((END+2) as i64),&((END+DAY) as i64)]).unwrap();
    fixture.db.execute("INSERT INTO promotion_attributions(environment,campaign_id,account_id,leader_id,candidate_id,first_clicked_at_ms,registered_at_ms,confirmed_at_ms,terms_version,source,reserved_micro_usd) VALUES('test',$1,'member','next-leader','next-candidate',$2,$3,$4,$5,'manual_code',0)", &[&NEXT,&((END+2) as i64),&((AT+100) as i64),&((END+10) as i64),&TERMS]).unwrap();
    let later_at = END + 100;
    let later = fund_and_activate_at(&config, "next-season-renewal", later_at);
    for (source, kind, event, time, level) in [
        (
            "level_commit",
            "levelCommitted",
            "next-level",
            END + 20,
            Some(30),
        ),
        ("combat_commit", "combatKill", "next-combat", END + 30, None),
    ] {
        fixture
            .store
            .record_game_fact(
                &TrustedGameFact {
                    source: source.into(),
                    kind: kind.into(),
                    source_event_id: event.into(),
                    account_id: "member".into(),
                    character_index: 0,
                    occurred_at_ms: time,
                    level,
                    evidence_ref: "explicit-next-season-source-fixture".into(),
                },
                later_at + 1,
            )
            .unwrap();
    }
    assert_eq!(
        fixture
            .store
            .reconcile_live_monthly_revenue(later_at + 1)
            .unwrap()["allocationsImported"],
        1
    );
    assert_eq!(fixture.db.query_one("SELECT campaign_id FROM promotion_revenue_allocations WHERE environment='test' AND subscription_id=$1", &[&format!("monthly-item:{later}")]).unwrap().get::<_, String>(0), NEXT);
    assert_eq!(
        fixture
            .db
            .query_one(
                "SELECT count(*) FROM promotion_revenue_allocations WHERE environment='test'",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        3
    );
    assert_eq!(fixture.db.query_one("SELECT count(*) FROM promotion_revenue_allocations WHERE environment='test' AND campaign_id=$1", &[&TARGET_CAMPAIGN_ID]).unwrap().get::<_, i64>(0), 2);
    assert_eq!(
        fixture
            .store
            .process_pending_work(100, later_at + 1)
            .unwrap()["failed"],
        0
    );
    assert_eq!(
        fixture
            .store
            .me(TARGET_CAMPAIGN_ID, "leader", later_at + 1)
            .unwrap()["dashboard"]["estimatedCommissionMicroUsd"],
        "2000000"
    );
    assert_eq!(
        fixture.store.me(NEXT, "leader", later_at + 1).unwrap()["dashboard"]
            ["estimatedCommissionMicroUsd"],
        "1000000"
    );
    assert_eq!(
        fixture
            .store
            .reconcile_live_monthly_revenue(later_at + 2)
            .unwrap()["allocationsImported"],
        0
    );
    assert_eq!(
        fixture
            .db
            .query_one(
                "SELECT count(*) FROM promotion_revenue_allocations WHERE environment='test'",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        3
    );
}

fn legacy_result() -> mir2_simulation::promotion::CommissionResult {
    calculate_commission(&CommissionInput {
        environment: "test".into(),
        campaign_id: LEGACY_CAMPAIGN.into(),
        terms_version: TERMS.into(),
        starts_at_ms: AT,
        ends_at_ms: END,
        maximum_coverage_ms: 30 * DAY,
        per_member_revenue_cap_micro_usd: u64::MAX,
        reserve_bps: 0,
        members: vec![MemberInput {
            account_id: "member".into(),
            level_completed_at_ms: Some(AT + 1000),
            play_completed_at_ms: Some(AT + 2000),
            established_qualification_at_ms: None,
            excluded: false,
            revenue: vec![RevenueInput {
                allocation_id: "frozen-legacy-monthly".into(),
                paid_at_ms: ACTIVATED - 4,
                entitlement_at_ms: ACTIVATED,
                service_start_ms: ACTIVATED,
                service_end_ms: ACTIVATED + 30 * DAY,
                amount_micro_usd: 10_000_000,
                reversed_micro_usd: 0,
                is_card_payment: true,
            }],
        }],
    })
    .unwrap()
}

fn insert_frozen_legacy_settlement(db: &mut Client) {
    let rules: Value = serde_json::to_value(PromotionRules::default()).unwrap();
    db.execute("INSERT INTO promotion_campaigns(environment,campaign_id,terms_version,status,starts_at_ms,ends_at_ms,rules_json,rules_approved,commission_enabled,configured_by,approved_by,created_at_ms,funding_mode) VALUES('test',$1,$2,'locked',$3,$4,$5,TRUE,TRUE,'operator','finance',$3,'revenue_share')", &[&LEGACY_CAMPAIGN,&TERMS,&(AT as i64),&(END as i64),&rules]).unwrap();
    db.execute("INSERT INTO promotion_leaders(environment,campaign_id,account_id,leader_id,invite_code,display_character_index,display_name,status,accepted_terms_version,applied_at_ms) VALUES('test',$1,'leader','legacy-leader','legacy-invite',0,'Leader','approved',$2,$3)", &[&LEGACY_CAMPAIGN,&TERMS,&((AT+200) as i64)]).unwrap();
    let snapshot = json!({"result":legacy_result()});
    db.execute("INSERT INTO promotion_settlements(environment,campaign_id,settlement_id,leader_id,status,algorithm_version,snapshot_json,snapshot_sha256,total_micro_usd,payable_cents,reserve_micro_usd,created_by,reviewed_by,created_at_ms,reviewed_at_ms) VALUES('test',$1,'legacy-frozen-settlement','legacy-leader','approved',$2,$3,$4,166666,17,0,'operator','finance',$5,$5)", &[&LEGACY_CAMPAIGN,&ALGORITHM_VERSION,&snapshot,&"d".repeat(64),&(NOW as i64)]).unwrap();
}

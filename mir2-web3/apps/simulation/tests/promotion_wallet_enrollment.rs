//! Isolated native PostgreSQL fixtures. No external wallet, payment or game
//! evidence is synthesized by the test-only enrollment path under test.
use mir2_simulation::promotion::{CampaignDraft, PromotionRules, PromotionStore};
use mir2_simulation::{apply_migrations, AccountRecord, SimulationConfig};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};

const AT: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;
const TERMS: &str = "test-wallet-enrollment-v1";

#[test]
fn legacy_rules_keep_wallet_enrollment_disabled_and_approval_covers_the_flag() {
    let rules = PromotionRules::default();
    assert!(!rules.test_wallet_enrollment);
    let mut old = serde_json::to_value(&rules).unwrap();
    old.as_object_mut().unwrap().remove("testWalletEnrollment");
    let decoded: PromotionRules = serde_json::from_value(old.clone()).unwrap();
    assert!(!decoded.test_wallet_enrollment);
    assert_eq!(
        serde_json::to_value(&decoded).unwrap(),
        old,
        "false must retain legacy canonical JSON"
    );
    let mut enabled = decoded.clone();
    enabled.test_wallet_enrollment = true;
    assert_ne!(
        serde_json::to_value(decoded).unwrap(),
        serde_json::to_value(enabled).unwrap()
    );
}

struct Fixture {
    base: String,
    schema: String,
    scoped: String,
    db: Client,
}
impl Fixture {
    fn new() -> Option<Self> {
        if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
            eprintln!("SKIPPED native PG: explicit isolated opt-in required");
            return None;
        }
        let base = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL")
            .expect("private isolated URI required");
        let parsed: Config = base.parse().unwrap();
        assert!(parsed.get_dbname().unwrap_or("").contains("promotion"));
        assert!(parsed.get_options().is_none());
        assert!(parsed.get_hosts().iter().all(
            |h| matches!(h, postgres::config::Host::Tcp(n) if n=="127.0.0.1"||n=="localhost")
        ));
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("promotion_wallet_{}_{nonce}", std::process::id());
        let mut root = Client::connect(&base, NoTls).unwrap();
        root.batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let scoped = format!("{base}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",if base.contains('?') {"&"} else {"?"});
        let mut db = Client::connect(&scoped, NoTls).unwrap();
        apply_migrations(&mut db).unwrap();
        Some(Self {
            base,
            schema,
            scoped,
            db,
        })
    }
    fn store(&self, env: &str) -> PromotionStore {
        PromotionStore::new(self.scoped.clone(), env).unwrap()
    }
    fn account(&mut self, id: &str, registered: u64, level: Option<u16>) {
        let record = match level {
            None => AccountRecord::empty(),
            Some(level) => {
                let mut character = SimulationConfig::default().default_character;
                character.name = id.to_owned();
                character.index = 0;
                character.level = level;
                AccountRecord::new(character)
            }
        };
        let raw = serde_json::to_value(&record).unwrap();
        self.db.execute("INSERT INTO accounts(account_id,raw_json,created_at) VALUES($1,$2,to_timestamp($3::bigint::double precision/1000.0))",&[&id,&raw,&(registered as i64)]).unwrap();
        if let Some(level) = level {
            self.db.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,0,$1,'Warrior','Male',$2,$3)",&[&id,&i32::from(level),&raw["characters"][0]]).unwrap();
        }
    }
    fn credential(&mut self, id: &str, kind: &str, n: u64, revoked: bool) {
        self.db.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms,revoked_at_ms) VALUES($1,$2,$3,$4,1,$5)",&[&format!("credential-{id}"),&id,&kind,&format!("0x{n:064x}"),&revoked.then_some(2i64)]).unwrap();
    }
    fn grant(&mut self, env: &str, id: &str, role: &str) {
        self.db.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES($1,$2,$3,'explicit-isolated-fixture',1)",&[&env,&id,&role]).unwrap();
    }
    fn command(
        &self,
        store: &PromotionStore,
        campaign: &str,
        actor: &str,
        value: Value,
        key: &str,
        now: u64,
    ) -> Value {
        store
            .admin(campaign, actor, value, key, now)
            .unwrap_or_else(|e| panic!("fixture {campaign}/{key}: {e}"))
    }
    fn campaign(&self, id: &str, flag: bool, open: bool, leader_limit: u64) {
        let store = self.store("test");
        let draft = draft(id, flag, leader_limit);
        self.command(&store,id,"operator",json!({"action":"configureDraft","draft":draft,"evidenceRef":"isolated-test-wallet-fixture"}),&format!("draft-{id}"),AT+1);
        self.command(
            &store,
            id,
            "finance",
            json!({"action":"approveRules","evidenceRef":"isolated-independent-review"}),
            &format!("approve-{id}"),
            AT + 2,
        );
        if open {
            self.command(
                &store,
                id,
                "operator",
                json!({"action":"setRecruitment","enabled":true,"evidenceRef":"test-only"}),
                &format!("open-{id}"),
                AT + 3,
            );
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this test's generated schema, never an existing account database.
        if let Ok(mut db) = Client::connect(&self.base, NoTls) {
            let _ = db.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema));
        }
    }
}
fn draft(id: &str, flag: bool, leader_limit: u64) -> CampaignDraft {
    let mut rules = PromotionRules::default();
    rules.test_wallet_enrollment = flag;
    rules.minimum_level = 1;
    rules.active_utc_days = 1;
    rules.maximum_coverage_ms = 7 * DAY;
    rules.reserve_bps = 0;
    CampaignDraft {
        campaign_id: id.into(),
        funding_mode: "revenue_share".into(),
        terms_version: TERMS.into(),
        starts_at_ms: AT,
        ends_at_ms: AT + 7 * DAY,
        rules,
        budget_micro_usd: 0,
        member_limit: 0,
        leader_limit,
        per_leader_member_limit: 0,
        per_member_revenue_cap_micro_usd: 0,
    }
}
fn apply(
    store: &PromotionStore,
    campaign: &str,
    account: &str,
    index: i32,
    key: &str,
    at: u64,
) -> Result<Value, String> {
    store.apply(campaign, account, index, TERMS, key, at)
}

#[test]
fn native_wallet_enrollment_is_explicit_test_only_and_does_not_create_game_or_money_facts() {
    let Some(mut fixture) = Fixture::new() else {
        return;
    };
    for id in [
        "operator", "finance", "wallet", "second", "member", "revoked", "passkey", "missing",
        "invalid",
    ] {
        fixture.account(id, if id == "member" { AT + 1700 } else { AT + 1000 }, None);
    }
    fixture.account("ordinary", AT + 1000, Some(1));
    fixture.account("low", AT + 1000, Some(0));
    for env in ["test", "development", "production"] {
        fixture.grant(env, "operator", "operations");
        fixture.grant(env, "finance", "finance");
    }
    for (id, kind, n, revoked) in [
        ("wallet", "sui_wallet", 1, false),
        ("second", "sui_wallet", 2, false),
        ("revoked", "sui_wallet", 3, true),
        ("passkey", "sui_passkey", 4, false),
        ("invalid", "sui_wallet", 5, false),
    ] {
        fixture.credential(id, kind, n, revoked);
    }
    fixture.db.execute("UPDATE identity_credentials SET credential_subject='browser-supplied-name' WHERE account_id='invalid'",&[]).unwrap();
    let store = fixture.store("test");
    // A flag cannot be configured outside test, even by an authorized operator.
    for env in ["development", "production"] {
        assert_eq!(fixture.store(env).admin("forbidden","operator",json!({"action":"configureDraft","draft":draft("forbidden",true,0),"evidenceRef":"fixture"}),"reject-nontest",AT+1).unwrap_err(),"promotionEnvironmentMismatch");
    }
    fixture.campaign("disabled", false, true, 0);
    assert_eq!(
        apply(
            &store,
            "disabled",
            "wallet",
            -1,
            "disabled-apply",
            AT + 1400
        )
        .unwrap_err(),
        "promotionTestWalletEnrollmentUnavailable"
    );
    fixture.campaign("closed", true, false, 0);
    assert_eq!(
        apply(&store, "closed", "wallet", -1, "closed-apply", AT + 1400).unwrap_err(),
        "promotionRecruitmentClosed"
    );
    fixture.campaign("playtest", true, true, 0);
    for (id, error) in [
        ("revoked", "promotionWalletProofRequired"),
        ("passkey", "promotionWalletProofRequired"),
        ("missing", "promotionWalletProofRequired"),
        ("invalid", "promotionWalletProofRequired"),
    ] {
        assert_eq!(
            apply(
                &store,
                "playtest",
                id,
                -1,
                &format!("reject-{id}"),
                AT + 1400
            )
            .unwrap_err(),
            error
        );
    }
    assert_eq!(
        apply(
            &store,
            "playtest",
            "wallet",
            -2,
            "reject-other-negative",
            AT + 1400
        )
        .unwrap_err(),
        "promotionTestWalletEnrollmentUnavailable"
    );
    assert_eq!(
        apply(&store, "playtest", "wallet", -1, "reject-future", AT - 1).unwrap_err(),
        "promotionRecruitmentClosed"
    );
    assert_eq!(
        apply(
            &store,
            "playtest",
            "wallet",
            -1,
            "reject-expired",
            AT + 7 * DAY
        )
        .unwrap_err(),
        "promotionRecruitmentClosed"
    );
    let enrolled = apply(&store, "playtest", "wallet", -1, "wallet-enroll", AT + 1500).unwrap();
    assert_eq!(enrolled["status"], "approved");
    assert_eq!(enrolled["testWalletEnrollment"], true);
    assert_eq!(
        apply(&store, "playtest", "wallet", -1, "wallet-enroll", AT + 1501).unwrap(),
        enrolled
    );
    assert_eq!(
        apply(
            &store,
            "playtest",
            "wallet",
            -1,
            "wallet-new-key",
            AT + 1502
        )
        .unwrap()["replayed"],
        true
    );
    let mine = store.me("playtest", "wallet", AT + 1550).unwrap();
    assert_eq!(mine["leader"]["characterIndex"], -1);
    assert_eq!(mine["leader"]["displayName"], "Wallet 0x000000…0001");
    assert!(mine["characters"].as_array().unwrap().is_empty());
    let code = mine["leader"]["inviteCode"].as_str().unwrap();
    assert!(!code.is_empty());
    // A later credential revocation never enrolls again under a new request.
    fixture
        .db
        .execute(
            "UPDATE identity_credentials SET revoked_at_ms=$1 WHERE account_id='wallet'",
            &[&((AT + 1551) as i64)],
        )
        .unwrap();
    assert_eq!(
        apply(
            &store,
            "playtest",
            "wallet",
            -1,
            "revoked-after-enroll",
            AT + 1552
        )
        .unwrap_err(),
        "promotionWalletProofRequired"
    );
    let click = store
        .record_invitation_click("playtest", code, AT + 1600)
        .unwrap();
    let candidate = store
        .candidate_from_click(
            "playtest",
            "member",
            click["clickId"].as_str().unwrap(),
            "member-candidate",
            AT + 1800,
        )
        .unwrap();
    let bound = store
        .confirm(
            "playtest",
            "member",
            candidate["candidateId"].as_str().unwrap(),
            TERMS,
            "member-confirm",
            AT + 1900,
        )
        .unwrap();
    assert_eq!(bound["status"], "bound");
    assert_eq!(bound["reservedMicroUsd"], "0");
    assert_eq!(
        store.process_pending_work(100, AT + 2000).unwrap()["failed"],
        0
    );
    let member = store.me("playtest", "member", AT + 2001).unwrap();
    assert_eq!(member["attribution"]["status"], "bound");
    assert!(member["attribution"]["qualifiedAtMs"].is_null());
    let mine = store.me("playtest", "wallet", AT + 2001).unwrap();
    assert_eq!(mine["dashboard"]["confirmed"], 1);
    assert_eq!(mine["dashboard"]["qualified"], 0);
    assert_eq!(mine["dashboard"]["estimatedCommissionMicroUsd"], "0");
    let raw: Value = fixture
        .db
        .query_one(
            "SELECT raw_json FROM accounts WHERE account_id='wallet'",
            &[],
        )
        .unwrap()
        .get(0);
    let account: AccountRecord = serde_json::from_value(raw).unwrap();
    assert!(
        account.characters.is_empty()
            && account.saves.is_empty()
            && account.billing.is_none()
            && account.monthly_card.is_none()
    );
    for table in [
        "characters",
        "promotion_game_facts",
        "promotion_game_source_events",
        "promotion_revenue_allocations",
        "promotion_credit_lot_allocations",
    ] {
        assert_eq!(
            fixture
                .db
                .query_one(
                    &format!(
                        "SELECT count(*) FROM {table} WHERE {}",
                        if table == "characters" {
                            "account_id IN ('wallet','member')"
                        } else {
                            "TRUE"
                        }
                    ),
                    &[]
                )
                .unwrap()
                .get::<_, i64>(0),
            0,
            "no invented facts in {table}"
        );
    }
    let audit:Value=fixture.db.query_one("SELECT summary_json FROM promotion_audit_log WHERE action='leader.test_wallet_enrolled'",&[]).unwrap().get(0);
    assert_eq!(audit["testOnly"], true);
    assert_eq!(audit["gameQualificationGranted"], false);
    assert_eq!(audit["paymentEvidenceCreated"], false);
    // Ordinary owned-role enrollment is unchanged, including its review step.
    assert_eq!(
        apply(&store, "playtest", "low", 0, "ordinary-low", AT + 2100).unwrap_err(),
        "promotionLevelRequired"
    );
    assert_eq!(
        apply(
            &store,
            "playtest",
            "ordinary",
            0,
            "ordinary-apply",
            AT + 2100
        )
        .unwrap()["status"],
        "applied"
    );
    assert!(store.me("playtest", "ordinary", AT + 2101).unwrap()["leader"]["inviteCode"].is_null());
    fixture.campaign("capacity", true, true, 1);
    apply(
        &store,
        "capacity",
        "second",
        -1,
        "capacity-first",
        AT + 2200,
    )
    .unwrap();
    fixture
        .db
        .execute(
            "UPDATE identity_credentials SET revoked_at_ms=NULL WHERE account_id='wallet'",
            &[],
        )
        .unwrap();
    assert_eq!(
        apply(
            &store,
            "capacity",
            "wallet",
            -1,
            "capacity-reject",
            AT + 2201
        )
        .unwrap_err(),
        "promotionLeaderLimit"
    );
    // Defense in depth: an invalid trusted DB row must not bypass apply's env check.
    fixture.db.execute("INSERT INTO promotion_campaigns(environment,campaign_id,terms_version,status,starts_at_ms,ends_at_ms,rules_json,rules_approved,configured_by,created_at_ms,funding_mode) SELECT 'production','injected',terms_version,'approved',starts_at_ms,ends_at_ms,rules_json,TRUE,configured_by,created_at_ms,funding_mode FROM promotion_campaigns WHERE environment='test' AND campaign_id='playtest'",&[]).unwrap();
    assert_eq!(
        apply(
            &fixture.store("production"),
            "injected",
            "wallet",
            -1,
            "injected-negative",
            AT + 2300
        )
        .unwrap_err(),
        "promotionEnvironmentMismatch"
    );
    assert_eq!(
        apply(
            &fixture.store("production"),
            "injected",
            "ordinary",
            0,
            "injected-ordinary",
            AT + 2300
        )
        .unwrap_err(),
        "promotionEnvironmentMismatch"
    );
    fixture.command(
        &store,
        "playtest",
        "operator",
        json!({"action":"setRecruitment","enabled":false,"evidenceRef":"end-fixture"}),
        "close-playtest",
        AT + 2400,
    );
    assert_eq!(
        apply(&store, "playtest", "second", -1, "reject-paused", AT + 2401).unwrap_err(),
        "promotionRecruitmentClosed"
    );
    println!("native PG wallet playtest: explicit test gate, real credential ownership/revocation, instant invitation, new member bound, zero invented game/payment facts, ordinary role review and leader capacity passed");
}

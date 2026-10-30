//! Explicitly isolated native PostgreSQL migration fixtures. These exercise the
//! real store, billing authority and SQL constraints; no chain RPC, wallet
//! signature, deployed realm or external payment provider is contacted.
use mir2_simulation::billing::RechargeOffer;
use mir2_simulation::promotion::{
    CampaignDraft, PromotionMigrationScope, PromotionRules, PromotionStore, SOURCE_CAMPAIGN_ID,
    TARGET_CAMPAIGN_ID,
};
use mir2_simulation::sui_usdc_billing::{
    UsdcDomain, UsdcRechargeOrder, VerifiedNativeUsdcTransferReceipt, TESTNET_GENESIS_DIGEST,
    TESTNET_USDC_COIN_TYPE,
};
use mir2_simulation::{apply_migrations, AccountRecord, SimulationConfig};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const AT: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;
const SOURCE_TERMS: &str = "isolated-migration-source-terms";
const TARGET_TERMS: &str = "isolated-migration-current-terms";
const OLD_LEADER: &str = "migration_old_leader";
const OLD_MEMBER: &str = "migration_old_member";
const LEADER: &str = "migration_windows_leader";
const MEMBER: &str = "migration_windows_member";
const OPERATIONS: &str = "migration_fixture_operations";
const FINANCE: &str = "migration_fixture_finance";

fn address(marker: char) -> String {
    format!("0x{}", marker.to_string().repeat(64))
}
fn scope() -> PromotionMigrationScope {
    PromotionMigrationScope {
        member_wallet: address('a'),
        leader_wallet: address('b'),
    }
}
fn digest(value: &Value) -> String {
    Sha256::digest(serde_json::to_vec(value).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct Realm {
    base: String,
    schema: String,
    url: String,
    db: Client,
}
impl Realm {
    fn new(base: &str, role: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("promotion_migration_{role}_{}_{nonce}", std::process::id());
        let mut root = Client::connect(base, NoTls).unwrap();
        root.batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let url = format!("{base}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000", if base.contains('?') {"&"} else {"?"});
        let mut db = Client::connect(&url, NoTls).unwrap();
        apply_migrations(&mut db).unwrap();
        apply_migrations(&mut db).unwrap();
        assert_eq!(
            db.query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        Self {
            base: base.into(),
            schema,
            url,
            db,
        }
    }
    fn store(&self) -> PromotionStore {
        PromotionStore::new(self.url.clone(), "test").unwrap()
    }
    fn account(&mut self, id: &str, registered: u64, wallet: Option<&str>) {
        let mut character = SimulationConfig::default().default_character;
        character.name = id.into();
        character.index = 0;
        character.level = 1;
        let raw = serde_json::to_value(AccountRecord::new(character)).unwrap();
        self.db.execute("INSERT INTO accounts(account_id,raw_json,created_at) VALUES($1,$2,to_timestamp($3::bigint::double precision/1000.0))", &[&id,&raw,&(registered as i64)]).unwrap();
        self.db.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,0,$1,'Warrior','Male',1,$2)", &[&id,&raw["characters"][0]]).unwrap();
        if let Some(wallet) = wallet {
            self.db.execute("INSERT INTO identity_credentials(credential_id,account_id,credential_kind,credential_subject,created_at_ms) VALUES($1,$2,'sui_wallet',$3,$4)", &[&format!("credential-{id}"),&id,&wallet,&(registered as i64)]).unwrap();
        }
    }
    fn operators(&mut self) {
        for (actor, role) in [(OPERATIONS, "operations"), (FINANCE, "finance")] {
            self.account(actor, AT + 10, None);
            self.db.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,$2,'isolated-migration-fixture',$3)", &[&actor,&role,&((AT+20) as i64)]).unwrap();
        }
    }
    fn campaign(&self, id: &str, terms: &str) {
        let mut rules = PromotionRules::default();
        rules.test_wallet_enrollment = true;
        rules.minimum_level = 1;
        rules.active_utc_days = 1;
        rules.maximum_coverage_ms = 7 * DAY;
        rules.reserve_bps = 0;
        let draft = CampaignDraft {
            campaign_id: id.into(),
            terms_version: terms.into(),
            starts_at_ms: AT,
            ends_at_ms: AT + 7 * DAY,
            rules,
            funding_mode: "revenue_share".into(),
            budget_micro_usd: 0,
            member_limit: 0,
            leader_limit: 0,
            per_leader_member_limit: 0,
            per_member_revenue_cap_micro_usd: 0,
        };
        let store = self.store();
        for (actor, action, at, key) in [
            (
                OPERATIONS,
                json!({"action":"configureDraft","draft":draft,"evidenceRef":"isolated-fixture-no-production"}),
                AT + 30,
                "draft",
            ),
            (
                FINANCE,
                json!({"action":"approveRules","evidenceRef":"isolated-independent-review"}),
                AT + 40,
                "approve",
            ),
            (
                OPERATIONS,
                json!({"action":"setRecruitment","enabled":true,"evidenceRef":"isolated-test-only"}),
                AT + 50,
                "recruit",
            ),
            (
                FINANCE,
                json!({"action":"setCommission","enabled":true,"evidenceRef":"isolated-test-only"}),
                AT + 60,
                "commission",
            ),
        ] {
            store
                .admin(id, actor, action, &format!("{id}-{key}"), at)
                .unwrap();
        }
    }
    fn accounts_snapshot(&mut self) -> String {
        let rows: Vec<Value> = self
            .db
            .query(
                "SELECT to_jsonb(a) FROM accounts a ORDER BY account_id",
                &[],
            )
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect();
        digest(&json!(rows))
    }
    fn relation_count(&mut self, table: &str) -> i64 {
        assert!(matches!(
            table,
            "promotion_candidates" | "promotion_attributions" | "promotion_revenue_allocations"
        ));
        self.db
            .query_one(
                &format!(
                    "SELECT count(*) FROM {table} WHERE environment='test' AND campaign_id=$1"
                ),
                &[&TARGET_CAMPAIGN_ID],
            )
            .unwrap()
            .get(0)
    }
}
impl Drop for Realm {
    fn drop(&mut self) {
        // The name is generated above from literals, PID and nanoseconds. Never
        // remove an existing/shared schema or database, even after an assertion.
        assert!(self.schema.starts_with("promotion_migration_"));
        if let Ok(mut root) = Client::connect(&self.base, NoTls) {
            let _ = root.batch_execute(&format!("DROP SCHEMA {} CASCADE", self.schema));
        }
    }
}
struct Pair {
    source: Realm,
    target: Realm,
}
impl Pair {
    fn new(enroll_target_leader: bool) -> Option<Self> {
        if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
            eprintln!("SKIPPED migration PG: explicit isolated local opt-in required");
            return None;
        }
        let base =
            std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL").expect("isolated local URI required");
        let config: Config = base.parse().unwrap();
        assert!(config.get_dbname().unwrap_or("").contains("promotion"));
        assert!(config.get_options().is_none());
        assert!(!config.get_hosts().is_empty());
        assert!(config
            .get_hostaddrs()
            .iter()
            .all(|address| address.is_loopback()));
        assert!(config.get_hosts().iter().all(|host| matches!(host,postgres::config::Host::Tcp(name) if name=="127.0.0.1"||name=="localhost")));
        let mut source = Realm::new(&base, "source");
        let mut target = Realm::new(&base, "target");
        source.operators();
        target.operators();
        source.account(OLD_LEADER, AT + 100, Some(&address('b')));
        source.account(OLD_MEMBER, AT + 200, Some(&address('a')));
        target.account(MEMBER, AT + 2000, Some(&address('a')));
        target.account(LEADER, AT + 3000, Some(&address('b')));
        source.campaign(SOURCE_CAMPAIGN_ID, SOURCE_TERMS);
        target.campaign(TARGET_CAMPAIGN_ID, TARGET_TERMS);
        let old = source.store();
        old.apply(
            SOURCE_CAMPAIGN_ID,
            OLD_LEADER,
            -1,
            SOURCE_TERMS,
            "source-leader-current-terms",
            AT + 120,
        )
        .unwrap();
        let code = old.me(SOURCE_CAMPAIGN_ID, OLD_LEADER, AT + 600).unwrap()["leader"]
            ["inviteCode"]
            .as_str()
            .unwrap()
            .to_owned();
        let click = old
            .record_invitation_click(SOURCE_CAMPAIGN_ID, &code, AT + 150)
            .unwrap();
        let candidate = old
            .candidate_from_click(
                SOURCE_CAMPAIGN_ID,
                OLD_MEMBER,
                click["clickId"].as_str().unwrap(),
                "source-member-click",
                AT + 800,
            )
            .unwrap();
        old.confirm(
            SOURCE_CAMPAIGN_ID,
            OLD_MEMBER,
            candidate["candidateId"].as_str().unwrap(),
            SOURCE_TERMS,
            "source-member-terms",
            AT + 1000,
        )
        .unwrap();
        if enroll_target_leader {
            target
                .store()
                .apply(
                    TARGET_CAMPAIGN_ID,
                    LEADER,
                    -1,
                    TARGET_TERMS,
                    "target-leader-current-terms",
                    AT + 4000,
                )
                .unwrap();
        }
        Some(Self { source, target })
    }
    fn preview(&self, at: u64) -> Result<Value, String> {
        self.target
            .store()
            .preview_legacy_invitation_migration(&self.source.url, &scope(), at)
    }
    fn stage(&self, snapshot: &str, key: &str, at: u64) -> Result<Value, String> {
        self.target.store().stage_legacy_invitation_migration(
            &self.source.url,
            &scope(),
            snapshot,
            OPERATIONS,
            FINANCE,
            "isolated-fixture-reviewed-source-binding",
            key,
            at,
        )
    }
    fn confirm(&self, candidate: &str, key: &str, at: u64) -> Result<Value, String> {
        self.target
            .store()
            .confirm(TARGET_CAMPAIGN_ID, MEMBER, candidate, TARGET_TERMS, key, at)
    }
}

fn fund_native(realm: &Realm, account: &str, at: u64, key: &str) -> UsdcRechargeOrder {
    // Start at the verified-receipt boundary with explicitly synthetic facts.
    // This is the actual config/SQL authority path, not a fake chain RPC.
    let config = SimulationConfig::default()
        .with_postgres_account_store(realm.url.clone())
        .unwrap();
    let offer = RechargeOffer {
        id: "migration-usdc-fixture".into(),
        label: "Isolated migration receipt fixture".into(),
        currency: "usd".into(),
        amount_minor: 100,
        credits: 100,
    };
    let domain = UsdcDomain {
        chain_identifier: "4c78adac".into(),
        package_id: address('1'),
        config_id: address('2'),
        merchant: address('3'),
        coin_type: TESTNET_USDC_COIN_TYPE.into(),
        rule_version: 1,
    };
    let order = config
        .prepare_direct_usdc_recharge_order(
            account,
            0,
            key,
            &offer,
            &domain,
            &format!("credential-{account}"),
            &address('a'),
            &"c".repeat(64),
            at + 120_000,
            at + 60_000,
            at,
        )
        .unwrap();
    let receipt = VerifiedNativeUsdcTransferReceipt {
        chain_identifier: order.domain.chain_identifier.clone(),
        genesis_digest: TESTNET_GENESIS_DIGEST.into(),
        order_id: order.id.clone(),
        account_ref: order.account_ref.clone(),
        payer: address('a'),
        merchant: order.domain.merchant.clone(),
        coin_type: order.domain.coin_type.clone(),
        amount_micro: order.amount_micro,
        frozen_hash: order.frozen_hash().unwrap(),
        digest: format!(
            "{}{}",
            if key.ends_with('2') { '4' } else { '3' },
            "Z".repeat(43)
        ),
        checkpoint: 501,
        confirmed_at_ms: at + 100,
        observed_at_ms: at + 101,
    };
    config
        .confirm_usdc_native_transfer_payment(account, &order.id, &receipt, at + 102)
        .unwrap();
    assert_eq!(
        config
            .consume_inactive_pending_recharge_credits(account, 0, at + 103)
            .unwrap()
            .granted,
        100
    );
    config.usdc_recharge_order(account, &order.id).unwrap()
}
fn migration_count(realm: &mut Realm, table: &str) -> i64 {
    assert!(matches!(
        table,
        "promotion_attribution_migration_evidence"
            | "promotion_attribution_migration_confirmations"
            | "promotion_attribution_migration_payment_claims"
    ));
    realm
        .db
        .query_one(&format!("SELECT count(*) FROM {table}"), &[])
        .unwrap()
        .get(0)
}
fn assert_not_staged(pair: &mut Pair) {
    assert_eq!(pair.target.relation_count("promotion_candidates"), 0);
    assert_eq!(pair.target.relation_count("promotion_attributions"), 0);
    assert_eq!(
        migration_count(&mut pair.target, "promotion_attribution_migration_evidence"),
        0
    );
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_confirmations"
        ),
        0
    );
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_payment_claims"
        ),
        0
    );
}

#[test]
fn native_migration_stages_only_then_records_real_current_terms_confirmation() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    let paid_order = fund_native(&pair.target, MEMBER, AT + 5000, "migration-payment");
    let source_accounts = pair.source.accounts_snapshot();
    let target_accounts = pair.target.accounts_snapshot();
    let original_relation: Value = pair
        .source
        .db
        .query_one(
            "SELECT to_jsonb(a) FROM promotion_attributions a WHERE account_id=$1",
            &[&OLD_MEMBER],
        )
        .unwrap()
        .get(0);
    let preview = pair.preview(AT + 6000).unwrap();
    let snapshot = preview["snapshotSha256"].as_str().unwrap();
    assert_eq!(snapshot.len(), 64);
    let staged = pair
        .stage(snapshot, "stage-reviewed-pair", AT + 6001)
        .unwrap();
    let candidate = staged["candidateId"].as_str().unwrap();
    let migration = staged["migrationId"].as_str().unwrap();
    assert_eq!(pair.target.relation_count("promotion_candidates"), 1);
    assert_eq!(pair.target.relation_count("promotion_attributions"), 0);
    assert_eq!(
        pair.target.relation_count("promotion_revenue_allocations"),
        0
    );
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_confirmations"
        ),
        0
    );
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_payment_claims"
        ),
        0
    );
    let evidence=pair.target.db.query_one("SELECT source_confirmed_at_ms,eligibility_anchor_ms,prepared_at_ms,target_terms_version,source_terms_version FROM promotion_attribution_migration_evidence WHERE migration_id=$1",&[&migration]).unwrap();
    assert_eq!(evidence.get::<_, i64>(0), (AT + 1000) as i64);
    assert_eq!(evidence.get::<_, i64>(1), (AT + 3000) as i64);
    assert_eq!(evidence.get::<_, i64>(2), (AT + 6001) as i64);
    assert_eq!(evidence.get::<_, String>(3), TARGET_TERMS);
    assert_eq!(evidence.get::<_, String>(4), SOURCE_TERMS);
    assert_eq!(
        pair.target
            .db
            .query_one(
                "SELECT source FROM promotion_candidates WHERE candidate_id=$1",
                &[&candidate]
            )
            .unwrap()
            .get::<_, String>(0),
        "migration_import"
    );
    assert_eq!(
        pair.target
            .db
            .query_one(
                "SELECT reserved_micro_usd FROM promotion_campaigns WHERE campaign_id=$1",
                &[&TARGET_CAMPAIGN_ID]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(pair.source.accounts_snapshot(), source_accounts);
    assert_eq!(pair.target.accounts_snapshot(), target_accounts);
    let replay = pair
        .stage(snapshot, "stage-reviewed-pair", AT + 6002)
        .unwrap();
    assert_eq!(replay["candidateId"], staged["candidateId"]);
    assert_eq!(replay["migrationId"], staged["migrationId"]);
    assert!(pair
        .target
        .store()
        .confirm(
            TARGET_CAMPAIGN_ID,
            MEMBER,
            candidate,
            SOURCE_TERMS,
            "wrong-current-terms",
            AT + 6900
        )
        .is_err());
    assert_eq!(pair.target.relation_count("promotion_attributions"), 0);
    pair.confirm(candidate, "member-current-terms-confirm", AT + 7000)
        .unwrap();
    let row=pair.target.db.query_one("SELECT a.confirmed_at_ms,a.terms_version,m.confirmed_at_ms,m.accepted_terms_version,m.source_relation_key FROM promotion_attributions a JOIN promotion_attribution_migration_confirmations m ON m.environment=a.environment AND m.target_campaign_id=a.campaign_id AND m.account_id=a.account_id WHERE a.account_id=$1",&[&MEMBER]).unwrap();
    assert_eq!(row.get::<_, i64>(0), (AT + 7000) as i64);
    assert_eq!(row.get::<_, i64>(2), (AT + 7000) as i64);
    assert_eq!(row.get::<_, String>(1), TARGET_TERMS);
    assert_eq!(row.get::<_, String>(3), TARGET_TERMS);
    assert_eq!(row.get::<_, String>(4).len(), 64);
    let claims = pair
        .target
        .db
        .query(
            "SELECT order_id,account_id FROM promotion_attribution_migration_payment_claims",
            &[],
        )
        .unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].get::<_, String>(0), paid_order.id);
    assert_eq!(claims[0].get::<_, String>(1), MEMBER);
    assert_eq!(pair.source.accounts_snapshot(), source_accounts);
    assert_eq!(pair.target.accounts_snapshot(), target_accounts);
    let original_after: Value = pair
        .source
        .db
        .query_one(
            "SELECT to_jsonb(a) FROM promotion_attributions a WHERE account_id=$1",
            &[&OLD_MEMBER],
        )
        .unwrap()
        .get(0);
    assert_eq!(original_after, original_relation);
    let replay = pair
        .confirm(candidate, "member-current-terms-confirm", AT + 9000)
        .unwrap();
    assert_eq!(replay["status"], "bound");
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_confirmations"
        ),
        1
    );
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_payment_claims"
        ),
        1
    );

    // Actual billing/monthly APIs prove that this pre-import payment is not
    // mistaken for a payment preceding the verified historical invitation.
    let config = SimulationConfig::default()
        .with_postgres_account_store(pair.target.url.clone())
        .unwrap()
        .with_billing_monthly_card_credit_price(Some(100))
        .unwrap();
    let card = config
        .buy_monthly_card_for_character(MEMBER, 0, "migration-monthly", AT + 8000)
        .unwrap();
    config
        .activate_monthly_card_for_character(MEMBER, 0, card.unique_id, AT + 8001)
        .unwrap();
    pair.target
        .store()
        .reconcile_live_monthly_revenue(AT + 8100)
        .unwrap();
    let allocation=pair.target.db.query_one("SELECT funding_status,reason_code,paid_at_ms FROM promotion_revenue_allocations WHERE account_id=$1",&[&MEMBER]).unwrap();
    assert_eq!(allocation.get::<_, String>(0), "verified_paid");
    assert_eq!(allocation.get::<_, String>(1), "verified_paid_credit_fifo");
    assert!(allocation.get::<_, i64>(2) < (AT + 7000) as i64);
    assert_eq!(
        pair.source
            .db
            .query_one("SELECT count(*) FROM promotion_revenue_allocations", &[])
            .unwrap()
            .get::<_, i64>(0),
        0
    );

    for table in [
        "promotion_attribution_migration_evidence",
        "promotion_attribution_migration_confirmations",
        "promotion_attribution_migration_payment_claims",
    ] {
        let before: Value = pair
            .target
            .db
            .query_one(
                &format!("SELECT jsonb_agg(to_jsonb(t)) FROM {table} t"),
                &[],
            )
            .unwrap()
            .get(0);
        for sql in [
            format!("UPDATE {table} SET migration_id=migration_id"),
            format!("DELETE FROM {table}"),
            format!("TRUNCATE {table} CASCADE"),
        ] {
            let error = pair.target.db.batch_execute(&sql).unwrap_err();
            assert!(
                error
                    .as_db_error()
                    .unwrap()
                    .message()
                    .contains("append-only"),
                "actual immutable SQL guard must reject the operation"
            );
        }
        let after: Value = pair
            .target
            .db
            .query_one(
                &format!("SELECT jsonb_agg(to_jsonb(t)) FROM {table} t"),
                &[],
            )
            .unwrap()
            .get(0);
        assert_eq!(before, after);
    }
}

#[test]
fn native_migration_requires_two_actors_exact_proof_and_current_wallet_ownership() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    let preview = pair.preview(AT + 6000).unwrap();
    let snapshot = preview["snapshotSha256"].as_str().unwrap();
    let accounts = pair.target.accounts_snapshot();
    assert!(pair
        .stage(&"0".repeat(64), "tampered-snapshot", AT + 6001)
        .is_err());
    assert_not_staged(&mut pair);
    pair.target.db.execute("INSERT INTO promotion_operator_roles(environment,account_id,role,granted_by,granted_at_ms) VALUES('test',$1,'finance','fixture-role-for-distinct-actor-regression',$2)",&[&OPERATIONS,&((AT+6100) as i64)]).unwrap();
    assert!(pair
        .target
        .store()
        .stage_legacy_invitation_migration(
            &pair.source.url,
            &scope(),
            snapshot,
            OPERATIONS,
            OPERATIONS,
            "isolated-no-self-review",
            "same-reviewer",
            AT + 6200
        )
        .is_err());
    assert_not_staged(&mut pair);
    assert!(pair
        .target
        .store()
        .stage_legacy_invitation_migration(
            &pair.source.url,
            &scope(),
            snapshot,
            OPERATIONS,
            LEADER,
            "isolated-reviewer-must-have-role",
            "wrong-reviewer",
            AT + 6201
        )
        .is_err());
    assert_not_staged(&mut pair);
    pair.target
        .db
        .execute(
            "UPDATE identity_credentials SET revoked_at_ms=$2 WHERE account_id=$1",
            &[&MEMBER, &((AT + 6250) as i64)],
        )
        .unwrap();
    assert!(pair
        .stage(snapshot, "revoked-target-wallet", AT + 6300)
        .is_err());
    assert_not_staged(&mut pair);
    assert_eq!(pair.target.accounts_snapshot(), accounts);
}

#[test]
fn native_migration_cannot_invent_target_leader_terms_or_repair_source_drift() {
    let Some(mut pair) = Pair::new(false) else {
        return;
    };
    assert!(
        pair.preview(AT + 6000).is_err(),
        "legacy approval is not acceptance of current Windows terms"
    );
    assert_not_staged(&mut pair);
    pair.target
        .store()
        .apply(
            TARGET_CAMPAIGN_ID,
            LEADER,
            -1,
            TARGET_TERMS,
            "real-target-leader-acceptance",
            AT + 6100,
        )
        .unwrap();
    let preview = pair.preview(AT + 6200).unwrap();
    let snapshot = preview["snapshotSha256"].as_str().unwrap();
    pair.source.db.execute("UPDATE promotion_attributions SET status='excluded',reason_code='fixture-source-review' WHERE account_id=$1",&[&OLD_MEMBER]).unwrap();
    assert!(pair
        .stage(snapshot, "source-final-relation-drift", AT + 6300)
        .is_err());
    assert_not_staged(&mut pair);
}

#[test]
fn native_migration_does_not_relax_ordinary_paid_account_confirmation() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    fund_native(&pair.target, MEMBER, AT + 5000, "ordinary-payment");
    let store = pair.target.store();
    let code = store.me(TARGET_CAMPAIGN_ID, LEADER, AT + 6000).unwrap()["leader"]["inviteCode"]
        .as_str()
        .unwrap()
        .to_owned();
    let candidate = store
        .create_candidate(
            TARGET_CAMPAIGN_ID,
            &code,
            MEMBER,
            "manual_code",
            "ordinary-paid-candidate",
            AT + 6100,
        )
        .unwrap();
    assert_eq!(
        store
            .confirm(
                TARGET_CAMPAIGN_ID,
                MEMBER,
                candidate["candidateId"].as_str().unwrap(),
                TARGET_TERMS,
                "ordinary-paid-confirm",
                AT + 6200
            )
            .unwrap_err(),
        "promotionAlreadyPaid"
    );
    assert_eq!(pair.target.relation_count("promotion_attributions"), 0);
    assert_eq!(
        migration_count(&mut pair.target, "promotion_attribution_migration_evidence"),
        0
    );
}

#[test]
fn native_migration_rejects_payments_predating_verified_source_relationship() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    // A deliberately contradictory source history must not become trusted just
    // because an attribution row exists. No actual provider is contacted.
    fund_native(
        &pair.source,
        OLD_MEMBER,
        AT + 500,
        "source-prebinding-payment",
    );
    match pair.preview(AT + 6000) {
        Err(_) => (),
        Ok(preview) => assert!(pair
            .stage(
                preview["snapshotSha256"].as_str().unwrap(),
                "source-payment-before-binding",
                AT + 6001
            )
            .is_err()),
    }
    assert_not_staged(&mut pair);

    let Some(mut pair) = Pair::new(true) else {
        unreachable!();
    };
    // This separate fixture has a target account older than the source binding;
    // the payment is still too early for the historical relationship anchor.
    pair.target.db.execute("UPDATE accounts SET created_at=to_timestamp($2::bigint::double precision/1000.0) WHERE account_id IN ($1,$3)",&[&MEMBER,&((AT+100) as i64),&LEADER]).unwrap();
    pair.target
        .db
        .execute(
            "UPDATE identity_credentials SET created_at_ms=$1 WHERE account_id IN ($2,$3)",
            &[&((AT + 100) as i64), &MEMBER, &LEADER],
        )
        .unwrap();
    fund_native(&pair.target, MEMBER, AT + 500, "target-prebinding-payment");
    match pair.preview(AT + 6000) {
        Err(_) => (),
        Ok(preview) => assert!(pair
            .stage(
                preview["snapshotSha256"].as_str().unwrap(),
                "target-payment-before-binding",
                AT + 6001
            )
            .is_err()),
    }
    assert_not_staged(&mut pair);
}

#[test]
fn native_migration_does_not_whitelist_payments_arriving_after_staging() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    fund_native(&pair.target, MEMBER, AT + 5000, "whitelisted-payment");
    let preview = pair.preview(AT + 6000).unwrap();
    let staged = pair
        .stage(
            preview["snapshotSha256"].as_str().unwrap(),
            "stage-before-extra-payment",
            AT + 6001,
        )
        .unwrap();
    fund_native(&pair.target, MEMBER, AT + 6200, "not-whitelisted-payment-2");
    let accounts = pair.target.accounts_snapshot();
    assert!(pair
        .confirm(
            staged["candidateId"].as_str().unwrap(),
            "confirm-with-new-payment",
            AT + 7000
        )
        .is_err());
    assert_eq!(pair.target.relation_count("promotion_attributions"), 0);
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_confirmations"
        ),
        0
    );
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_payment_claims"
        ),
        0
    );
    assert_eq!(pair.target.accounts_snapshot(), accounts);
}

#[test]
fn native_migration_reuses_confirmation_capacity_without_reserving_at_stage() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    pair.target.db.execute("UPDATE promotion_campaigns SET member_limit=1,per_leader_member_limit=1 WHERE environment='test' AND campaign_id=$1",&[&TARGET_CAMPAIGN_ID]).unwrap();
    pair.target
        .account("migration_ordinary_member", AT + 2500, Some(&address('d')));
    let store = pair.target.store();
    let code = store.me(TARGET_CAMPAIGN_ID, LEADER, AT + 5000).unwrap()["leader"]["inviteCode"]
        .as_str()
        .unwrap()
        .to_owned();
    let ordinary = store
        .create_candidate(
            TARGET_CAMPAIGN_ID,
            &code,
            "migration_ordinary_member",
            "manual_code",
            "ordinary-capacity-candidate",
            AT + 5100,
        )
        .unwrap();
    let preview = pair.preview(AT + 6000).unwrap();
    let staged = pair
        .stage(
            preview["snapshotSha256"].as_str().unwrap(),
            "stage-no-reservation",
            AT + 6001,
        )
        .unwrap();
    assert_eq!(pair.target.relation_count("promotion_attributions"), 0);
    store
        .confirm(
            TARGET_CAMPAIGN_ID,
            "migration_ordinary_member",
            ordinary["candidateId"].as_str().unwrap(),
            TARGET_TERMS,
            "ordinary-claims-only-place",
            AT + 6500,
        )
        .unwrap();
    assert_eq!(
        pair.confirm(
            staged["candidateId"].as_str().unwrap(),
            "migration-must-recheck-capacity",
            AT + 7000
        )
        .unwrap_err(),
        "promotionBudgetFull"
    );
    assert_eq!(pair.target.relation_count("promotion_attributions"), 1);
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_confirmations"
        ),
        0
    );
    assert_eq!(
        pair.target
            .db
            .query_one(
                "SELECT reserved_micro_usd FROM promotion_campaigns WHERE campaign_id=$1",
                &[&TARGET_CAMPAIGN_ID]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[test]
fn native_migration_cannot_replace_an_ordinary_confirmed_target_relationship() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    let store = pair.target.store();
    let code = store.me(TARGET_CAMPAIGN_ID, LEADER, AT + 5000).unwrap()["leader"]["inviteCode"]
        .as_str()
        .unwrap()
        .to_owned();
    let candidate = store
        .create_candidate(
            TARGET_CAMPAIGN_ID,
            &code,
            MEMBER,
            "manual_code",
            "ordinary-existing-candidate",
            AT + 5100,
        )
        .unwrap();
    store
        .confirm(
            TARGET_CAMPAIGN_ID,
            MEMBER,
            candidate["candidateId"].as_str().unwrap(),
            TARGET_TERMS,
            "ordinary-existing-confirm",
            AT + 5200,
        )
        .unwrap();
    let original: Value = pair
        .target
        .db
        .query_one(
            "SELECT to_jsonb(a) FROM promotion_attributions a WHERE account_id=$1",
            &[&MEMBER],
        )
        .unwrap()
        .get(0);
    match pair.preview(AT + 6000) {
        Err(_) => (),
        Ok(preview) => assert!(pair
            .stage(
                preview["snapshotSha256"].as_str().unwrap(),
                "must-not-replace-existing",
                AT + 6001
            )
            .is_err()),
    }
    assert_eq!(
        migration_count(&mut pair.target, "promotion_attribution_migration_evidence"),
        0
    );
    let after: Value = pair
        .target
        .db
        .query_one(
            "SELECT to_jsonb(a) FROM promotion_attributions a WHERE account_id=$1",
            &[&MEMBER],
        )
        .unwrap()
        .get(0);
    assert_eq!(original, after);
}

#[test]
fn native_migration_expired_preparation_can_only_be_replaced_by_a_fresh_preparation() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    let preview = pair.preview(AT + 6000).unwrap();
    let staged = pair
        .stage(
            preview["snapshotSha256"].as_str().unwrap(),
            "first-expiring-stage",
            AT + 6001,
        )
        .unwrap();
    let old_candidate = staged["candidateId"].as_str().unwrap();
    let expired_at = staged["expiresAtMs"].as_u64().unwrap() + 1;
    assert!(
        expired_at < AT + 2000 + DAY,
        "fixture must remain within the unchanged registration confirmation window"
    );
    assert!(pair
        .confirm(old_candidate, "expired-proof-confirm", expired_at)
        .is_err());
    assert_eq!(pair.target.relation_count("promotion_attributions"), 0);
    let refreshed = pair.preview(expired_at + 1).unwrap();
    let replacement = pair
        .stage(
            refreshed["snapshotSha256"].as_str().unwrap(),
            "fresh-reviewed-stage",
            expired_at + 2,
        )
        .unwrap();
    assert_ne!(replacement["candidateId"], staged["candidateId"]);
    pair.confirm(
        replacement["candidateId"].as_str().unwrap(),
        "fresh-member-acceptance",
        expired_at + 3,
    )
    .unwrap();
    assert_eq!(
        migration_count(&mut pair.target, "promotion_attribution_migration_evidence"),
        2
    );
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_confirmations"
        ),
        1
    );
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_payment_claims"
        ),
        0
    );
    assert!(
        pair.preview(expired_at + 4).is_err(),
        "a finally confirmed source relation cannot be imported again"
    );
}

#[test]
fn native_migration_scope_rejects_production_same_realm_and_invalid_wallets() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    assert_eq!(
        pair.target
            .store()
            .preview_legacy_invitation_migration(&pair.target.url, &scope(), AT + 6000)
            .unwrap_err(),
        "promotionMigrationPolicyDenied"
    );
    let production = PromotionStore::new(pair.target.url.clone(), "production").unwrap();
    assert_eq!(
        production
            .preview_legacy_invitation_migration(&pair.source.url, &scope(), AT + 6000)
            .unwrap_err(),
        "promotionMigrationPolicyDenied"
    );
    let mut invalid = scope();
    invalid.member_wallet = address('0');
    assert!(pair
        .target
        .store()
        .preview_legacy_invitation_migration(&pair.source.url, &invalid, AT + 6000)
        .is_err());
    invalid.member_wallet = invalid.leader_wallet.clone();
    assert_eq!(
        pair.target
            .store()
            .preview_legacy_invitation_migration(&pair.source.url, &invalid, AT + 6000)
            .unwrap_err(),
        "promotionSelfReferral"
    );
    assert_not_staged(&mut pair);
}

#[test]
fn native_migration_rejects_cross_realm_payment_reuse_and_existing_source_revenue() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    // Deliberately contradictory verified-receipt fixtures reuse one canonical
    // chain/digest in different local ledgers. The migration must detect it;
    // this does not assert that either synthetic payment occurred on a chain.
    fund_native(
        &pair.source,
        OLD_MEMBER,
        AT + 5000,
        "source-canonical-payment",
    );
    fund_native(&pair.target, MEMBER, AT + 5000, "target-canonical-payment");
    let source = pair.source.accounts_snapshot();
    let target = pair.target.accounts_snapshot();
    assert!(pair.preview(AT + 6000).is_err());
    assert_not_staged(&mut pair);
    assert_eq!(pair.source.accounts_snapshot(), source);
    assert_eq!(pair.target.accounts_snapshot(), target);

    let Some(mut pair) = Pair::new(true) else {
        unreachable!();
    };
    fund_native(
        &pair.source,
        OLD_MEMBER,
        AT + 5000,
        "source-revenue-payment",
    );
    let config = SimulationConfig::default()
        .with_postgres_account_store(pair.source.url.clone())
        .unwrap()
        .with_billing_monthly_card_credit_price(Some(100))
        .unwrap();
    let card = config
        .buy_monthly_card_for_character(OLD_MEMBER, 0, "source-monthly", AT + 5400)
        .unwrap();
    config
        .activate_monthly_card_for_character(OLD_MEMBER, 0, card.unique_id, AT + 5401)
        .unwrap();
    pair.source
        .store()
        .reconcile_live_monthly_revenue(AT + 5500)
        .unwrap();
    assert_eq!(
        pair.source
            .db
            .query_one(
                "SELECT count(*) FROM promotion_revenue_allocations WHERE account_id=$1",
                &[&OLD_MEMBER]
            )
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    let source = pair.source.accounts_snapshot();
    assert!(pair.preview(AT + 6000).is_err());
    assert_not_staged(&mut pair);
    assert_eq!(
        pair.target.relation_count("promotion_revenue_allocations"),
        0
    );
    assert_eq!(pair.source.accounts_snapshot(), source);
}

#[test]
fn native_migration_rejects_source_audit_semantics_that_disagree_with_final_rows() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    pair.preview(AT + 6000).unwrap();
    // Preserve the original append-only confirmation audit. Model inconsistent
    // historical mutable rows, rather than disabling or rewriting that audit.
    let changed_terms = "inconsistent-historical-source-terms";
    pair.source
        .db
        .execute(
            "UPDATE promotion_campaigns SET terms_version=$2 WHERE campaign_id=$1",
            &[&SOURCE_CAMPAIGN_ID, &changed_terms],
        )
        .unwrap();
    pair.source
        .db
        .execute(
            "UPDATE promotion_leaders SET accepted_terms_version=$2 WHERE campaign_id=$1",
            &[&SOURCE_CAMPAIGN_ID, &changed_terms],
        )
        .unwrap();
    pair.source
        .db
        .execute(
            "UPDATE promotion_attributions SET terms_version=$2 WHERE campaign_id=$1",
            &[&SOURCE_CAMPAIGN_ID, &changed_terms],
        )
        .unwrap();
    assert!(
        pair.preview(AT + 6001).is_err(),
        "matching current rows cannot invent the member's historical terms acceptance"
    );
    assert_not_staged(&mut pair);

    let Some(mut pair) = Pair::new(true) else {
        unreachable!();
    };
    pair.preview(AT + 6000).unwrap();
    let other = "migration_inconsistent_source_leader";
    pair.source.account(other, AT + 90, None);
    pair.source.db.execute("UPDATE identity_credentials SET account_id=$2 WHERE account_id=$1 AND credential_kind='sui_wallet'", &[&OLD_LEADER, &other]).unwrap();
    let leader = pair
        .source
        .store()
        .apply(
            SOURCE_CAMPAIGN_ID,
            other,
            -1,
            SOURCE_TERMS,
            "second-source-leader-current-terms",
            AT + 110,
        )
        .unwrap();
    let leader_id = leader["leaderId"].as_str().unwrap();
    pair.source
        .db
        .execute(
            "UPDATE promotion_candidates SET leader_id=$2 WHERE campaign_id=$1 AND account_id=$3",
            &[&SOURCE_CAMPAIGN_ID, &leader_id, &OLD_MEMBER],
        )
        .unwrap();
    pair.source
        .db
        .execute(
            "UPDATE promotion_attributions SET leader_id=$2 WHERE campaign_id=$1 AND account_id=$3",
            &[&SOURCE_CAMPAIGN_ID, &leader_id, &OLD_MEMBER],
        )
        .unwrap();
    assert!(
        pair.preview(AT + 6001).is_err(),
        "a final-row leader change cannot override the original audited invitation owner"
    );
    assert_not_staged(&mut pair);
}

#[test]
fn native_migration_requires_original_attribution_candidate_and_registration_to_agree() {
    for assignment in [
        "source='manual_code'",
        "first_clicked_at_ms=first_clicked_at_ms+1",
        "registered_at_ms=registered_at_ms+1",
    ] {
        let Some(mut pair) = Pair::new(true) else {
            return;
        };
        pair.preview(AT + 6000).unwrap();
        // The assignment is a literal test allowlist, never external SQL input.
        pair.source.db.execute(&format!("UPDATE promotion_attributions SET {assignment} WHERE environment='test' AND campaign_id=$1 AND account_id=$2"), &[&SOURCE_CAMPAIGN_ID, &OLD_MEMBER]).unwrap();
        assert!(
            pair.preview(AT + 6001).is_err(),
            "contradictory original attribution metadata must be refused: {assignment}"
        );
        assert_not_staged(&mut pair);
    }
}

#[test]
fn native_migration_events_drain_through_the_normal_internal_projection_worker() {
    let Some(mut pair) = Pair::new(true) else {
        return;
    };
    let preview = pair.preview(AT + 6000).unwrap();
    let staged = pair
        .stage(
            preview["snapshotSha256"].as_str().unwrap(),
            "worker-stage",
            AT + 6001,
        )
        .unwrap();
    let migration = staged["migrationId"].as_str().unwrap();
    let store = pair.target.store();
    let prepared = store.process_pending_work(100, AT + 6500).unwrap();
    assert_eq!(prepared["failed"], 0);
    assert_eq!(prepared["externalDispatchImplemented"], false);
    let event = pair.target.db.query_one("SELECT status,last_error_code,result_json FROM promotion_outbox WHERE environment='test' AND event_type='migration.prepared' AND resource_id=$1", &[&migration]).unwrap();
    assert_eq!(event.get::<_, String>(0), "completed");
    assert!(event.get::<_, Option<String>>(1).is_none());
    assert_eq!(
        event.get::<_, Value>(2)["internalProjectionCompleted"],
        true
    );
    assert_eq!(pair.target.relation_count("promotion_attributions"), 0);
    assert_eq!(
        migration_count(
            &mut pair.target,
            "promotion_attribution_migration_payment_claims"
        ),
        0
    );
    pair.confirm(
        staged["candidateId"].as_str().unwrap(),
        "worker-member-current-terms",
        AT + 7000,
    )
    .unwrap();
    let confirmed = store.process_pending_work(100, AT + 7500).unwrap();
    assert_eq!(confirmed["failed"], 0);
    let events = pair.target.db.query("SELECT event_type,status,last_error_code,result_json FROM promotion_outbox WHERE environment='test' AND resource_id=$1 AND event_type IN ('migration.prepared','migration.confirmed') ORDER BY event_type", &[&migration]).unwrap();
    assert_eq!(events.len(), 2);
    for event in events {
        assert_eq!(event.get::<_, String>(1), "completed");
        assert!(event.get::<_, Option<String>>(2).is_none());
        assert_eq!(
            event.get::<_, Value>(3)["internalProjectionCompleted"],
            true
        );
        assert_eq!(event.get::<_, Value>(3)["externalDispatchCompleted"], false);
    }
    assert_eq!(
        store.process_pending_work(100, AT + 8000).unwrap()["claimed"],
        0
    );
    assert_eq!(
        pair.target
            .db
            .query_one(
                "SELECT count(*) FROM promotion_outbox WHERE status<>'completed'",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

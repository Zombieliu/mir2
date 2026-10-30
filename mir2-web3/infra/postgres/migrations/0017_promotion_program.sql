-- Numeron promotion control plane. No default campaign, money or approved rules.
-- The account identifier is the existing game account, never a wallet or nickname.
CREATE TABLE IF NOT EXISTS promotion_campaigns (
    environment TEXT NOT NULL CHECK (environment IN ('development','test','production')),
    campaign_id TEXT NOT NULL,
    terms_version TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','approved','active','closed','locked')),
    starts_at_ms BIGINT NOT NULL CHECK (starts_at_ms > 0),
    ends_at_ms BIGINT NOT NULL CHECK (ends_at_ms > starts_at_ms),
    rules_json JSONB NOT NULL,
    rules_approved BOOLEAN NOT NULL DEFAULT FALSE,
    recruitment_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    commission_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    payout_enabled BOOLEAN NOT NULL DEFAULT FALSE CHECK (NOT payout_enabled),
    budget_micro_usd BIGINT NOT NULL DEFAULT 0 CHECK (budget_micro_usd >= 0),
    reserved_micro_usd BIGINT NOT NULL DEFAULT 0 CHECK (reserved_micro_usd >= 0 AND reserved_micro_usd <= budget_micro_usd),
    member_limit BIGINT NOT NULL DEFAULT 0 CHECK (member_limit >= 0),
    leader_limit BIGINT NOT NULL DEFAULT 0 CHECK (leader_limit >= 0),
    per_leader_member_limit BIGINT NOT NULL DEFAULT 0 CHECK (per_leader_member_limit >= 0),
    per_member_revenue_cap_micro_usd BIGINT NOT NULL DEFAULT 0 CHECK (per_member_revenue_cap_micro_usd >= 0),
    input_watermark_ms BIGINT,
    configured_by TEXT NOT NULL,
    approved_by TEXT,
    created_at_ms BIGINT NOT NULL,
    PRIMARY KEY (environment,campaign_id),
    CHECK (NOT recruitment_enabled OR (rules_approved AND status='active' AND budget_micro_usd>0 AND member_limit>0 AND leader_limit>0 AND per_leader_member_limit>0 AND per_member_revenue_cap_micro_usd>0)),
    CHECK (NOT commission_enabled OR rules_approved)
    ,CHECK (approved_by IS NULL OR approved_by<>configured_by)
);

CREATE TABLE IF NOT EXISTS promotion_leaders (
    environment TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    leader_id TEXT NOT NULL,
    invite_code TEXT NOT NULL,
    display_character_index INTEGER NOT NULL,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'applied' CHECK (status IN ('applied','approved','restricted','suspended','closed')),
    accepted_terms_version TEXT NOT NULL,
    applied_at_ms BIGINT NOT NULL,
    PRIMARY KEY (environment,campaign_id,account_id),
    UNIQUE (environment,campaign_id,leader_id),
    UNIQUE (environment,invite_code),
    FOREIGN KEY (environment,campaign_id) REFERENCES promotion_campaigns(environment,campaign_id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS promotion_candidates (
    environment TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    candidate_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    leader_id TEXT NOT NULL,
    first_clicked_at_ms BIGINT NOT NULL,
    expires_at_ms BIGINT NOT NULL CHECK (expires_at_ms > first_clicked_at_ms),
    source TEXT NOT NULL CHECK (source IN ('invite_link','manual_code')),
    PRIMARY KEY (environment,candidate_id),
    FOREIGN KEY (environment,campaign_id,leader_id) REFERENCES promotion_leaders(environment,campaign_id,leader_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_promotion_candidates_owner ON promotion_candidates(environment,campaign_id,account_id,first_clicked_at_ms DESC);

CREATE TABLE IF NOT EXISTS promotion_invitation_clicks (
    environment TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    click_id TEXT NOT NULL,
    leader_id TEXT NOT NULL,
    clicked_at_ms BIGINT NOT NULL,
    expires_at_ms BIGINT NOT NULL CHECK(expires_at_ms>clicked_at_ms),
    claimed_by_account_id TEXT REFERENCES accounts(account_id) ON DELETE RESTRICT,
    PRIMARY KEY(environment,click_id),
    FOREIGN KEY(environment,campaign_id,leader_id) REFERENCES promotion_leaders(environment,campaign_id,leader_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_promotion_invitation_clicks_leader ON promotion_invitation_clicks(environment,campaign_id,leader_id,clicked_at_ms);

CREATE TABLE IF NOT EXISTS promotion_attributions (
    environment TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    leader_id TEXT NOT NULL,
    candidate_id TEXT NOT NULL,
    first_clicked_at_ms BIGINT NOT NULL,
    registered_at_ms BIGINT NOT NULL,
    confirmed_at_ms BIGINT NOT NULL,
    terms_version TEXT NOT NULL,
    source TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'bound' CHECK (status IN ('bound','review','qualified','excluded')),
    reason_code TEXT NOT NULL DEFAULT 'requirements_pending',
    qualified_at_ms BIGINT,
    qualification_algorithm_version TEXT,
    reserved_micro_usd BIGINT NOT NULL CHECK (reserved_micro_usd >= 0),
    PRIMARY KEY (environment,campaign_id,account_id),
    UNIQUE (environment,candidate_id),
    FOREIGN KEY (environment,campaign_id,leader_id) REFERENCES promotion_leaders(environment,campaign_id,leader_id) ON DELETE RESTRICT,
    FOREIGN KEY (environment,candidate_id) REFERENCES promotion_candidates(environment,candidate_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_promotion_attributions_leader ON promotion_attributions(environment,campaign_id,leader_id,confirmed_at_ms DESC,account_id);

-- Facts are emitted by trusted game authority; browser heartbeats are not facts.
CREATE TABLE IF NOT EXISTS promotion_game_facts (
    environment TEXT NOT NULL CHECK (environment IN ('development','test','production')),
    source TEXT NOT NULL CHECK (source IN ('level_commit','combat_commit','quest_commit')),
    kind TEXT NOT NULL CHECK (kind IN ('levelCommitted','combatKill','questCompleted')),
    source_event_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    character_index INTEGER NOT NULL,
    source_occurred_at_ms BIGINT NOT NULL,
    utc_day BIGINT NOT NULL,
    level INTEGER,
    evidence_ref TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    PRIMARY KEY (environment,source,source_event_id),
    CHECK ((source='level_commit' AND kind='levelCommitted' AND level BETWEEN 1 AND 65535)
        OR (source='combat_commit' AND kind='combatKill' AND level IS NULL)
        OR (source='quest_commit' AND kind='questCompleted' AND level IS NULL))
);
CREATE INDEX IF NOT EXISTS idx_promotion_game_facts_account ON promotion_game_facts(environment,account_id,source_occurred_at_ms);

-- Imported allocations require authenticated server evidence and proven paid lots.
-- Untraceable historical credit and gifts are retained as pending/excluded, never
-- silently promoted to paid revenue. Source callbacks cannot mutate old records.
CREATE TABLE IF NOT EXISTS promotion_revenue_allocations (
    environment TEXT NOT NULL CHECK (environment IN ('development','test','production')),
    allocation_id TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    source_order_id TEXT NOT NULL,
    subscription_id TEXT NOT NULL,
    paid_at_ms BIGINT NOT NULL,
    entitlement_at_ms BIGINT NOT NULL,
    service_start_ms BIGINT NOT NULL,
    service_end_ms BIGINT NOT NULL CHECK (service_end_ms>service_start_ms),
    eligible_micro_usd BIGINT NOT NULL CHECK (eligible_micro_usd>=0),
    original_currency TEXT NOT NULL,
    original_amount_minor BIGINT NOT NULL CHECK (original_amount_minor>=0),
    fx_snapshot_ref TEXT NOT NULL,
    source_evidence_ref TEXT NOT NULL,
    source_payload_hash TEXT NOT NULL,
    funding_status TEXT NOT NULL CHECK (funding_status IN ('verified_paid','pending','excluded')),
    reason_code TEXT NOT NULL,
    is_card_payment BOOLEAN NOT NULL,
    PRIMARY KEY (environment,allocation_id),
    UNIQUE (environment,subscription_id),
    UNIQUE (environment,campaign_id,source_order_id,subscription_id),
    FOREIGN KEY (environment,campaign_id) REFERENCES promotion_campaigns(environment,campaign_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_promotion_revenue_member ON promotion_revenue_allocations(environment,campaign_id,account_id);

CREATE TABLE IF NOT EXISTS promotion_credit_lot_allocations (
    environment TEXT NOT NULL,
    allocation_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    recharge_order_id TEXT NOT NULL,
    paid_credits BIGINT NOT NULL CHECK (paid_credits>0),
    net_micro_usd BIGINT NOT NULL CHECK (net_micro_usd>=0),
    source_evidence_ref TEXT NOT NULL,
    PRIMARY KEY(environment,allocation_id),
    FOREIGN KEY(environment,allocation_id) REFERENCES promotion_revenue_allocations(environment,allocation_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_promotion_lot_order ON promotion_credit_lot_allocations(environment,account_id,recharge_order_id);

CREATE TABLE IF NOT EXISTS promotion_revenue_adjustments (
    environment TEXT NOT NULL,
    adjustment_id TEXT NOT NULL,
    allocation_id TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    reversal_micro_usd BIGINT NOT NULL CHECK (reversal_micro_usd>0),
    occurred_at_ms BIGINT NOT NULL,
    evidence_ref TEXT NOT NULL,
    PRIMARY KEY (environment,adjustment_id),
    UNIQUE (environment,source_event_id),
    FOREIGN KEY (environment,allocation_id) REFERENCES promotion_revenue_allocations(environment,allocation_id) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS promotion_pending_reversals (
    environment TEXT NOT NULL CHECK(environment IN ('development','test','production')),
    source_event_id TEXT NOT NULL,
    allocation_id TEXT NOT NULL,
    reversal_micro_usd BIGINT NOT NULL CHECK(reversal_micro_usd>0),
    occurred_at_ms BIGINT NOT NULL,
    evidence_ref TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','applied','failed')),
    PRIMARY KEY(environment,source_event_id)
);

CREATE TABLE IF NOT EXISTS promotion_settlements (
    environment TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    settlement_id TEXT NOT NULL,
    leader_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'review' CHECK (status IN ('review','approved','cancelled')),
    algorithm_version TEXT NOT NULL,
    snapshot_json JSONB NOT NULL,
    snapshot_sha256 TEXT NOT NULL,
    total_micro_usd BIGINT NOT NULL CHECK (total_micro_usd>=0),
    payable_cents BIGINT NOT NULL CHECK (payable_cents>=0),
    reserve_micro_usd BIGINT NOT NULL CHECK (reserve_micro_usd>=0),
    created_by TEXT NOT NULL,
    reviewed_by TEXT,
    created_at_ms BIGINT NOT NULL,
    reviewed_at_ms BIGINT,
    PRIMARY KEY (environment,settlement_id),
    UNIQUE (environment,campaign_id,leader_id),
    FOREIGN KEY (environment,campaign_id,leader_id) REFERENCES promotion_leaders(environment,campaign_id,leader_id) ON DELETE RESTRICT,
    CHECK (reviewed_by IS NULL OR reviewed_by<>created_by),
    CHECK ((status='approved') = (reviewed_by IS NOT NULL AND reviewed_at_ms IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS promotion_payout_addresses (
    environment TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    version BIGINT NOT NULL CHECK (version>0),
    network TEXT NOT NULL,
    wallet_address TEXT NOT NULL,
    verified_credential_id TEXT NOT NULL,
    verified_at_ms BIGINT NOT NULL,
    cooldown_until_ms BIGINT NOT NULL,
    PRIMARY KEY (environment,account_id,version)
);
CREATE TABLE IF NOT EXISTS promotion_payouts (
    environment TEXT NOT NULL,
    payout_id TEXT NOT NULL,
    settlement_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','awaiting_approval','submitted','unknown','confirmed','failed','cancelled')),
    network TEXT NOT NULL,
    coin_type TEXT NOT NULL,
    destination TEXT NOT NULL,
    address_version BIGINT NOT NULL,
    amount_micro_usd BIGINT NOT NULL CHECK (amount_micro_usd>0),
    created_by TEXT NOT NULL,
    reviewed_by TEXT,
    tx_digest TEXT,
    created_at_ms BIGINT NOT NULL,
    PRIMARY KEY (environment,payout_id),
    UNIQUE (environment,settlement_id),
    FOREIGN KEY (environment,settlement_id) REFERENCES promotion_settlements(environment,settlement_id) ON DELETE RESTRICT,
    CHECK (reviewed_by IS NULL OR reviewed_by<>created_by),
    CHECK (status NOT IN ('submitted','unknown','confirmed') OR (tx_digest IS NOT NULL AND reviewed_by IS NOT NULL)),
    CHECK (status NOT IN ('submitted','unknown','confirmed')) -- No signer/transfer adapter in this release.
);

CREATE TABLE IF NOT EXISTS promotion_appeals (
    environment TEXT NOT NULL,
    appeal_id TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    category TEXT NOT NULL,
    message TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','review','resolved','rejected')),
    created_at_ms BIGINT NOT NULL,
    PRIMARY KEY (environment,appeal_id),
    FOREIGN KEY (environment,campaign_id) REFERENCES promotion_campaigns(environment,campaign_id) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS promotion_operator_roles (
    environment TEXT NOT NULL CHECK (environment IN ('development','test','production')),
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    role TEXT NOT NULL CHECK (role IN ('operations','risk','finance','payoutSubmit','payoutReview','auditor')),
    granted_by TEXT NOT NULL,
    granted_at_ms BIGINT NOT NULL,
    revoked_at_ms BIGINT,
    PRIMARY KEY(environment,account_id,role)
);
CREATE TABLE IF NOT EXISTS promotion_commission_runs (
    environment TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    leader_id TEXT NOT NULL,
    input_sha256 TEXT NOT NULL,
    input_json JSONB NOT NULL,
    result_json JSONB NOT NULL,
    created_at_ms BIGINT NOT NULL,
    PRIMARY KEY(environment,campaign_id,leader_id,input_sha256),
    FOREIGN KEY(environment,campaign_id,leader_id) REFERENCES promotion_leaders(environment,campaign_id,leader_id) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS promotion_commission_entries (
    environment TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    campaign_id TEXT NOT NULL,
    leader_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK(kind IN ('locked_reversal')),
    amount_micro_usd BIGINT NOT NULL CHECK(amount_micro_usd<0),
    source_adjustment_id TEXT NOT NULL,
    settlement_id TEXT NOT NULL,
    occurred_at_ms BIGINT NOT NULL,
    PRIMARY KEY(environment,entry_id),
    UNIQUE(environment,source_adjustment_id),
    FOREIGN KEY(environment,settlement_id) REFERENCES promotion_settlements(environment,settlement_id) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS promotion_idempotency (
    environment TEXT NOT NULL,
    account_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    operation TEXT NOT NULL,
    request_sha256 TEXT NOT NULL,
    response_json JSONB NOT NULL,
    created_at_ms BIGINT NOT NULL,
    PRIMARY KEY (environment,account_id,idempotency_key)
);
CREATE TABLE IF NOT EXISTS promotion_audit_log (
    environment TEXT NOT NULL,
    audit_id BIGSERIAL PRIMARY KEY,
    actor_id TEXT NOT NULL,
    action TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    evidence_ref TEXT NOT NULL,
    summary_json JSONB NOT NULL,
    occurred_at_ms BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_promotion_audit_environment ON promotion_audit_log(environment,occurred_at_ms DESC);
CREATE TABLE IF NOT EXISTS promotion_outbox (
    environment TEXT NOT NULL,
    outbox_id BIGSERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    event_json JSONB NOT NULL,
    created_at_ms BIGINT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','processing','completed','failed')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),
    last_error_code TEXT,
    last_attempt_at_ms BIGINT,
    next_attempt_at_ms BIGINT,
    completed_at_ms BIGINT,
    result_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    UNIQUE (environment,event_type,resource_id)
);
CREATE INDEX IF NOT EXISTS idx_promotion_outbox_due
    ON promotion_outbox(environment,status,next_attempt_at_ms,created_at_ms,outbox_id)
    WHERE status IN ('pending','failed');

CREATE OR REPLACE FUNCTION promotion_forbid_ledger_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'promotion append-only record cannot be changed'; END $$;
DO $$ DECLARE t TEXT; BEGIN
    FOREACH t IN ARRAY ARRAY['promotion_audit_log','promotion_game_facts','promotion_revenue_allocations','promotion_credit_lot_allocations','promotion_revenue_adjustments','promotion_payout_addresses','promotion_commission_runs','promotion_commission_entries'] LOOP
        IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname=t||'_immutable' AND tgrelid=to_regclass(t)) THEN
            EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE OR TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION promotion_forbid_ledger_mutation()',t||'_immutable',t);
        END IF;
    END LOOP;
END $$;

-- One reviewed test-realm invitation migration. Preparation preserves source
-- evidence but creates neither an attribution nor a reservation. Only the
-- member's later authenticated acceptance creates a confirmation and claims.
-- Account/character/payment JSON and historical promotion rows are untouched.

ALTER TABLE promotion_candidates
    DROP CONSTRAINT IF EXISTS promotion_candidates_source_check;
ALTER TABLE promotion_candidates
    ADD CONSTRAINT promotion_candidates_source_check
    CHECK (source IN ('invite_link','manual_code','migration_import'));
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='promotion_candidates'::regclass AND conname='promotion_candidates_migration_identity_key') THEN
        ALTER TABLE promotion_candidates ADD CONSTRAINT promotion_candidates_migration_identity_key
            UNIQUE (environment,campaign_id,candidate_id,account_id,leader_id);
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS promotion_attribution_migration_evidence (
    environment TEXT NOT NULL CHECK (environment='test'),
    migration_id TEXT NOT NULL,
    source_relation_key TEXT NOT NULL CHECK (source_relation_key ~ '^[0-9a-f]{64}$'),
    source_database_sha256 TEXT NOT NULL CHECK (source_database_sha256 ~ '^[0-9a-f]{64}$'),
    source_campaign_id TEXT NOT NULL CHECK (source_campaign_id='numeron-playtest-20261029'),
    source_account_sha256 TEXT NOT NULL CHECK (source_account_sha256 ~ '^[0-9a-f]{64}$'),
    source_leader_account_sha256 TEXT NOT NULL CHECK (source_leader_account_sha256 ~ '^[0-9a-f]{64}$'),
    source_confirmed_at_ms BIGINT NOT NULL CHECK (source_confirmed_at_ms>0),
    source_terms_version TEXT NOT NULL CHECK (length(source_terms_version)>0),
    source_evidence_json JSONB NOT NULL CHECK (jsonb_typeof(source_evidence_json)='object'),
    source_evidence_sha256 TEXT NOT NULL CHECK (source_evidence_sha256 ~ '^[0-9a-f]{64}$'),
    target_campaign_id TEXT NOT NULL CHECK (target_campaign_id='numeron-windows-playtest-20261030'),
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    leader_account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    leader_id TEXT NOT NULL,
    candidate_id TEXT NOT NULL,
    target_terms_version TEXT NOT NULL CHECK (length(target_terms_version)>0),
    eligibility_anchor_ms BIGINT NOT NULL CHECK (eligibility_anchor_ms>=source_confirmed_at_ms),
    prepared_at_ms BIGINT NOT NULL CHECK (prepared_at_ms>=eligibility_anchor_ms),
    expires_at_ms BIGINT NOT NULL CHECK (expires_at_ms>prepared_at_ms),
    snapshot_sha256 TEXT NOT NULL CHECK (snapshot_sha256 ~ '^[0-9a-f]{64}$'),
    prepared_by TEXT NOT NULL CHECK (length(prepared_by)>0),
    reviewed_by TEXT NOT NULL CHECK (length(reviewed_by)>0),
    evidence_ref TEXT NOT NULL CHECK (length(evidence_ref)>0),
    PRIMARY KEY (environment,migration_id),
    UNIQUE (environment,candidate_id),
    UNIQUE (environment,migration_id,source_relation_key,target_campaign_id,account_id,candidate_id,target_terms_version),
    CHECK (account_id<>leader_account_id),
    CHECK (prepared_by<>reviewed_by),
    FOREIGN KEY (environment,target_campaign_id,leader_id)
        REFERENCES promotion_leaders(environment,campaign_id,leader_id) ON DELETE RESTRICT,
    FOREIGN KEY (environment,target_campaign_id,candidate_id,account_id,leader_id)
        REFERENCES promotion_candidates(environment,campaign_id,candidate_id,account_id,leader_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_promotion_migration_pending
    ON promotion_attribution_migration_evidence(environment,target_campaign_id,account_id,prepared_at_ms DESC);

CREATE TABLE IF NOT EXISTS promotion_attribution_migration_confirmations (
    environment TEXT NOT NULL CHECK (environment='test'),
    migration_id TEXT NOT NULL,
    source_relation_key TEXT NOT NULL CHECK (source_relation_key ~ '^[0-9a-f]{64}$'),
    target_campaign_id TEXT NOT NULL CHECK (target_campaign_id='numeron-windows-playtest-20261030'),
    account_id TEXT NOT NULL,
    candidate_id TEXT NOT NULL,
    accepted_terms_version TEXT NOT NULL CHECK (length(accepted_terms_version)>0),
    confirmed_at_ms BIGINT NOT NULL CHECK (confirmed_at_ms>0),
    PRIMARY KEY (environment,migration_id),
    UNIQUE (environment,source_relation_key),
    UNIQUE (environment,target_campaign_id,account_id),
    UNIQUE (environment,candidate_id),
    UNIQUE (environment,migration_id,account_id),
    FOREIGN KEY (environment,migration_id,source_relation_key,target_campaign_id,account_id,candidate_id,accepted_terms_version)
        REFERENCES promotion_attribution_migration_evidence(environment,migration_id,source_relation_key,target_campaign_id,account_id,candidate_id,target_terms_version)
        ON DELETE RESTRICT,
    FOREIGN KEY (environment,target_campaign_id,account_id)
        REFERENCES promotion_attributions(environment,campaign_id,account_id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS promotion_attribution_migration_payment_claims (
    environment TEXT NOT NULL CHECK (environment='test'),
    migration_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    order_id TEXT NOT NULL,
    payment_identity_sha256 TEXT NOT NULL CHECK (payment_identity_sha256 ~ '^[0-9a-f]{64}$'),
    paid_at_ms BIGINT NOT NULL CHECK (paid_at_ms>0),
    PRIMARY KEY (environment,migration_id,order_id),
    UNIQUE (environment,payment_identity_sha256),
    UNIQUE (environment,account_id,order_id),
    FOREIGN KEY (environment,migration_id,account_id)
        REFERENCES promotion_attribution_migration_confirmations(environment,migration_id,account_id)
        ON DELETE RESTRICT
);

-- Preserve the exact evidence, real acceptance and payment claims after insert.
-- Use the existing promotion append-only guard, including TRUNCATE protection.
DO $$
DECLARE t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'promotion_attribution_migration_evidence',
        'promotion_attribution_migration_confirmations',
        'promotion_attribution_migration_payment_claims'
    ] LOOP
        IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgrelid=t::regclass AND tgname=t||'_immutable') THEN
            EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE OR TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION promotion_forbid_ledger_mutation()',t||'_immutable',t);
        END IF;
    END LOOP;
END $$;

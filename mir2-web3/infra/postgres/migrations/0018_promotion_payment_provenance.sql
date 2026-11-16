-- Funding is first committed atomically inside accounts.raw_json.billing.provenance.
-- The promotion importer retains each immutable source observation and supports
-- multiple exact FIFO credit lots funding one actually redeemed monthly unit.
ALTER TABLE promotion_credit_lot_allocations
    ADD COLUMN IF NOT EXISTS lot_id TEXT NOT NULL DEFAULT 'legacy-unproven';
ALTER TABLE promotion_credit_lot_allocations
    ADD COLUMN IF NOT EXISTS credits_offset BIGINT NOT NULL DEFAULT 0 CHECK(credits_offset>=0);
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='promotion_credit_lot_allocations'::regclass AND conname='promotion_credit_lot_allocations_pkey' AND pg_get_constraintdef(oid)='PRIMARY KEY (environment, allocation_id)') THEN
        ALTER TABLE promotion_credit_lot_allocations DROP CONSTRAINT promotion_credit_lot_allocations_pkey;
        ALTER TABLE promotion_credit_lot_allocations ADD PRIMARY KEY(environment,allocation_id,recharge_order_id,lot_id,credits_offset);
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS promotion_monthly_source_observations (
    environment TEXT NOT NULL CHECK(environment IN ('development','test','production')),
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    subscription_id TEXT NOT NULL,
    source_payload_sha256 TEXT NOT NULL CHECK(source_payload_sha256 ~ '^[0-9a-f]{64}$'),
    funding_status TEXT NOT NULL CHECK(funding_status IN ('verified_paid','pending','excluded')),
    reason_code TEXT NOT NULL,
    source_json JSONB NOT NULL,
    observed_at_ms BIGINT NOT NULL CHECK(observed_at_ms>0),
    PRIMARY KEY(environment,account_id,subscription_id,source_payload_sha256)
);
CREATE INDEX IF NOT EXISTS idx_promotion_monthly_source_account
    ON promotion_monthly_source_observations(environment,account_id,observed_at_ms);
DO $$ BEGIN
    IF NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname='promotion_monthly_source_observations_immutable' AND tgrelid='promotion_monthly_source_observations'::regclass) THEN
        CREATE TRIGGER promotion_monthly_source_observations_immutable BEFORE UPDATE OR DELETE OR TRUNCATE
            ON promotion_monthly_source_observations FOR EACH STATEMENT EXECUTE FUNCTION promotion_forbid_ledger_mutation();
    END IF;
END $$;

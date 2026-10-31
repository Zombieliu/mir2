CREATE TABLE IF NOT EXISTS promotion_revenue_source_holds (
    environment TEXT NOT NULL CHECK(environment IN ('development','test','production')),
    campaign_id TEXT NOT NULL,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    subscription_id TEXT NOT NULL,
    allocation_id TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    reason_code TEXT NOT NULL,
    source_payload_sha256 TEXT NOT NULL CHECK(source_payload_sha256 ~ '^[0-9a-f]{64}$'),
    target_reversal_micro_usd BIGINT NOT NULL DEFAULT 0 CHECK(target_reversal_micro_usd>=0),
    reversal_occurred_at_ms BIGINT,
    updated_at_ms BIGINT NOT NULL CHECK(updated_at_ms>0),
    PRIMARY KEY(environment,campaign_id,account_id,subscription_id),
    FOREIGN KEY(environment,campaign_id) REFERENCES promotion_campaigns(environment,campaign_id) ON DELETE RESTRICT,
    FOREIGN KEY(environment,allocation_id) REFERENCES promotion_revenue_allocations(environment,allocation_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_promotion_active_revenue_holds
    ON promotion_revenue_source_holds(environment,campaign_id,account_id) WHERE active;

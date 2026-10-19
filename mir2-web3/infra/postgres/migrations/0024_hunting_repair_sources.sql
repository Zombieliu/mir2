-- Server-issued repair quotes and their immutable complete source outcomes.
-- No character/account cascade: old incarnation evidence must survive deletion.
CREATE TABLE IF NOT EXISTS hunting_repair_quotes (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    quote_id TEXT NOT NULL CHECK (quote_id ~ '^[0-9a-f]{64}$'),
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    character_incarnation TEXT NOT NULL,
    source_digest TEXT NOT NULL,
    cost_bound_gold BIGINT NOT NULL CHECK (cost_bound_gold >= 0),
    quoted_save_revision BIGINT NOT NULL CHECK (quoted_save_revision >= 0),
    quote_json JSONB NOT NULL,
    quote_digest TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms >= 0),
    expires_at_ms BIGINT NOT NULL CHECK (expires_at_ms > created_at_ms),
    PRIMARY KEY (realm_id, quote_id),
    UNIQUE (realm_id,quote_id,account_id,character_index,character_incarnation)
);
CREATE INDEX IF NOT EXISTS hunting_repair_quote_owner_idx
    ON hunting_repair_quotes(realm_id,account_id,character_index,character_incarnation,created_at_ms);

CREATE TABLE IF NOT EXISTS hunting_repair_results (
    realm_id TEXT NOT NULL,
    quote_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    character_incarnation TEXT NOT NULL,
    source_digest TEXT NOT NULL,
    cost_bound_gold BIGINT NOT NULL CHECK (cost_bound_gold >= 0),
    payment_kind TEXT NOT NULL CHECK (payment_kind IN ('charged','free')),
    before_revision BIGINT NOT NULL CHECK (before_revision >= 0),
    after_revision BIGINT NOT NULL CHECK (after_revision = before_revision + 1),
    checkpoint_json JSONB NOT NULL,
    checkpoint_digest TEXT NOT NULL,
    operation_key TEXT NOT NULL,
    business_day BIGINT NOT NULL,
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms >= 0),
    CHECK ((payment_kind='free' AND cost_bound_gold=0) OR (payment_kind='charged' AND cost_bound_gold>0)),
    PRIMARY KEY (realm_id,quote_id),
    UNIQUE (realm_id,operation_key),
    FOREIGN KEY (realm_id,quote_id,account_id,character_index,character_incarnation)
        REFERENCES hunting_repair_quotes(realm_id,quote_id,account_id,character_index,character_incarnation),
    FOREIGN KEY (realm_id,operation_key)
        REFERENCES hunting_economy_operations(realm_id,operation_key)
        DEFERRABLE INITIALLY DEFERRED
);
DROP TRIGGER IF EXISTS hunting_repair_quote_immutable ON hunting_repair_quotes;
CREATE TRIGGER hunting_repair_quote_immutable
    BEFORE UPDATE OR DELETE ON hunting_repair_quotes
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_repair_result_immutable ON hunting_repair_results;
CREATE TRIGGER hunting_repair_result_immutable
    BEFORE UPDATE OR DELETE ON hunting_repair_results
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();

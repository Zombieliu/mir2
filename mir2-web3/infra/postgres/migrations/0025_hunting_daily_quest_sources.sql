-- The civil UTC+8 daily claim period is independent of the economy's 04:00
-- settlement day. Digest/reward edits never create another business claim.
CREATE TABLE IF NOT EXISTS hunting_daily_quest_source_checkpoints (
    realm_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    character_incarnation TEXT NOT NULL,
    quest_id INTEGER NOT NULL CHECK (quest_id IN (2100001,2100002,2100003,2100004)),
    cadence TEXT NOT NULL CHECK (cadence = 'daily-utc8-v1'),
    claim_period BIGINT NOT NULL CHECK (claim_period >= 0),
    source_key TEXT NOT NULL,
    source_digest TEXT NOT NULL,
    bound_gold BIGINT NOT NULL CHECK (bound_gold > 0),
    before_revision BIGINT NOT NULL CHECK (before_revision >= 0),
    after_revision BIGINT NOT NULL CHECK (after_revision = before_revision + 1),
    checkpoint_json JSONB NOT NULL,
    checkpoint_digest TEXT NOT NULL,
    operation_key TEXT NOT NULL,
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms >= 0),
    PRIMARY KEY (realm_id,account_id,character_index,character_incarnation,quest_id,cadence,claim_period),
    UNIQUE (realm_id,operation_key),
    CHECK (source_key = 'quest-daily-v1:' || quest_id::text || ':' || claim_period::text),
    FOREIGN KEY (realm_id,operation_key)
        REFERENCES hunting_economy_operations(realm_id,operation_key)
        DEFERRABLE INITIALLY DEFERRED
);

DROP TRIGGER IF EXISTS hunting_daily_quest_source_immutable ON hunting_daily_quest_source_checkpoints;
CREATE TRIGGER hunting_daily_quest_source_immutable
    BEFORE UPDATE OR DELETE ON hunting_daily_quest_source_checkpoints
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();

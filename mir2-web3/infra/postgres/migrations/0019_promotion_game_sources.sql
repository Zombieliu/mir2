-- Captured only by SourceOfTruth character-save transactions. In-memory
-- session state and Mirror writes never populate this immutable source inbox.
CREATE TABLE IF NOT EXISTS promotion_game_source_events (
    environment TEXT NOT NULL CHECK (environment IN ('development','test','production')),
    source_event_id TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('level_commit','combat_commit','quest_commit')),
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE RESTRICT,
    character_index INTEGER NOT NULL,
    source_save_version BIGINT NOT NULL CHECK (source_save_version > 0),
    source_revision BIGINT NOT NULL CHECK (source_revision >= 0),
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms > 0),
    fact_json JSONB NOT NULL,
    PRIMARY KEY (environment, source_event_id)
);
CREATE INDEX IF NOT EXISTS idx_promotion_game_source_backlog
    ON promotion_game_source_events(environment, committed_at_ms, source_event_id);

-- Separate delivery receipts leave immutable source evidence untouched. A
-- worker that crashes after ingestion retries the exact same fact and then
-- writes this receipt. record_game_fact validates equal payloads on replay.
CREATE TABLE IF NOT EXISTS promotion_game_source_deliveries (
    environment TEXT NOT NULL,
    source_event_id TEXT NOT NULL,
    delivered_at_ms BIGINT NOT NULL CHECK (delivered_at_ms > 0),
    PRIMARY KEY (environment, source_event_id),
    FOREIGN KEY (environment, source_event_id)
        REFERENCES promotion_game_source_events(environment, source_event_id) ON DELETE RESTRICT
);

CREATE OR REPLACE FUNCTION promotion_game_source_immutable() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'promotion game source records are append only';
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE table_name TEXT;
BEGIN
    FOREACH table_name IN ARRAY ARRAY['promotion_game_source_events','promotion_game_source_deliveries']
    LOOP
        IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname='promotion_game_source_append_only' AND tgrelid=to_regclass(table_name)) THEN
            EXECUTE format('CREATE TRIGGER promotion_game_source_append_only BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION promotion_game_source_immutable()', table_name);
        END IF;
        IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname='promotion_game_source_no_truncate' AND tgrelid=to_regclass(table_name)) THEN
            EXECUTE format('CREATE TRIGGER promotion_game_source_no_truncate BEFORE TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION promotion_game_source_immutable()', table_name);
        END IF;
    END LOOP;
END;
$$;

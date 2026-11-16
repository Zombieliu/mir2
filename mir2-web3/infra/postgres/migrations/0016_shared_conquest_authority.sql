-- Shared siege authority is independent of personal stage5_systems snapshots.
-- Registration fees, settlement rewards and affected Guild/account saves commit
-- in one transaction with this record. No legacy personal owner is imported.
CREATE TABLE IF NOT EXISTS shared_conquests (
    conquest_index INTEGER PRIMARY KEY CHECK (conquest_index > 0),
    raw_json JSONB NOT NULL CHECK (jsonb_typeof(raw_json) = 'object'),
    store_version BIGINT NOT NULL CHECK (store_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

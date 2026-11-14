-- Private sealed results and spent tombstones. Never a client-visible item blob.
-- The singleton is written by the same transaction as the character checkpoint.
CREATE TABLE IF NOT EXISTS sealed_stone_authority (
    singleton BOOLEAN PRIMARY KEY CHECK (singleton),
    registry_json JSONB,
    store_version BIGINT NOT NULL DEFAULT 1 CHECK (store_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO sealed_stone_authority (singleton) VALUES (TRUE)
    ON CONFLICT (singleton) DO NOTHING;

-- Canonical guild authority. Legacy stage5 guild blobs are intentionally not imported.
-- Guild mutations and affected character saves commit in one repository transaction.
CREATE TABLE IF NOT EXISTS shared_guilds (
    guild_id TEXT PRIMARY KEY,
    name_key TEXT NOT NULL UNIQUE DEFERRABLE INITIALLY DEFERRED,
    raw_json JSONB NOT NULL,
    store_version BIGINT NOT NULL CHECK (store_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS shared_guild_members (
    guild_id TEXT NOT NULL REFERENCES shared_guilds(guild_id) ON DELETE CASCADE,
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    FOREIGN KEY (account_id, character_index) REFERENCES characters(account_id, character_index) DEFERRABLE INITIALLY DEFERRED,
    PRIMARY KEY (guild_id, account_id, character_index),
    UNIQUE (account_id, character_index) DEFERRABLE INITIALLY DEFERRED
);
-- Deliberately no account cascade: deleting an account must remove its membership
-- through the same explicit guild transaction, not silently mutate another authority.

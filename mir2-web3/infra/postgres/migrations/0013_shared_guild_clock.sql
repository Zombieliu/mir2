-- A single fenced clock advances every guild once per admitted server minute.
-- Time is Unix milliseconds read from PostgreSQL clock_timestamp(), never a client.
CREATE TABLE IF NOT EXISTS shared_guild_clock (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    generation BIGINT NOT NULL DEFAULT 0 CHECK (generation >= 0),
    owner_token TEXT,
    lease_expires_ms BIGINT NOT NULL DEFAULT 0 CHECK (lease_expires_ms >= 0),
    minute_anchor_ms BIGINT NOT NULL DEFAULT 0 CHECK (minute_anchor_ms >= 0),
    store_version BIGINT NOT NULL DEFAULT 1 CHECK (store_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK ((owner_token IS NULL AND lease_expires_ms = 0) OR
           (owner_token IS NOT NULL AND owner_token ~ '^[0-9A-Fa-f]{32}$' AND generation > 0 AND lease_expires_ms > minute_anchor_ms))
);
INSERT INTO shared_guild_clock(singleton) VALUES(TRUE) ON CONFLICT(singleton) DO NOTHING;

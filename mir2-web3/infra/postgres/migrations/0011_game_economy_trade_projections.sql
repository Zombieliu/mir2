-- Durable per-character materialization work for a two-party economy trade.
--
-- The ledger, receipt/outbox envelope, and both rows are inserted by the same
-- PostgreSQL transaction. `own_offer` and `incoming_offer` are validated Rust
-- `SharedTradeOffer` JSON; the outbox event id commits their canonical payload.
CREATE TABLE IF NOT EXISTS game_economy_trade_projections (
    event_id TEXT NOT NULL
        REFERENCES game_economy_transactions(event_id) ON DELETE CASCADE,
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    own_offer JSONB NOT NULL,
    incoming_offer JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'projected')),
    projected_at_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (event_id, account_id, character_index)
);

CREATE INDEX IF NOT EXISTS idx_game_economy_trade_projections_pending_identity
    ON game_economy_trade_projections(account_id, character_index, status, event_id);
-- A committed Zone pickup must never be recreated merely because the recipient
-- checkpoint was full or its private save failed. The intent is canonical JSON
-- bound by the receipt/outbox envelope; it is inserted atomically with both.
CREATE TABLE IF NOT EXISTS game_economy_ground_drop_projections (
    event_id TEXT NOT NULL
        REFERENCES game_economy_transactions(event_id) ON DELETE CASCADE,
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    intent JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'projected')),
    projected_at_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (event_id, account_id, character_index)
);

CREATE INDEX IF NOT EXISTS idx_game_economy_ground_drop_projections_pending_identity
    ON game_economy_ground_drop_projections(account_id, character_index, status, event_id);

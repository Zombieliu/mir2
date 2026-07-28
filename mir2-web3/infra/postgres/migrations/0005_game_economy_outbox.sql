CREATE TABLE IF NOT EXISTS game_economy_balances (
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    asset_kind TEXT NOT NULL,
    asset_key TEXT NOT NULL,
    amount BIGINT NOT NULL CHECK (
        amount >= 0
        AND (asset_kind <> 'item' OR amount <= 1)
    ),
    balance_version BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, character_index, asset_kind, asset_key)
);

CREATE TABLE IF NOT EXISTS game_economy_transactions (
    idempotency_key TEXT PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    transaction_kind TEXT NOT NULL,
    receipt JSONB NOT NULL,
    committed_at_ms BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS game_economy_outbox (
    event_id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE
        REFERENCES game_economy_transactions(idempotency_key),
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'delivering', 'dispatched', 'dead_letter')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at_ms BIGINT NOT NULL DEFAULT 0,
    locked_by TEXT,
    locked_until_ms BIGINT,
    last_error TEXT,
    created_at_ms BIGINT NOT NULL,
    dispatched_at_ms BIGINT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_game_economy_outbox_delivery
    ON game_economy_outbox(status, next_attempt_at_ms, created_at_ms);

CREATE TABLE IF NOT EXISTS game_economy_inbox (
    consumer_id TEXT NOT NULL,
    event_id TEXT NOT NULL,
    payload_digest TEXT NOT NULL,
    processed_at_ms BIGINT NOT NULL,
    payload JSONB NOT NULL,
    PRIMARY KEY (consumer_id, event_id)
);

CREATE TABLE IF NOT EXISTS game_economy_reconciliation_runs (
    run_id TEXT PRIMARY KEY,
    started_at_ms BIGINT NOT NULL,
    completed_at_ms BIGINT,
    pending_count BIGINT NOT NULL DEFAULT 0,
    expired_delivery_count BIGINT NOT NULL DEFAULT 0,
    dead_letter_count BIGINT NOT NULL DEFAULT 0,
    transaction_without_outbox_count BIGINT NOT NULL DEFAULT 0,
    negative_balance_count BIGINT NOT NULL DEFAULT 0,
    healthy BOOLEAN NOT NULL DEFAULT false,
    details JSONB NOT NULL DEFAULT '{}'::jsonb
);

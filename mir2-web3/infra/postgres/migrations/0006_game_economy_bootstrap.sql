-- Gate 18: one-way admission of legacy character assets into the transactional
-- economy ledger. The immutable digest lets operators prove which live runtime
-- snapshot established the opening balances; subsequent gameplay changes must
-- use game_economy_transactions and may never silently re-bootstrap.
CREATE TABLE IF NOT EXISTS game_economy_bootstraps (
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    snapshot_digest TEXT NOT NULL,
    gold BIGINT NOT NULL CHECK (gold >= 0),
    experience BIGINT NOT NULL CHECK (experience >= 0),
    item_quantity BIGINT NOT NULL CHECK (item_quantity >= 0),
    item_kind_count INTEGER NOT NULL CHECK (item_kind_count >= 0),
    bootstrapped_at_ms BIGINT NOT NULL,
    details JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, character_index)
);

CREATE INDEX IF NOT EXISTS idx_game_economy_bootstraps_recent
    ON game_economy_bootstraps(bootstrapped_at_ms DESC);

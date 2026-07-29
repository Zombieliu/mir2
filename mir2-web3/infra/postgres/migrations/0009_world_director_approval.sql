CREATE TABLE IF NOT EXISTS world_director_control_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    checkpoint_json JSONB NOT NULL,
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    updated_at_ms BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE world_director_control_state
    ADD COLUMN IF NOT EXISTS revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0);

CREATE TABLE IF NOT EXISTS world_director_audit (
    audit_id TEXT PRIMARY KEY,
    proposal_id TEXT,
    action TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    from_status TEXT,
    to_status TEXT,
    reason TEXT NOT NULL,
    occurred_at_ms BIGINT NOT NULL,
    previous_hash TEXT NOT NULL,
    record_hash TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_world_director_audit_recent
    ON world_director_audit(occurred_at_ms DESC, audit_id DESC);
CREATE INDEX IF NOT EXISTS idx_world_director_audit_proposal
    ON world_director_audit(proposal_id, occurred_at_ms DESC);

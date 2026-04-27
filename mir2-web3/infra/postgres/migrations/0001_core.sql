CREATE TABLE IF NOT EXISTS accounts (
    account_id TEXT PRIMARY KEY,
    password_snapshot TEXT NOT NULL DEFAULT '',
    storage_size INTEGER NOT NULL DEFAULT 80,
    has_expanded_storage BOOLEAN NOT NULL DEFAULT FALSE,
    expanded_storage_expiry_time_binary_datetime BIGINT NOT NULL DEFAULT 0,
    storage_password_snapshot TEXT NOT NULL DEFAULT '',
    storage_password_last_set_binary_datetime BIGINT NOT NULL DEFAULT 0,
    is_banned BOOLEAN NOT NULL DEFAULT FALSE,
    ban_reason TEXT NOT NULL DEFAULT '',
    ban_until_ms BIGINT,
    banned_at_ms BIGINT,
    store_version BIGINT NOT NULL DEFAULT 0,
    raw_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS store_version BIGINT NOT NULL DEFAULT 0;
ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS is_banned BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS ban_reason TEXT NOT NULL DEFAULT '';
ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS ban_until_ms BIGINT;
ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS banned_at_ms BIGINT;

CREATE TABLE IF NOT EXISTS characters (
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    character_index INTEGER NOT NULL,
    character_name TEXT NOT NULL,
    class TEXT NOT NULL,
    gender TEXT NOT NULL,
    level INTEGER NOT NULL,
    raw_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, character_index)
);

CREATE INDEX IF NOT EXISTS idx_characters_name ON characters(character_name);

CREATE TABLE IF NOT EXISTS character_saves (
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    character_index INTEGER NOT NULL,
    map_file_name TEXT NOT NULL DEFAULT '',
    map_title TEXT NOT NULL DEFAULT '',
    position_x INTEGER NOT NULL DEFAULT 0,
    position_y INTEGER NOT NULL DEFAULT 0,
    direction TEXT NOT NULL DEFAULT 'down',
    hp INTEGER NOT NULL DEFAULT 0,
    max_hp INTEGER NOT NULL DEFAULT 0,
    mp INTEGER NOT NULL DEFAULT 0,
    gold BIGINT NOT NULL DEFAULT 0,
    credit BIGINT NOT NULL DEFAULT 0,
    snapshot_json JSONB NOT NULL,
    stage5_systems_json JSONB,
    save_version BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, character_index),
    FOREIGN KEY (account_id, character_index)
        REFERENCES characters(account_id, character_index)
        ON DELETE CASCADE
);

ALTER TABLE character_saves
    ADD COLUMN IF NOT EXISTS save_version BIGINT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_character_saves_map ON character_saves(map_file_name);

CREATE TABLE IF NOT EXISTS admin_commands (
    command_id TEXT PRIMARY KEY,
    command_type TEXT NOT NULL,
    status TEXT NOT NULL,
    operator_id TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    trace_id TEXT NOT NULL,
    envelope_json JSONB NOT NULL,
    result_message TEXT,
    error_code TEXT,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_admin_commands_recent
    ON admin_commands(updated_at_ms DESC, created_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_admin_commands_target
    ON admin_commands(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_admin_commands_operator
    ON admin_commands(operator_id);

CREATE TABLE IF NOT EXISTS admin_audit_records (
    audit_id TEXT PRIMARY KEY,
    command_id TEXT NOT NULL REFERENCES admin_commands(command_id) ON DELETE RESTRICT,
    operator_id TEXT NOT NULL,
    operator_email TEXT NOT NULL,
    operator_role_snapshot TEXT NOT NULL,
    permission TEXT NOT NULL,
    target_json JSONB NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    status TEXT NOT NULL,
    error_code TEXT,
    trace_id TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    completed_at_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_admin_audit_recent
    ON admin_audit_records(created_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_admin_audit_command
    ON admin_audit_records(command_id);
CREATE INDEX IF NOT EXISTS idx_admin_audit_target
    ON admin_audit_records(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_admin_audit_operator
    ON admin_audit_records(operator_id);

CREATE TABLE IF NOT EXISTS admin_approvals (
    approval_id TEXT PRIMARY KEY,
    command_id TEXT NOT NULL,
    command_type TEXT NOT NULL,
    status TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    requested_reason TEXT NOT NULL,
    decided_by TEXT,
    decision_reason TEXT,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    decided_at_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_admin_approvals_recent
    ON admin_approvals(updated_at_ms DESC, created_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_admin_approvals_command
    ON admin_approvals(command_id);
CREATE INDEX IF NOT EXISTS idx_admin_approvals_status
    ON admin_approvals(status);

ALTER TABLE admin_approvals
    ADD COLUMN IF NOT EXISTS updated_at_ms BIGINT NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS admin_outbox (
    outbox_id TEXT PRIMARY KEY,
    command_id TEXT REFERENCES admin_commands(command_id) ON DELETE RESTRICT,
    topic TEXT NOT NULL,
    payload_json JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    nats_status TEXT NOT NULL DEFAULT 'pending',
    redpanda_status TEXT NOT NULL DEFAULT 'not_configured',
    last_error TEXT,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at_ms BIGINT,
    dispatched_at_ms BIGINT,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE admin_outbox
    ALTER COLUMN command_id DROP NOT NULL;
ALTER TABLE admin_outbox
    ADD COLUMN IF NOT EXISTS nats_status TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE admin_outbox
    ADD COLUMN IF NOT EXISTS redpanda_status TEXT NOT NULL DEFAULT 'not_configured';
ALTER TABLE admin_outbox
    ADD COLUMN IF NOT EXISTS last_error TEXT;
ALTER TABLE admin_outbox
    ADD COLUMN IF NOT EXISTS dispatched_at_ms BIGINT;

CREATE INDEX IF NOT EXISTS idx_admin_outbox_pending
    ON admin_outbox(status, next_attempt_at_ms, created_at_ms);

CREATE TABLE IF NOT EXISTS admin_activities (
    activity_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    activity_type TEXT NOT NULL,
    status TEXT NOT NULL,
    signal TEXT NOT NULL DEFAULT '',
    start_at_ms BIGINT,
    reward TEXT NOT NULL DEFAULT '',
    condition TEXT NOT NULL DEFAULT '',
    realms_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_by TEXT NOT NULL DEFAULT '',
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE admin_activities
    ADD COLUMN IF NOT EXISTS reward TEXT NOT NULL DEFAULT '';
ALTER TABLE admin_activities
    ADD COLUMN IF NOT EXISTS condition TEXT NOT NULL DEFAULT '';
ALTER TABLE admin_activities
    ADD COLUMN IF NOT EXISTS realms_json JSONB NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE admin_activities
    ADD COLUMN IF NOT EXISTS created_by TEXT NOT NULL DEFAULT '';
ALTER TABLE admin_activities
    ADD COLUMN IF NOT EXISTS updated_at_ms BIGINT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_admin_activities_status
    ON admin_activities(status, updated_at_ms DESC);

CREATE TABLE IF NOT EXISTS admin_market_price_feeds (
    item TEXT PRIMARY KEY,
    latest_price BIGINT NOT NULL,
    sample_count INTEGER NOT NULL,
    source TEXT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_admin_market_price_feeds_updated
    ON admin_market_price_feeds(updated_at_ms DESC);

CREATE TABLE IF NOT EXISTS admin_trade_graph_edges (
    edge_id TEXT PRIMARY KEY,
    from_player_id TEXT NOT NULL,
    to_player_id TEXT NOT NULL,
    signal TEXT NOT NULL,
    risk TEXT NOT NULL,
    evidence TEXT NOT NULL DEFAULT '',
    updated_at_ms BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_admin_trade_graph_edges_updated
    ON admin_trade_graph_edges(updated_at_ms DESC);

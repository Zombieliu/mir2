-- Commercial identity control plane.  The game account blob remains the
-- authoritative gameplay record; these tables own credentials, recovery,
-- revocable browser sessions and security audit history.

CREATE TABLE IF NOT EXISTS identity_credentials (
    credential_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    credential_kind TEXT NOT NULL,
    credential_subject TEXT NOT NULL,
    display_name TEXT NOT NULL DEFAULT '',
    created_at_ms BIGINT NOT NULL,
    last_used_at_ms BIGINT,
    revoked_at_ms BIGINT,
    revoked_reason TEXT,
    metadata_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT identity_credentials_kind_check
        CHECK (credential_kind IN ('password', 'sui_passkey', 'sui_wallet')),
    CONSTRAINT identity_credentials_subject_check
        CHECK (length(credential_subject) BETWEEN 1 AND 512)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_identity_credentials_live_subject
    ON identity_credentials(credential_kind, credential_subject)
    WHERE revoked_at_ms IS NULL;
CREATE INDEX IF NOT EXISTS idx_identity_credentials_account
    ON identity_credentials(account_id, created_at_ms DESC);

CREATE TABLE IF NOT EXISTS identity_sessions (
    session_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    auth_method TEXT NOT NULL,
    credential_id TEXT REFERENCES identity_credentials(credential_id) ON DELETE SET NULL,
    issued_at_ms BIGINT NOT NULL,
    last_seen_at_ms BIGINT NOT NULL,
    expires_at_ms BIGINT NOT NULL,
    revoked_at_ms BIGINT,
    revoked_reason TEXT,
    peer_fingerprint TEXT NOT NULL DEFAULT '',
    user_agent_summary TEXT NOT NULL DEFAULT '',
    gateway_id TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT identity_sessions_auth_method_check
        CHECK (auth_method IN ('password', 'sui_passkey', 'sui_wallet', 'recovery')),
    CONSTRAINT identity_sessions_expiry_check
        CHECK (expires_at_ms > issued_at_ms)
);

CREATE INDEX IF NOT EXISTS idx_identity_sessions_account_live
    ON identity_sessions(account_id, expires_at_ms DESC)
    WHERE revoked_at_ms IS NULL;
CREATE INDEX IF NOT EXISTS idx_identity_sessions_expiry
    ON identity_sessions(expires_at_ms);

CREATE TABLE IF NOT EXISTS identity_recovery_codes (
    recovery_code_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    used_at_ms BIGINT,
    revoked_at_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT identity_recovery_code_hash_check
        CHECK (length(code_hash) >= 32)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_identity_recovery_codes_hash
    ON identity_recovery_codes(code_hash);
CREATE INDEX IF NOT EXISTS idx_identity_recovery_codes_account_live
    ON identity_recovery_codes(account_id, created_at_ms DESC)
    WHERE used_at_ms IS NULL AND revoked_at_ms IS NULL;

CREATE TABLE IF NOT EXISTS identity_audit_events (
    event_id TEXT PRIMARY KEY,
    account_id TEXT,
    event_type TEXT NOT NULL,
    outcome TEXT NOT NULL,
    reason_code TEXT NOT NULL DEFAULT '',
    session_id TEXT,
    credential_id TEXT,
    peer_fingerprint TEXT NOT NULL DEFAULT '',
    user_agent_summary TEXT NOT NULL DEFAULT '',
    trace_id TEXT NOT NULL,
    details_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at_ms BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT identity_audit_outcome_check
        CHECK (outcome IN ('success', 'failure', 'blocked'))
);

CREATE INDEX IF NOT EXISTS idx_identity_audit_account_recent
    ON identity_audit_events(account_id, occurred_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_identity_audit_type_recent
    ON identity_audit_events(event_type, occurred_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_identity_audit_trace
    ON identity_audit_events(trace_id);


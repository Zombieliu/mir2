-- Production AI daily-report control plane.
--
-- Metrics/evidence are immutable inputs. Narrative can be regenerated, but every
-- run, review and delivery attempt is retained for audit and incident recovery.

CREATE TABLE IF NOT EXISTS admin_daily_reports (
    report_id TEXT PRIMARY KEY,
    report_date TEXT NOT NULL,
    timezone TEXT NOT NULL,
    scope TEXT NOT NULL,
    status TEXT NOT NULL,
    source_window_start_ms BIGINT NOT NULL,
    source_window_end_ms BIGINT NOT NULL,
    metrics_json JSONB NOT NULL,
    evidence_json JSONB NOT NULL,
    operations_markdown TEXT NOT NULL,
    player_markdown TEXT NOT NULL,
    generation_source TEXT NOT NULL,
    model TEXT,
    prompt_version TEXT NOT NULL,
    input_sha256 TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    created_by TEXT NOT NULL,
    reviewed_by TEXT,
    review_reason TEXT,
    published_by TEXT,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    reviewed_at_ms BIGINT,
    published_at_ms BIGINT,
    UNIQUE (report_date, timezone, scope)
);

CREATE INDEX IF NOT EXISTS admin_daily_reports_status_date_idx
    ON admin_daily_reports (status, report_date DESC);

CREATE TABLE IF NOT EXISTS admin_daily_report_runs (
    run_id TEXT PRIMARY KEY,
    report_id TEXT,
    report_date TEXT NOT NULL,
    trigger TEXT NOT NULL,
    status TEXT NOT NULL,
    model TEXT,
    input_sha256 TEXT,
    error_code TEXT,
    error_message TEXT,
    started_at_ms BIGINT NOT NULL,
    completed_at_ms BIGINT,
    FOREIGN KEY (report_id) REFERENCES admin_daily_reports(report_id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS admin_daily_report_runs_date_idx
    ON admin_daily_report_runs (report_date DESC, started_at_ms DESC);

CREATE TABLE IF NOT EXISTS admin_daily_report_deliveries (
    delivery_id TEXT PRIMARY KEY,
    report_id TEXT NOT NULL,
    channel TEXT NOT NULL,
    destination_label TEXT NOT NULL,
    status TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at_ms BIGINT,
    last_attempt_at_ms BIGINT,
    delivered_at_ms BIGINT,
    provider_message_id TEXT,
    last_error TEXT,
    created_at_ms BIGINT NOT NULL,
    updated_at_ms BIGINT NOT NULL,
    UNIQUE (report_id, channel, destination_label),
    FOREIGN KEY (report_id) REFERENCES admin_daily_reports(report_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS admin_daily_report_deliveries_due_idx
    ON admin_daily_report_deliveries (status, next_attempt_at_ms);

CREATE TABLE IF NOT EXISTS admin_daily_report_events (
    event_id TEXT PRIMARY KEY,
    report_id TEXT,
    event_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT,
    payload_json JSONB NOT NULL,
    occurred_at_ms BIGINT NOT NULL,
    FOREIGN KEY (report_id) REFERENCES admin_daily_reports(report_id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS admin_daily_report_events_report_idx
    ON admin_daily_report_events (report_id, occurred_at_ms DESC);

-- A consumer/schema/event acknowledgement is durable only together with an
-- immutable metric revision. Default export never grants full-realm coverage.
CREATE TABLE IF NOT EXISTS hunting_economy_metric_exports (
    realm_id TEXT NOT NULL,
    event_id TEXT NOT NULL,
    consumer_version TEXT NOT NULL CHECK (length(btrim(consumer_version)) > 0),
    business_day BIGINT NOT NULL,
    metric_revision BIGINT NOT NULL CHECK (metric_revision > 0),
    source_payload_digest TEXT NOT NULL,
    disposition TEXT NOT NULL CHECK (disposition IN ('inserted_partial', 'reused_existing')),
    exported_at_ms BIGINT NOT NULL CHECK (exported_at_ms >= 0),
    PRIMARY KEY (realm_id, event_id, consumer_version),
    FOREIGN KEY (realm_id, event_id) REFERENCES hunting_economy_outbox(realm_id, event_id),
    FOREIGN KEY (realm_id, business_day, metric_revision)
        REFERENCES hunting_economy_daily_metric_versions(realm_id, business_day, metric_revision)
);
CREATE INDEX IF NOT EXISTS hunting_economy_metric_export_day_idx
    ON hunting_economy_metric_exports(realm_id, business_day, consumer_version);
DROP TRIGGER IF EXISTS hunting_economy_metric_export_immutable ON hunting_economy_metric_exports;
CREATE TRIGGER hunting_economy_metric_export_immutable
    BEFORE UPDATE OR DELETE ON hunting_economy_metric_exports
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
REVOKE ALL ON hunting_economy_metric_exports FROM PUBLIC;

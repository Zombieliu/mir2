CREATE DATABASE IF NOT EXISTS mir2_events;

CREATE TABLE IF NOT EXISTS mir2_events.admin_events
(
    event_id String,
    event_type LowCardinality(String),
    schema_version UInt16,
    command_id String,
    operator_id String,
    status LowCardinality(String),
    occurred_at_ms UInt64,
    payload_json String,
    ingested_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = MergeTree
ORDER BY (event_type, occurred_at_ms, event_id);

CREATE TABLE IF NOT EXISTS mir2_events.admin_command_events
(
    command_id String,
    status LowCardinality(String),
    result_message Nullable(String),
    error_code Nullable(String),
    updated_at_ms UInt64,
    ingested_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = MergeTree
ORDER BY (updated_at_ms, command_id);

DROP TABLE IF EXISTS mir2_events.admin_events_mv;
DROP TABLE IF EXISTS mir2_events.admin_command_events_mv_v2;
DROP TABLE IF EXISTS mir2_events.admin_events_kafka;

CREATE TABLE mir2_events.admin_events_kafka
(
    eventId String,
    eventType String,
    schemaVersion UInt16,
    commandId String,
    operatorId String,
    status String,
    occurredAtMs UInt64,
    payloadJson String
)
ENGINE = Kafka
SETTINGS
    kafka_broker_list = 'redpanda:9092',
    kafka_topic_list = 'admin.command.succeeded,admin.command.failed,admin.command.denied,admin.approval.requested,admin.approval.approved,admin.approval.rejected,admin.outbox.retry,admin.outbox.dead_letter',
    kafka_group_name = 'clickhouse-admin-events-v4',
    kafka_format = 'JSONEachRow',
    kafka_num_consumers = 1;

CREATE MATERIALIZED VIEW mir2_events.admin_events_mv
TO mir2_events.admin_events
AS
SELECT
    eventId AS event_id,
    eventType AS event_type,
    schemaVersion AS schema_version,
    commandId AS command_id,
    operatorId AS operator_id,
    status,
    occurredAtMs AS occurred_at_ms,
    payloadJson AS payload_json
FROM mir2_events.admin_events_kafka;

CREATE MATERIALIZED VIEW mir2_events.admin_command_events_mv_v2
TO mir2_events.admin_command_events
AS
SELECT
    commandId AS command_id,
    status,
    nullIf(JSONExtractString(payloadJson, 'resultMessage'), '') AS result_message,
    nullIf(JSONExtractString(payloadJson, 'errorCode'), '') AS error_code,
    occurredAtMs AS updated_at_ms
FROM mir2_events.admin_events_kafka
WHERE eventType IN ('admin.command.succeeded', 'admin.command.failed', 'admin.command.denied');

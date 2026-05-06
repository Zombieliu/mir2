CREATE DATABASE IF NOT EXISTS mir2_events;

CREATE TABLE IF NOT EXISTS mir2_events.gameplay_events
(
    event_id String,
    event_type LowCardinality(String),
    schema_version UInt16,
    occurred_at_ms UInt64,
    zone_id LowCardinality(String),
    command_kind LowCardinality(String),
    account_id Nullable(String),
    character_index Nullable(Int32),
    character_name Nullable(String),
    packet_count UInt64,
    snapshot_tick UInt64,
    payload_json String,
    ingested_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = MergeTree
ORDER BY (zone_id, occurred_at_ms, event_id);

DROP TABLE IF EXISTS mir2_events.gameplay_events_mv;
DROP TABLE IF EXISTS mir2_events.gameplay_events_kafka;

CREATE TABLE mir2_events.gameplay_events_kafka
(
    eventId String,
    eventType String,
    schemaVersion UInt16,
    occurredAtMs UInt64,
    zoneId String,
    commandKind String,
    accountId Nullable(String),
    characterIndex Nullable(Int32),
    characterName Nullable(String),
    packetCount UInt64,
    snapshotTick UInt64,
    payloadJson String
)
ENGINE = Kafka
SETTINGS
    kafka_broker_list = 'redpanda:9092',
    kafka_topic_list = 'gameplay.command.executed',
    kafka_group_name = 'clickhouse-gameplay-events-v1',
    kafka_format = 'JSONEachRow',
    kafka_num_consumers = 1;

CREATE MATERIALIZED VIEW mir2_events.gameplay_events_mv
TO mir2_events.gameplay_events
AS
SELECT
    eventId AS event_id,
    eventType AS event_type,
    schemaVersion AS schema_version,
    occurredAtMs AS occurred_at_ms,
    zoneId AS zone_id,
    commandKind AS command_kind,
    accountId AS account_id,
    characterIndex AS character_index,
    characterName AS character_name,
    packetCount AS packet_count,
    snapshotTick AS snapshot_tick,
    payloadJson AS payload_json
FROM mir2_events.gameplay_events_kafka;

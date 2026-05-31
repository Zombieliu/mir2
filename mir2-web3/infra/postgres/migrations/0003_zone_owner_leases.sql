-- 0003_zone_owner_leases.sql
--
-- Durable, fenced zone-ownership leases for distributed gateway failover.
--
-- A single row per zone records the current owner, a monotonically increasing
-- fencing token (bumped only when ownership is *stolen* after TTL expiry), and an
-- absolute expiry timestamp. This lets a second gateway process take authoritative
-- ownership of a zone after the previous owner dies and its lease lapses, while a
-- returning zombie owner is fenced out (its stale token fails renewal and any
-- token-checked write).
--
-- This table is the coordination primitive for failover; transferring the live
-- in-memory zone runtime to the new owner still requires the zone-owner RPC
-- transport (tracked separately).

CREATE TABLE IF NOT EXISTS zone_owner_leases (
    zone_id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    fencing_token BIGINT NOT NULL,
    expires_at_ms BIGINT NOT NULL,
    heartbeat_at_ms BIGINT NOT NULL,
    acquired_at_ms BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_zone_owner_leases_owner ON zone_owner_leases(owner_id);
CREATE INDEX IF NOT EXISTS idx_zone_owner_leases_expiry ON zone_owner_leases(expires_at_ms);

-- Startup migration only. Recovery workers never create or repair authority.
-- The exact recovered ZoneBaseSnapshot survives a crash before activation.
-- Canonical accounts remain in their source tables; no account credentials or
-- connection strings are copied into this audit.
CREATE TABLE IF NOT EXISTS public.zone_player_drop_recovery_receipts (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    zone_id TEXT NOT NULL CHECK (length(zone_id)>0),
    recovery_id TEXT NOT NULL CHECK (length(recovery_id)>0),
    owner_id TEXT NOT NULL CHECK (length(owner_id)>0),
    owner_epoch BIGINT NOT NULL CHECK (owner_epoch>0),
    source_digest TEXT NOT NULL CHECK (source_digest ~ '^[0-9a-f]{64}$'),
    operation_set_digest TEXT NOT NULL CHECK (operation_set_digest ~ '^[0-9a-f]{64}$'),
    operation_count BIGINT NOT NULL CHECK (operation_count>=0),
    observation_sequence BIGINT NOT NULL CHECK (observation_sequence>=0),
    base_snapshot_id TEXT NOT NULL CHECK (length(base_snapshot_id)>0),
    base_snapshot_digest TEXT NOT NULL CHECK (base_snapshot_digest ~ '^[0-9a-f]{64}$'),
    base_snapshot_bytes BYTEA NOT NULL CHECK (octet_length(base_snapshot_bytes)>0),
    receipt JSONB NOT NULL CHECK (jsonb_typeof(receipt)='object'),
    receipt_digest TEXT NOT NULL CHECK (receipt_digest ~ '^[0-9a-f]{64}$'),
    observed_at_ms BIGINT NOT NULL CHECK (observed_at_ms>0),
    valid_until_ms BIGINT NOT NULL CHECK (valid_until_ms>observed_at_ms),
    PRIMARY KEY(source_namespace,zone_id,recovery_id)
);
CREATE INDEX IF NOT EXISTS zone_player_drop_recovery_receipts_owner
    ON public.zone_player_drop_recovery_receipts(source_namespace,zone_id,owner_epoch,observed_at_ms);

CREATE OR REPLACE FUNCTION public.protect_zone_player_drop_recovery_receipt()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'immutable player drop recovery receipt';
END;
$$;
DROP TRIGGER IF EXISTS zone_player_drop_recovery_receipt_immutable
    ON public.zone_player_drop_recovery_receipts;
CREATE TRIGGER zone_player_drop_recovery_receipt_immutable
    BEFORE UPDATE OR DELETE ON public.zone_player_drop_recovery_receipts
    FOR EACH ROW EXECUTE FUNCTION public.protect_zone_player_drop_recovery_receipt();

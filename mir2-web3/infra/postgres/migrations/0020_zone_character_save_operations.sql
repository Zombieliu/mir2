-- Provisioned at startup, never by a Zone mutation worker. This ledger lives
-- beside accounts/character_saves so publication and its receipt share one tx.
CREATE TABLE IF NOT EXISTS public.zone_character_save_operations (
    operation_id TEXT PRIMARY KEY,
    rpc_session_id TEXT NOT NULL,
    zone_id TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    fencing_token BIGINT NOT NULL CHECK (fencing_token > 0),
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    checkpoint_digest TEXT NOT NULL CHECK (checkpoint_digest ~ '^[0-9a-f]{64}$'),
    frozen_checkpoint JSONB NOT NULL CHECK (jsonb_typeof(frozen_checkpoint) = 'object'),
    expected_digest TEXT NOT NULL CHECK (expected_digest ~ '^[0-9a-f]{64}$'),
    expected_checkpoint JSONB NOT NULL CHECK (jsonb_typeof(expected_checkpoint) = 'object'),
    last_access_binary_datetime BIGINT,
    status TEXT NOT NULL CHECK (status IN ('Prepared', 'Committed', 'Released')),
    prepared_at_ms BIGINT NOT NULL CHECK (prepared_at_ms > 0),
    committed_at_ms BIGINT,
    released_at_ms BIGINT,
    saved_checkpoint JSONB,
    account_store_version BIGINT,
    character_save_version BIGINT,
    CHECK (
        (status = 'Prepared' AND committed_at_ms IS NULL AND released_at_ms IS NULL
         AND saved_checkpoint IS NULL AND account_store_version IS NULL
         AND character_save_version IS NULL)
        OR
        (status IN ('Committed', 'Released') AND committed_at_ms IS NOT NULL
         AND committed_at_ms > 0 AND saved_checkpoint IS NOT NULL
         AND account_store_version IS NOT NULL AND account_store_version > 0
         AND character_save_version IS NOT NULL AND character_save_version > 0
         AND ((status = 'Committed' AND released_at_ms IS NULL)
              OR (status = 'Released' AND released_at_ms IS NOT NULL AND released_at_ms > 0)))
    )
);

-- Never cascade receipts when an account is removed: a former owner can still
-- reconcile its exact acknowledged/unknown operation after a takeover.
CREATE INDEX IF NOT EXISTS zone_character_save_operations_binding
    ON public.zone_character_save_operations (zone_id, rpc_session_id, prepared_at_ms);

CREATE OR REPLACE FUNCTION public.protect_zone_character_save_operation()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.operation_id, NEW.rpc_session_id, NEW.zone_id, NEW.owner_id,
        NEW.fencing_token, NEW.account_id, NEW.character_index,
        NEW.checkpoint_digest, NEW.frozen_checkpoint, NEW.expected_digest, NEW.expected_checkpoint,
        NEW.last_access_binary_datetime, NEW.prepared_at_ms)
       IS DISTINCT FROM
       (OLD.operation_id, OLD.rpc_session_id, OLD.zone_id, OLD.owner_id,
        OLD.fencing_token, OLD.account_id, OLD.character_index,
        OLD.checkpoint_digest, OLD.frozen_checkpoint, OLD.expected_digest, OLD.expected_checkpoint,
        OLD.last_access_binary_datetime, OLD.prepared_at_ms) THEN
        RAISE EXCEPTION 'immutable zone save operation binding';
    END IF;
    IF OLD.status = 'Prepared' AND NEW.status = 'Committed' THEN
        RETURN NEW;
    END IF;
    IF OLD.status IN ('Committed', 'Released') AND
       (NEW.saved_checkpoint, NEW.account_store_version,
        NEW.character_save_version, NEW.committed_at_ms)
       IS DISTINCT FROM
       (OLD.saved_checkpoint, OLD.account_store_version,
        OLD.character_save_version, OLD.committed_at_ms) THEN
        RAISE EXCEPTION 'immutable zone save operation evidence';
    END IF;
    IF OLD.status = 'Committed' AND NEW.status = 'Released' THEN
        RETURN NEW;
    END IF;
    IF NEW IS NOT DISTINCT FROM OLD THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'invalid zone save operation transition';
END;
$$;
DROP TRIGGER IF EXISTS zone_character_save_operation_immutable
    ON public.zone_character_save_operations;
CREATE TRIGGER zone_character_save_operation_immutable
    BEFORE UPDATE ON public.zone_character_save_operations
    FOR EACH ROW EXECUTE FUNCTION public.protect_zone_character_save_operation();

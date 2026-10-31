-- Startup migration only. Workers never create or repair authority tables.
CREATE TABLE IF NOT EXISTS public.zone_player_drop_operations (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    operation_key TEXT NOT NULL,
    world_effect_id TEXT NOT NULL,
    zone_id TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    owner_epoch BIGINT NOT NULL CHECK (owner_epoch > 0),
    operation_digest TEXT NOT NULL CHECK (operation_digest ~ '^[0-9a-f]{64}$'),
    intent_digest TEXT NOT NULL CHECK (intent_digest ~ '^[0-9a-f]{64}$'),
    operation JSONB NOT NULL CHECK (jsonb_typeof(operation) = 'object'),
    expires_at_ms BIGINT NOT NULL CHECK (expires_at_ms > 0),
    status TEXT NOT NULL CHECK (status IN ('Prepared','Committed','Consumed','Expired','Aborted')),
    prepared_at_ms BIGINT NOT NULL CHECK (prepared_at_ms > 0),
    event_id TEXT,
    committed_at_ms BIGINT,
    terminal_event_id TEXT,
    terminal_at_ms BIGINT,
    terminal_account_id TEXT,
    terminal_character_index INTEGER,
    ack_owner_id TEXT,
    ack_owner_epoch BIGINT,
    ack_sequence BIGINT,
    ack_at_ms BIGINT,
    aborted_at_ms BIGINT,
    abort_audit JSONB,
    abort_digest TEXT,
    PRIMARY KEY(source_namespace,operation_key),
    UNIQUE(source_namespace,world_effect_id),
    UNIQUE(source_namespace,event_id),
    UNIQUE(source_namespace,terminal_event_id),
    CHECK ((status IN ('Prepared','Aborted') AND event_id IS NULL AND committed_at_ms IS NULL)
        OR (status IN ('Committed','Consumed','Expired') AND event_id IS NOT NULL AND committed_at_ms > 0)),
    CHECK ((status IN ('Prepared','Committed','Aborted') AND terminal_event_id IS NULL
            AND terminal_at_ms IS NULL AND terminal_account_id IS NULL AND terminal_character_index IS NULL)
        OR (status='Expired' AND terminal_event_id IS NULL AND terminal_at_ms >= expires_at_ms
            AND terminal_account_id IS NULL AND terminal_character_index IS NULL)
        OR (status='Consumed' AND terminal_event_id IS NOT NULL AND terminal_at_ms > 0
            AND terminal_account_id IS NOT NULL AND terminal_character_index IS NOT NULL)),
    CHECK ((ack_owner_id IS NULL AND ack_owner_epoch IS NULL AND ack_sequence IS NULL AND ack_at_ms IS NULL)
        OR (ack_owner_id IS NOT NULL AND ack_owner_epoch > 0 AND ack_sequence >= 0 AND ack_at_ms > 0)),
    CHECK ((status='Aborted' AND aborted_at_ms IS NOT NULL AND aborted_at_ms>0
            AND abort_audit IS NOT NULL AND jsonb_typeof(abort_audit)='object'
            AND abort_digest IS NOT NULL AND abort_digest ~ '^[0-9a-f]{64}$')
        OR (status<>'Aborted' AND aborted_at_ms IS NULL AND abort_audit IS NULL AND abort_digest IS NULL))
);
CREATE INDEX IF NOT EXISTS zone_player_drop_operations_zone_enumeration
    ON public.zone_player_drop_operations(source_namespace,zone_id,operation_key);

CREATE TABLE IF NOT EXISTS public.zone_player_drop_observation_audit (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    world_effect_id TEXT NOT NULL,
    observer_owner_id TEXT NOT NULL,
    observer_epoch BIGINT NOT NULL CHECK (observer_epoch > 0),
    observer_sequence BIGINT NOT NULL CHECK (observer_sequence >= 0),
    observed_state TEXT NOT NULL CHECK (observed_state IN ('Committed','Consumed','Expired')),
    observed_at_ms BIGINT NOT NULL CHECK (observed_at_ms > 0),
    PRIMARY KEY(source_namespace,world_effect_id,observer_epoch,observer_sequence),
    FOREIGN KEY(source_namespace,world_effect_id) REFERENCES public.zone_player_drop_operations(source_namespace,world_effect_id)
);

CREATE TABLE IF NOT EXISTS public.zone_player_drop_abort_audit (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    operation_key TEXT NOT NULL,
    recovery_id TEXT NOT NULL CHECK (length(recovery_id)>0),
    recovery_owner_id TEXT NOT NULL CHECK (length(recovery_owner_id)>0),
    recovery_epoch BIGINT NOT NULL CHECK (recovery_epoch>0),
    audited_receipt JSONB NOT NULL CHECK (jsonb_typeof(audited_receipt)='object'),
    audit_digest TEXT NOT NULL CHECK (audit_digest ~ '^[0-9a-f]{64}$'),
    PRIMARY KEY(source_namespace,operation_key),
    UNIQUE(source_namespace,operation_key,recovery_id),
    FOREIGN KEY(source_namespace,operation_key) REFERENCES public.zone_player_drop_operations(source_namespace,operation_key)
);

CREATE OR REPLACE FUNCTION public.protect_zone_player_drop_abort_audit()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'immutable player drop abort audit';
END;
$$;
DROP TRIGGER IF EXISTS zone_player_drop_abort_audit_immutable ON public.zone_player_drop_abort_audit;
CREATE TRIGGER zone_player_drop_abort_audit_immutable
    BEFORE UPDATE OR DELETE ON public.zone_player_drop_abort_audit
    FOR EACH ROW EXECUTE FUNCTION public.protect_zone_player_drop_abort_audit();

CREATE OR REPLACE FUNCTION public.protect_zone_player_drop_operation()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.source_namespace,NEW.operation_key,NEW.world_effect_id,NEW.zone_id,NEW.owner_id,NEW.owner_epoch,
        NEW.operation_digest,NEW.intent_digest,NEW.operation,NEW.expires_at_ms,NEW.prepared_at_ms)
       IS DISTINCT FROM
       (OLD.source_namespace,OLD.operation_key,OLD.world_effect_id,OLD.zone_id,OLD.owner_id,OLD.owner_epoch,
        OLD.operation_digest,OLD.intent_digest,OLD.operation,OLD.expires_at_ms,OLD.prepared_at_ms) THEN
        RAISE EXCEPTION 'immutable player drop operation binding';
    END IF;
    IF OLD.status<>'Prepared' AND (NEW.event_id,NEW.committed_at_ms)
       IS DISTINCT FROM (OLD.event_id,OLD.committed_at_ms) THEN
        RAISE EXCEPTION 'immutable player drop source commit';
    END IF;
    IF OLD.status='Aborted' AND (NEW.aborted_at_ms,NEW.abort_audit,NEW.abort_digest)
        IS DISTINCT FROM (OLD.aborted_at_ms,OLD.abort_audit,OLD.abort_digest) THEN
        RAISE EXCEPTION 'immutable player drop abort receipt';
    END IF;
    IF OLD.status IN ('Consumed','Expired') AND
       (NEW.status,NEW.terminal_event_id,NEW.terminal_at_ms,NEW.terminal_account_id,NEW.terminal_character_index)
       IS DISTINCT FROM
       (OLD.status,OLD.terminal_event_id,OLD.terminal_at_ms,OLD.terminal_account_id,OLD.terminal_character_index) THEN
        RAISE EXCEPTION 'immutable terminal player drop effect';
    END IF;
    IF NEW.status<>OLD.status AND NOT
       ((OLD.status='Prepared' AND NEW.status IN ('Committed','Aborted')) OR
        (OLD.status='Committed' AND NEW.status IN ('Consumed','Expired'))) THEN
        RAISE EXCEPTION 'invalid player drop operation transition';
    END IF;
    IF OLD.ack_owner_epoch IS NOT NULL AND
       (NEW.ack_owner_epoch IS NULL OR NEW.ack_owner_epoch<OLD.ack_owner_epoch OR
        (NEW.ack_owner_epoch=OLD.ack_owner_epoch AND
          (NEW.ack_owner_id<>OLD.ack_owner_id OR NEW.ack_sequence<OLD.ack_sequence))) THEN
        RAISE EXCEPTION 'player drop observation cannot move backwards';
    END IF;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS zone_player_drop_operation_immutable ON public.zone_player_drop_operations;
CREATE TRIGGER zone_player_drop_operation_immutable
    BEFORE UPDATE ON public.zone_player_drop_operations
    FOR EACH ROW EXECUTE FUNCTION public.protect_zone_player_drop_operation();

-- No INSERT seed: a deployment must census every canonical/offline/world item
-- and retained receipt before an audited administrator initializes this row.
-- Full u64 IDs cross the driver boundary as decimal text, never signed BIGINT.
CREATE TABLE IF NOT EXISTS public.zone_player_drop_item_uid_authority (
    source_namespace TEXT PRIMARY KEY DEFAULT current_schema() CHECK (length(source_namespace)>0),
    singleton BOOLEAN NOT NULL DEFAULT TRUE CHECK (singleton),
    issued_through NUMERIC(20,0) NOT NULL CHECK (issued_through BETWEEN 0 AND 18446744073709551615),
    census_digest TEXT NOT NULL CHECK (census_digest ~ '^[0-9a-f]{64}$'),
    initialized_by TEXT NOT NULL CHECK (length(initialized_by)>0),
    initialized_at_ms BIGINT NOT NULL CHECK (initialized_at_ms>0)
);
CREATE TABLE IF NOT EXISTS public.zone_player_drop_item_uid_census_audit (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    audit_id BIGSERIAL PRIMARY KEY,
    previous_floor NUMERIC(20,0),
    censused_floor NUMERIC(20,0) NOT NULL CHECK (censused_floor BETWEEN 0 AND 18446744073709551615),
    census_digest TEXT NOT NULL CHECK (census_digest ~ '^[0-9a-f]{64}$'),
    administrator_id TEXT NOT NULL CHECK (length(administrator_id)>0),
    recorded_at_ms BIGINT NOT NULL CHECK (recorded_at_ms>0)
);
CREATE TABLE IF NOT EXISTS public.zone_player_drop_item_uid_batches (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    operation_key TEXT NOT NULL,
    binding JSONB NOT NULL CHECK (jsonb_typeof(binding)='object'),
    item_count INTEGER NOT NULL CHECK (item_count BETWEEN 1 AND 1024),
    reserved_at_ms BIGINT NOT NULL CHECK (reserved_at_ms>0),
    PRIMARY KEY(source_namespace,operation_key)
);
CREATE TABLE IF NOT EXISTS public.zone_player_drop_item_uid_reservations (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    operation_key TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal>=0),
    item_uid NUMERIC(20,0) NOT NULL CHECK (item_uid BETWEEN 1 AND 18446744073709551615),
    PRIMARY KEY(source_namespace,operation_key,ordinal),
    UNIQUE(source_namespace,item_uid),
    FOREIGN KEY(source_namespace,operation_key) REFERENCES public.zone_player_drop_item_uid_batches(source_namespace,operation_key)
);
CREATE OR REPLACE FUNCTION public.protect_zone_player_drop_uid_authority()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.source_namespace<>OLD.source_namespace OR NEW.issued_through<OLD.issued_through THEN
        RAISE EXCEPTION 'player drop UID authority cannot move backwards';
    END IF;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS zone_player_drop_uid_authority_monotonic ON public.zone_player_drop_item_uid_authority;
CREATE TRIGGER zone_player_drop_uid_authority_monotonic
    BEFORE UPDATE ON public.zone_player_drop_item_uid_authority
    FOR EACH ROW EXECUTE FUNCTION public.protect_zone_player_drop_uid_authority();

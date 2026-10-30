-- Startup/admin migration only. An admission precedes bootstrap/UID effects;
-- only an acknowledged INSERT can issue a live continuation capability.
CREATE TABLE IF NOT EXISTS public.zone_player_drop_admissions (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    operation_key TEXT NOT NULL,
    world_effect_id TEXT NOT NULL,
    zone_id TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    owner_epoch BIGINT NOT NULL CHECK (owner_epoch>0),
    frozen JSONB NOT NULL CHECK (jsonb_typeof(frozen)='object'),
    admission_digest TEXT NOT NULL CHECK (admission_digest ~ '^[0-9a-f]{64}$'),
    expected_uid_count INTEGER NOT NULL CHECK (expected_uid_count BETWEEN 0 AND 1024),
    status TEXT NOT NULL CHECK (status IN ('Admitted','Linked','Aborted')),
    admitted_at_ms BIGINT NOT NULL CHECK (admitted_at_ms>0),
    linked_operation_key TEXT,
    linked_operation_digest TEXT,
    linked_at_ms BIGINT,
    aborted_at_ms BIGINT,
    abort_audit JSONB,
    abort_audit_digest TEXT,
    PRIMARY KEY(source_namespace,operation_key),
    UNIQUE(source_namespace,world_effect_id),
    FOREIGN KEY(source_namespace,linked_operation_key)
        REFERENCES public.zone_player_drop_operations(source_namespace,operation_key),
    CHECK ((status='Linked' AND linked_operation_key IS NOT NULL AND linked_operation_key=operation_key
            AND linked_operation_digest IS NOT NULL AND linked_operation_digest ~ '^[0-9a-f]{64}$'
            AND linked_at_ms IS NOT NULL AND linked_at_ms>0)
        OR (status<>'Linked' AND linked_operation_key IS NULL AND linked_operation_digest IS NULL AND linked_at_ms IS NULL)),
    CHECK ((status='Aborted' AND aborted_at_ms IS NOT NULL AND aborted_at_ms>0
            AND abort_audit IS NOT NULL AND jsonb_typeof(abort_audit)='object'
            AND abort_audit_digest IS NOT NULL AND abort_audit_digest ~ '^[0-9a-f]{64}$')
        OR (status<>'Aborted' AND aborted_at_ms IS NULL AND abort_audit IS NULL AND abort_audit_digest IS NULL))
);
CREATE INDEX IF NOT EXISTS zone_player_drop_admissions_zone
    ON public.zone_player_drop_admissions(source_namespace,zone_id,operation_key);

CREATE TABLE IF NOT EXISTS public.zone_player_drop_admission_abort_audit (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    operation_key TEXT NOT NULL,
    audit JSONB NOT NULL CHECK (jsonb_typeof(audit)='object'),
    audit_digest TEXT NOT NULL CHECK (audit_digest ~ '^[0-9a-f]{64}$'),
    PRIMARY KEY(source_namespace,operation_key),
    FOREIGN KEY(source_namespace,operation_key)
        REFERENCES public.zone_player_drop_admissions(source_namespace,operation_key)
);
CREATE OR REPLACE FUNCTION public.protect_zone_player_drop_admission_abort_audit()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'immutable player drop admission abort audit'; END;
$$;
DROP TRIGGER IF EXISTS zone_player_drop_admission_abort_audit_immutable
    ON public.zone_player_drop_admission_abort_audit;
CREATE TRIGGER zone_player_drop_admission_abort_audit_immutable BEFORE UPDATE OR DELETE
    ON public.zone_player_drop_admission_abort_audit FOR EACH ROW
    EXECUTE FUNCTION public.protect_zone_player_drop_admission_abort_audit();

CREATE OR REPLACE FUNCTION public.protect_zone_player_drop_admission()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'immutable player drop admission'; END IF;
    IF (NEW.source_namespace,NEW.operation_key,NEW.world_effect_id,NEW.zone_id,NEW.owner_id,NEW.owner_epoch,
        NEW.frozen,NEW.admission_digest,NEW.expected_uid_count,NEW.admitted_at_ms)
       IS DISTINCT FROM
       (OLD.source_namespace,OLD.operation_key,OLD.world_effect_id,OLD.zone_id,OLD.owner_id,OLD.owner_epoch,
        OLD.frozen,OLD.admission_digest,OLD.expected_uid_count,OLD.admitted_at_ms) THEN
        RAISE EXCEPTION 'immutable player drop admission payload';
    END IF;
    IF OLD.status<>'Admitted' AND NEW IS DISTINCT FROM OLD THEN
        RAISE EXCEPTION 'immutable resolved player drop admission';
    END IF;
    IF NEW.status<>OLD.status AND NOT (OLD.status='Admitted' AND NEW.status IN ('Linked','Aborted')) THEN
        RAISE EXCEPTION 'invalid player drop admission transition';
    END IF;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS zone_player_drop_admission_immutable ON public.zone_player_drop_admissions;
CREATE TRIGGER zone_player_drop_admission_immutable BEFORE UPDATE OR DELETE
    ON public.zone_player_drop_admissions FOR EACH ROW
    EXECUTE FUNCTION public.protect_zone_player_drop_admission();

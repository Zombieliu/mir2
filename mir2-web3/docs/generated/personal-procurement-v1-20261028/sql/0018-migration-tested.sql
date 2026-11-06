-- Procurement lives in accounts.raw_json and character_saves.snapshot_json.
-- A pre-V12 executable does not deserialize all of this authority. It must not erase
-- custody, pending deliveries, fee history or recovery jobs during an update.
-- Rollback: disable new procurement; retain V12 recovery workers and this guard.
-- Removing the guard requires an audited export/archive migration after every
-- custody and delivery has settled. An ordinary downgrade is never a rollback.
CREATE OR REPLACE FUNCTION mir2_guard_procurement_authority()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    collection_name TEXT;
    entry_key TEXT;
BEGIN
    IF OLD.raw_json ? 'procurement'
       AND OLD.raw_json->'procurement' IS DISTINCT FROM 'null'::jsonb THEN
        IF TG_OP = 'DELETE' THEN
            RAISE EXCEPTION 'procurement authority cannot be deleted without audited archival'
                USING ERRCODE = '23514';
        END IF;
        IF NOT (NEW.raw_json ? 'procurement')
           OR NEW.raw_json->'procurement' = 'null'::jsonb THEN
            RAISE EXCEPTION 'procurement authority cannot be dropped by an older writer'
                USING ERRCODE = '23514';
        END IF;
        FOREACH collection_name IN ARRAY ARRAY['orders','locks','credits','outbox','deliveries','chain_inbox','absence_inbox','requests','quota_quotes','quota_purchases'] LOOP
            FOR entry_key IN SELECT jsonb_object_keys(COALESCE(OLD.raw_json->'procurement'->collection_name,'{}'::jsonb)) LOOP
                IF NOT (COALESCE(NEW.raw_json->'procurement'->collection_name,'{}'::jsonb) ? entry_key) THEN
                    RAISE EXCEPTION 'procurement authority history cannot be erased: %/%',collection_name,entry_key
                        USING ERRCODE = '23514';
                END IF;
            END LOOP;
        END LOOP;
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS accounts_procurement_authority_guard ON accounts;
CREATE TRIGGER accounts_procurement_authority_guard
BEFORE UPDATE OF raw_json OR DELETE ON accounts
FOR EACH ROW EXECUTE FUNCTION mir2_guard_procurement_authority();

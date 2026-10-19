-- Executed after SET LOCAL ROLE to a fresh NOLOGIN role with only two SELECT
-- grants. Each attempted write or base-table read MUST fail with 42501.
DO $$
DECLARE rejected boolean;
BEGIN
    rejected:=false;
    BEGIN
        PERFORM count(*) FROM hunting_economy_wallets;
    EXCEPTION WHEN insufficient_privilege THEN rejected:=true; END;
    IF NOT rejected THEN RAISE EXCEPTION 'readonly observer could read private base wallets'; END IF;

    rejected:=false;
    BEGIN
        UPDATE hunting_economy_wallets SET bound_gold=bound_gold
        WHERE realm_id LIKE current_setting('hunting_observer_test.prefix')||'%';
    EXCEPTION WHEN insufficient_privilege THEN rejected:=true; END;
    IF NOT rejected THEN RAISE EXCEPTION 'readonly observer could UPDATE wallets'; END IF;

    rejected:=false;
    BEGIN
        DELETE FROM hunting_economy_wallets
        WHERE realm_id LIKE current_setting('hunting_observer_test.prefix')||'%';
    EXCEPTION WHEN insufficient_privilege THEN rejected:=true; END;
    IF NOT rejected THEN RAISE EXCEPTION 'readonly observer could DELETE wallets'; END IF;

    rejected:=false;
    BEGIN
        INSERT INTO hunting_economy_daily_metric_versions(realm_id,business_day,metric_revision,metric_source,created_at_ms)
        VALUES(current_setting('hunting_observer_test.prefix')||'-steady_complete',0,90000,'should-never-write',0);
    EXCEPTION WHEN insufficient_privilege THEN rejected:=true; END;
    IF NOT rejected THEN RAISE EXCEPTION 'readonly observer could INSERT observation metrics'; END IF;
END $$;

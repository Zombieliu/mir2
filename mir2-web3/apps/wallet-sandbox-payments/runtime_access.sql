-- Run separately as the game database administrator after migration.sql.
-- The application role cannot migrate, read ordinary players or update any
-- public game table. Parent deployment may enable LOGIN and set a private
-- password only after reviewing/granting this fixed role.
DO $role$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM pg_catalog.pg_roles WHERE rolname='mir2_wallet_sandbox') THEN
        CREATE ROLE mir2_wallet_sandbox NOLOGIN;
    END IF;
END
$role$;
ALTER ROLE mir2_wallet_sandbox NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE
    NOREPLICATION NOBYPASSRLS NOINHERIT;

CREATE SCHEMA IF NOT EXISTS dubhe_wallet_sandbox;
REVOKE ALL ON SCHEMA dubhe_wallet_sandbox FROM PUBLIC;
GRANT USAGE ON SCHEMA dubhe_wallet_sandbox, public TO mir2_wallet_sandbox;

CREATE OR REPLACE VIEW dubhe_wallet_sandbox.accounts
WITH (security_barrier=true) AS
SELECT a.* FROM public.accounts a
WHERE a.account_id ~ '^dq[0-9a-f]{14}$'
  AND EXISTS(SELECT 1 FROM public.dubhe_sandbox_wallets w WHERE w.account_id=a.account_id)
WITH LOCAL CHECK OPTION;

CREATE OR REPLACE VIEW dubhe_wallet_sandbox.character_saves
WITH (security_barrier=true) AS
SELECT s.* FROM public.character_saves s
WHERE s.account_id ~ '^dq[0-9a-f]{14}$'
  AND EXISTS(SELECT 1 FROM public.dubhe_sandbox_wallets w
             WHERE w.account_id=s.account_id
               AND (w.character_index=s.character_index OR w.character_index IS NULL))
WITH LOCAL CHECK OPTION;

CREATE OR REPLACE VIEW dubhe_wallet_sandbox.character_state
WITH (security_barrier=true) AS
SELECT s.* FROM public.character_state s
WHERE s.account_id ~ '^dq[0-9a-f]{14}$'
  AND EXISTS(SELECT 1 FROM public.dubhe_sandbox_wallets w
             WHERE w.account_id=s.account_id
               AND (w.character_index=s.character_index OR w.character_index IS NULL))
WITH LOCAL CHECK OPTION;

CREATE OR REPLACE VIEW dubhe_wallet_sandbox.gate14_session_leases
WITH (security_barrier=true) AS
SELECT s.* FROM public.gate14_session_leases s
WHERE s.account_id ~ '^dq[0-9a-f]{14}$'
  AND EXISTS(SELECT 1 FROM public.dubhe_sandbox_wallets w WHERE w.account_id=s.account_id);

REVOKE ALL ON public.accounts,public.character_saves,public.character_state,
    public.gate14_session_leases FROM mir2_wallet_sandbox;
REVOKE ALL ON ALL TABLES IN SCHEMA dubhe_wallet_sandbox FROM mir2_wallet_sandbox;
GRANT SELECT ON dubhe_wallet_sandbox.accounts,dubhe_wallet_sandbox.character_saves,
    dubhe_wallet_sandbox.character_state,dubhe_wallet_sandbox.gate14_session_leases
    TO mir2_wallet_sandbox;
GRANT UPDATE(raw_json,store_version,updated_at,is_banned,ban_reason,ban_until_ms,banned_at_ms)
    ON dubhe_wallet_sandbox.accounts TO mir2_wallet_sandbox;
GRANT UPDATE(credit,snapshot_json,save_version,updated_at)
    ON dubhe_wallet_sandbox.character_saves TO mir2_wallet_sandbox;
GRANT UPDATE(credit,save_version,updated_at_ms,updated_at)
    ON dubhe_wallet_sandbox.character_state TO mir2_wallet_sandbox;

GRANT SELECT,INSERT ON public.dubhe_sandbox_wallets,public.dubhe_sandbox_payment_orders,
    public.dubhe_sandbox_payment_receipts TO mir2_wallet_sandbox;
GRANT UPDATE(state,character_index,ready_at) ON public.dubhe_sandbox_wallets TO mir2_wallet_sandbox;
-- FOR UPDATE requires UPDATE on at least one column; application never changes
-- prepared-order business fields after creation.
GRANT UPDATE(merchant_order_id) ON public.dubhe_sandbox_payment_orders TO mir2_wallet_sandbox;
GRANT EXECUTE ON FUNCTION public.dubhe_sandbox_candidate_exists(TEXT) TO mir2_wallet_sandbox;
ALTER ROLE mir2_wallet_sandbox SET search_path TO dubhe_wallet_sandbox, public;

-- REVOKE cannot override inherited PUBLIC privileges. Refuse installation if
-- existing database policy would let this role bypass the restricted views.
DO $permissions$
DECLARE relation TEXT;
BEGIN
    IF pg_catalog.has_schema_privilege('mir2_wallet_sandbox', 'public', 'CREATE')
       OR pg_catalog.has_schema_privilege('mir2_wallet_sandbox', 'dubhe_wallet_sandbox', 'CREATE') THEN
        RAISE EXCEPTION 'sandbox runtime inherits schema CREATE privileges; review database policy';
    END IF;
    FOREACH relation IN ARRAY ARRAY['accounts','character_saves','character_state','gate14_session_leases'] LOOP
        IF pg_catalog.has_any_column_privilege('mir2_wallet_sandbox', 'public.' || relation, 'SELECT')
           OR pg_catalog.has_any_column_privilege('mir2_wallet_sandbox', 'public.' || relation, 'UPDATE') THEN
            RAISE EXCEPTION 'sandbox runtime inherits direct game-table privileges; review database policy';
        END IF;
    END LOOP;
END
$permissions$;

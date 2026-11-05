-- Additive sandbox-only tables. Never seed or change an existing game account.
CREATE TABLE IF NOT EXISTS public.dubhe_sandbox_wallets (
    owner_address TEXT PRIMARY KEY CHECK (owner_address ~ '^0x[0-9a-f]{64}$'),
    account_id TEXT NOT NULL UNIQUE CHECK (account_id ~ '^dq[0-9a-f]{14}$'),
    character_name TEXT NOT NULL UNIQUE CHECK (character_name ~ '^Dq[0-9a-f]{10}$'),
    character_index INTEGER CHECK (character_index >= 0),
    state TEXT NOT NULL DEFAULT 'provisioning' CHECK (state IN ('provisioning','ready')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ready_at TIMESTAMPTZ,
    CHECK (state != 'ready' OR character_index IS NOT NULL)
);
CREATE TABLE IF NOT EXISTS public.dubhe_sandbox_payment_orders (
    merchant_order_id TEXT PRIMARY KEY CHECK (merchant_order_id ~ '^numqa_[0-9a-f]{32}$'),
    request_id TEXT NOT NULL UNIQUE CHECK (request_id ~ '^[0-9a-f]{64}$'),
    owner_address TEXT NOT NULL REFERENCES public.dubhe_sandbox_wallets(owner_address),
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    server_id TEXT NOT NULL,
    network TEXT NOT NULL CHECK (network IN ('mainnet','testnet')),
    sku TEXT NOT NULL CHECK (sku = 'numeron-test-100'),
    amount INTEGER NOT NULL CHECK (amount = 100),
    currency TEXT NOT NULL CHECK (currency = 'usd'),
    credits INTEGER NOT NULL CHECK (credits = 100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS public.dubhe_sandbox_payment_receipts (
    order_id TEXT PRIMARY KEY CHECK (order_id ~ '^ord_[0-9a-f]{32}$'),
    merchant_order_id TEXT NOT NULL UNIQUE REFERENCES public.dubhe_sandbox_payment_orders(merchant_order_id),
    payment_intent_id TEXT NOT NULL UNIQUE CHECK (payment_intent_id ~ '^pi_[A-Za-z0-9_]+$'),
    payload_hash TEXT NOT NULL CHECK (payload_hash ~ '^[0-9a-f]{64}$'),
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    credits INTEGER NOT NULL CHECK (credits = 100),
    balance_before BIGINT NOT NULL,
    balance_after BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (balance_after = balance_before + credits)
);

-- Disclose only existence, never an unowned account's JSON or credentials.
-- Fixed pg_catalog lookup and qualified relations avoid caller search_path.
CREATE OR REPLACE FUNCTION public.dubhe_sandbox_candidate_exists(candidate TEXT)
RETURNS BOOLEAN
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = pg_catalog
AS $function$
BEGIN
    IF candidate IS NULL OR candidate !~ '^dq[0-9a-f]{14}$' THEN
        RAISE EXCEPTION USING ERRCODE = '22023', MESSAGE = 'invalid sandbox candidate';
    END IF;
    RETURN EXISTS(SELECT 1 FROM public.accounts a WHERE a.account_id = candidate);
END
$function$;
REVOKE ALL ON FUNCTION public.dubhe_sandbox_candidate_exists(TEXT) FROM PUBLIC;

-- Testnet native-USDC records. Never reuse Stripe order/payment identifiers.
CREATE TABLE IF NOT EXISTS public.dubhe_sandbox_chain_orders (
    merchant_ref TEXT PRIMARY KEY CHECK (merchant_ref ~ '^numchain_[0-9a-f]{32}$'),
    request_id TEXT UNIQUE NOT NULL CHECK (request_id ~ '^[0-9a-f]{64}$'),
    payer TEXT NOT NULL REFERENCES public.dubhe_sandbox_wallets(owner_address),
    account_id TEXT NOT NULL CHECK (account_id ~ '^dq[0-9a-f]{14}$'),
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    server_id TEXT NOT NULL,
    registry TEXT NOT NULL CHECK (registry ~ '^0x[0-9a-f]{64}$'),
    receiver TEXT NOT NULL CHECK (receiver ~ '^0x[0-9a-f]{64}$'),
    wallet_nonce UUID UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS public.dubhe_sandbox_chain_receipts (
    merchant_ref TEXT PRIMARY KEY REFERENCES public.dubhe_sandbox_chain_orders(merchant_ref),
    wallet_nonce UUID UNIQUE NOT NULL,
    sui_digest TEXT UNIQUE NOT NULL CHECK (sui_digest ~ '^[1-9A-HJ-NP-Za-km-z]{32,44}$'),
    grant_id TEXT UNIQUE NOT NULL CHECK (grant_id ~ '^qagrant_[0-9a-f]{32}$'),
    checkpoint NUMERIC(20,0) NOT NULL CHECK (checkpoint >= 0),
    balance_before BIGINT NOT NULL,
    balance_after BIGINT NOT NULL CHECK (balance_after = balance_before + 100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
REVOKE ALL ON public.dubhe_sandbox_chain_orders, public.dubhe_sandbox_chain_receipts FROM PUBLIC;
GRANT SELECT,INSERT ON public.dubhe_sandbox_chain_orders, public.dubhe_sandbox_chain_receipts TO mir2_wallet_sandbox;
GRANT UPDATE(wallet_nonce) ON public.dubhe_sandbox_chain_orders TO mir2_wallet_sandbox;

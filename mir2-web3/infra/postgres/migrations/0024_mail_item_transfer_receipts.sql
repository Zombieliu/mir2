-- Server-source evidence for exact player-mail withdrawals. These receipts
-- are not a global current-holder registry and do not enable PG mail claims.
-- No backfill: names, cached saves and legacy delivery text are not provenance.
CREATE TABLE IF NOT EXISTS public.mail_item_transfer_receipts (
    source_namespace TEXT NOT NULL DEFAULT current_schema() CHECK (length(source_namespace)>0),
    delivery_nonce TEXT NOT NULL CHECK (length(delivery_nonce) BETWEEN 1 AND 1024),
    recipient_account_id TEXT NOT NULL CHECK (length(recipient_account_id)>0),
    recipient_character_index INTEGER NOT NULL,
    binding JSONB NOT NULL CHECK (jsonb_typeof(binding)='object'),
    binding_digest TEXT NOT NULL CHECK (binding_digest ~ '^[0-9a-f]{64}$'),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(source_namespace,delivery_nonce)
);
CREATE INDEX IF NOT EXISTS mail_item_transfer_receipts_recipient
    ON public.mail_item_transfer_receipts(source_namespace,recipient_account_id,recipient_character_index,delivery_nonce);

CREATE OR REPLACE FUNCTION public.protect_mail_item_transfer_receipt()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'immutable mail item transfer receipt';
END;
$$;
DROP TRIGGER IF EXISTS mail_item_transfer_receipt_immutable ON public.mail_item_transfer_receipts;
CREATE TRIGGER mail_item_transfer_receipt_immutable
    BEFORE UPDATE OR DELETE ON public.mail_item_transfer_receipts
    FOR EACH ROW EXECUTE FUNCTION public.protect_mail_item_transfer_receipt();

-- Rollback: stop new source transfers and retain this additive table/history.
-- Do not delete receipts when a mail, character or account is deleted, and do
-- not downgrade to a writer that does not enforce receipt-bound mail payloads.

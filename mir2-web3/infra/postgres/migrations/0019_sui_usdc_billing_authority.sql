-- V13 native USDC recharge. The registries are written by the account trigger
-- in the SAME transaction as PaidPending / Applied and the authoritative save.
-- They are authority fences, not an eventually consistent indexer projection.
-- Rollback: disable NEW checkout; keep V13 verifier/owner settlement and these
-- guards. Never downgrade or drop history while payments can settle late.
-- One prepared but unconfirmed sponsor creation per network/config. A worker
-- lease expiring is not proof that the signed transaction cannot still land.
CREATE TABLE IF NOT EXISTS sui_usdc_active_creation (
    chain_identifier TEXT NOT NULL,
    config_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    PRIMARY KEY (chain_identifier, config_id)
);
CREATE TABLE IF NOT EXISTS sui_usdc_invoice_receipts (
    chain_identifier TEXT NOT NULL,
    invoice_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    binding_json JSONB NOT NULL,
    PRIMARY KEY (chain_identifier, invoice_id),
    UNIQUE (account_id, order_id)
);
CREATE TABLE IF NOT EXISTS sui_usdc_payment_receipts (
    chain_identifier TEXT NOT NULL,
    transaction_digest TEXT NOT NULL,
    event_seq NUMERIC(20,0) NOT NULL CHECK (event_seq >= 0 AND event_seq <= 18446744073709551615),
    invoice_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    receipt_json JSONB NOT NULL,
    PRIMARY KEY (chain_identifier, transaction_digest, event_seq),
    UNIQUE (chain_identifier, invoice_id),
    UNIQUE (account_id, order_id)
);

CREATE OR REPLACE FUNCTION mir2_guard_sui_usdc_billing_authority()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    old_orders JSONB;
    new_orders JSONB;
    entry_key TEXT;
    old_order JSONB;
    new_order JSONB;
    invoice JSONB;
    payment JSONB;
    existing RECORD;
    authority_field TEXT;
BEGIN
    old_orders := '{}'::jsonb;
    IF TG_OP <> 'INSERT' THEN
        old_orders := COALESCE(OLD.raw_json->'billing'->'suiUsdcOrders','{}'::jsonb);
    END IF;
    IF TG_OP = 'DELETE' THEN
        IF old_orders <> '{}'::jsonb THEN
            RAISE EXCEPTION 'USDC recharge history requires audited archival' USING ERRCODE='23514';
        END IF;
        RETURN OLD;
    END IF;
    new_orders := COALESCE(NEW.raw_json->'billing'->'suiUsdcOrders','{}'::jsonb);
    IF jsonb_typeof(new_orders) <> 'object' THEN
        RAISE EXCEPTION 'USDC recharge ledger must be an object' USING ERRCODE='23514';
    END IF;
    FOR entry_key,old_order IN SELECT key,value FROM jsonb_each(old_orders) LOOP
        new_order := new_orders->entry_key;
        IF new_order IS NULL THEN
            RAISE EXCEPTION 'USDC recharge authority cannot be erased by older writer' USING ERRCODE='23514';
        END IF;
        IF (old_order - ARRAY['payer','state','preparedTx','invoice','paymentHint','receipt','applied','jobLease','reviewReason'])
           IS DISTINCT FROM
           (new_order - ARRAY['payer','state','preparedTx','invoice','paymentHint','receipt','applied','jobLease','reviewReason']) THEN
            RAISE EXCEPTION 'USDC frozen order terms are immutable' USING ERRCODE='23514';
        END IF;
        FOREACH authority_field IN ARRAY ARRAY['payer','preparedTx','invoice','receipt','applied'] LOOP
            IF old_order->authority_field IS NOT NULL AND old_order->authority_field <> 'null'::jsonb
               AND old_order->authority_field IS DISTINCT FROM new_order->authority_field THEN
                RAISE EXCEPTION 'USDC authority binding is immutable: %',authority_field USING ERRCODE='23514';
            END IF;
        END LOOP;
    END LOOP;
    FOR entry_key,new_order IN SELECT key,value FROM jsonb_each(new_orders) LOOP
        invoice := new_order->'invoice';
        payment := new_order->'receipt';
        IF new_order->'preparedTx' IS NOT NULL AND new_order->'preparedTx' <> 'null'::jsonb THEN
            IF (invoice IS NULL OR invoice = 'null'::jsonb)
               AND NOT (COALESCE(new_order->>'state','') = 'review' AND COALESCE(new_order->>'reviewReason','') = 'createFailed') THEN
                INSERT INTO sui_usdc_active_creation(chain_identifier,config_id,account_id,order_id)
                VALUES(new_order->'domain'->>'chainIdentifier',new_order->'domain'->>'configId',NEW.account_id,entry_key)
                ON CONFLICT DO NOTHING;
                SELECT * INTO STRICT existing FROM sui_usdc_active_creation
                    WHERE chain_identifier=new_order->'domain'->>'chainIdentifier' AND config_id=new_order->'domain'->>'configId';
                IF existing.account_id <> NEW.account_id OR existing.order_id <> entry_key THEN
                    RAISE EXCEPTION 'USDC sponsor creation is already active for this network/config' USING ERRCODE='23505';
                END IF;
            ELSE
                DELETE FROM sui_usdc_active_creation
                WHERE chain_identifier=new_order->'domain'->>'chainIdentifier' AND config_id=new_order->'domain'->>'configId'
                    AND account_id=NEW.account_id AND order_id=entry_key;
            END IF;
        END IF;
        IF invoice IS NOT NULL AND invoice <> 'null'::jsonb THEN
            IF invoice->>'createDigest' IS DISTINCT FROM new_order->'preparedTx'->>'digest'
               OR COALESCE((invoice->>'checkpoint')::NUMERIC,0) <= 0 THEN
                RAISE EXCEPTION 'USDC invoice requires persisted creation digest and final checkpoint' USING ERRCODE='23514';
            END IF;
            INSERT INTO sui_usdc_invoice_receipts(chain_identifier,invoice_id,account_id,order_id,binding_json)
            VALUES(new_order->'domain'->>'chainIdentifier',invoice->>'invoiceId',NEW.account_id,entry_key,invoice)
            ON CONFLICT DO NOTHING;
            SELECT * INTO STRICT existing FROM sui_usdc_invoice_receipts
                WHERE chain_identifier=new_order->'domain'->>'chainIdentifier' AND invoice_id=invoice->>'invoiceId';
            IF existing.account_id <> NEW.account_id OR existing.order_id <> entry_key OR existing.binding_json IS DISTINCT FROM invoice THEN
                RAISE EXCEPTION 'USDC invoice already bound to another order' USING ERRCODE='23505';
            END IF;
        END IF;
        IF payment IS NOT NULL AND payment <> 'null'::jsonb THEN
            IF invoice IS NULL OR invoice='null'::jsonb OR payment->>'invoiceId' IS DISTINCT FROM invoice->>'invoiceId'
               OR payment->>'orderId' IS DISTINCT FROM entry_key
               OR payment->>'chainIdentifier' IS DISTINCT FROM new_order->'domain'->>'chainIdentifier'
               OR payment->>'packageId' IS DISTINCT FROM new_order->'domain'->>'packageId'
               OR payment->>'configId' IS DISTINCT FROM new_order->'domain'->>'configId'
               OR payment->>'accountRef' IS DISTINCT FROM new_order->>'accountRef'
               OR payment->>'payer' IS DISTINCT FROM new_order->>'payer'
               OR payment->>'merchant' IS DISTINCT FROM new_order->'domain'->>'merchant'
               OR payment->>'coinType' IS DISTINCT FROM new_order->'domain'->>'coinType'
               OR payment->>'amountMicro' IS DISTINCT FROM new_order->>'amountMicro'
               OR COALESCE((payment->>'checkpoint')::NUMERIC,0) < (invoice->>'checkpoint')::NUMERIC
               OR COALESCE((payment->>'confirmedAtMs')::NUMERIC,0) < (new_order->>'createdAtMs')::NUMERIC
               OR COALESCE((payment->>'confirmedAtMs')::NUMERIC,0) > (new_order->>'expiresAtMs')::NUMERIC THEN
                RAISE EXCEPTION 'USDC payment does not match invoice/order' USING ERRCODE='23514';
            END IF;
            INSERT INTO sui_usdc_payment_receipts(chain_identifier,transaction_digest,event_seq,invoice_id,account_id,order_id,receipt_json)
            VALUES(payment->>'chainIdentifier',payment->>'digest',(payment->>'eventSeq')::NUMERIC,payment->>'invoiceId',NEW.account_id,entry_key,payment)
            ON CONFLICT DO NOTHING;
            SELECT * INTO STRICT existing FROM sui_usdc_payment_receipts
                WHERE chain_identifier=payment->>'chainIdentifier' AND transaction_digest=payment->>'digest' AND event_seq=(payment->>'eventSeq')::NUMERIC;
            IF existing.account_id <> NEW.account_id OR existing.order_id <> entry_key OR existing.receipt_json IS DISTINCT FROM payment THEN
                RAISE EXCEPTION 'USDC payment event already consumed by another order' USING ERRCODE='23505';
            END IF;
        END IF;
    END LOOP;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS accounts_sui_usdc_billing_authority_guard ON accounts;
CREATE TRIGGER accounts_sui_usdc_billing_authority_guard
BEFORE INSERT OR UPDATE OF raw_json OR DELETE ON accounts
FOR EACH ROW EXECUTE FUNCTION mir2_guard_sui_usdc_billing_authority();

-- Populate fences for a restored V13 source before allowing subsequent writers.
UPDATE accounts SET raw_json=raw_json
WHERE COALESCE(raw_json->'billing'->'suiUsdcOrders','{}'::jsonb) <> '{}'::jsonb;

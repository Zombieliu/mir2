-- V14 direct native-USDC transfers. No account JSON rewrite or legacy receipt migration.
-- A whole transaction digest can fund exactly one order across both modes.
-- Existing invoice/event registries and their JSON remain untouched.
CREATE TABLE IF NOT EXISTS sui_usdc_transaction_claims (
    chain_identifier TEXT NOT NULL,
    transaction_digest TEXT NOT NULL,
    account_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    payment_mode TEXT NOT NULL CHECK (payment_mode IN ('invoice','direct_transfer')),
    receipt_json JSONB NOT NULL,
    PRIMARY KEY (chain_identifier, transaction_digest),
    UNIQUE (account_id, order_id)
);

-- Refuse an ambiguous legacy transaction instead of arbitrarily assigning it.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM sui_usdc_payment_receipts
               GROUP BY chain_identifier,transaction_digest HAVING count(*) > 1) THEN
        RAISE EXCEPTION 'USDC legacy digest has multiple payment claims; audited reconciliation required' USING ERRCODE='23514';
    END IF;
END $$;
INSERT INTO sui_usdc_transaction_claims(chain_identifier,transaction_digest,account_id,order_id,payment_mode,receipt_json)
SELECT chain_identifier,transaction_digest,account_id,order_id,'invoice',receipt_json
FROM sui_usdc_payment_receipts ON CONFLICT DO NOTHING;
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM sui_usdc_payment_receipts p
        LEFT JOIN sui_usdc_transaction_claims c USING(chain_identifier,transaction_digest)
        WHERE c.account_id IS DISTINCT FROM p.account_id OR c.order_id IS DISTINCT FROM p.order_id
           OR c.payment_mode IS DISTINCT FROM 'invoice' OR c.receipt_json IS DISTINCT FROM p.receipt_json
    ) THEN
        RAISE EXCEPTION 'USDC legacy digest claim disagrees with persisted receipt' USING ERRCODE='23514';
    END IF;
END $$;

-- Retain bound payment windows permanently; a final receipt closes the active
-- reservation irreversibly. Receipt digest claims remain permanent as well.
CREATE TABLE IF NOT EXISTS sui_usdc_payer_windows (
    account_id TEXT NOT NULL,
    order_id TEXT NOT NULL,
    chain_identifier TEXT NOT NULL,
    payer TEXT NOT NULL,
    merchant TEXT NOT NULL,
    coin_type TEXT NOT NULL,
    amount_micro NUMERIC(20,0) NOT NULL CHECK (amount_micro > 0 AND amount_micro <= 18446744073709551615),
    starts_at_ms NUMERIC(20,0) NOT NULL CHECK (starts_at_ms > 0),
    ends_at_ms NUMERIC(20,0) NOT NULL CHECK (ends_at_ms > starts_at_ms AND ends_at_ms <= 18446744073709551615),
    payment_mode TEXT NOT NULL CHECK (payment_mode IN ('invoice','direct_transfer')),
    active BOOLEAN NOT NULL,
    PRIMARY KEY (account_id, order_id)
);
CREATE INDEX IF NOT EXISTS sui_usdc_payer_windows_lookup ON sui_usdc_payer_windows(chain_identifier,payer,merchant,coin_type,amount_micro);
INSERT INTO sui_usdc_payer_windows(account_id,order_id,chain_identifier,payer,merchant,coin_type,amount_micro,starts_at_ms,ends_at_ms,payment_mode,active)
SELECT a.account_id,e.key,e.value->'domain'->>'chainIdentifier',e.value->>'payer',
       e.value->'domain'->>'merchant',e.value->'domain'->>'coinType',(e.value->>'amountMicro')::NUMERIC,
       (e.value->>'createdAtMs')::NUMERIC,(e.value->>'expiresAtMs')::NUMERIC,COALESCE(e.value->>'paymentMode','invoice'),
       (e.value->'receipt' IS NULL OR e.value->'receipt'='null'::jsonb) AND (e.value->'transferReceipt' IS NULL OR e.value->'transferReceipt'='null'::jsonb)
FROM accounts a CROSS JOIN LATERAL jsonb_each(COALESCE(a.raw_json->'billing'->'suiUsdcOrders','{}'::jsonb)) e
WHERE e.value->>'payer' IS NOT NULL ON CONFLICT DO NOTHING;

CREATE OR REPLACE FUNCTION mir2_claim_sui_usdc_transaction(chain TEXT, tx_digest TEXT, account TEXT, order_key TEXT, mode TEXT, receipt JSONB)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE existing RECORD;
BEGIN
    INSERT INTO sui_usdc_transaction_claims(chain_identifier,transaction_digest,account_id,order_id,payment_mode,receipt_json)
    VALUES(chain,tx_digest,account,order_key,mode,receipt) ON CONFLICT DO NOTHING;
    SELECT * INTO STRICT existing FROM sui_usdc_transaction_claims
        WHERE chain_identifier=chain AND transaction_digest=tx_digest;
    IF existing.account_id <> account OR existing.order_id <> order_key OR existing.payment_mode <> mode
       OR existing.receipt_json IS DISTINCT FROM receipt THEN
        RAISE EXCEPTION 'USDC transaction already consumed by another order or mode' USING ERRCODE='23505';
    END IF;
END $$;

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
    transfer JSONB;
    existing RECORD;
    authority_field TEXT;
    old_mode TEXT;
    new_mode TEXT;
    lock_key TEXT;
    is_active BOOLEAN;
    was_registered BOOLEAN;
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
        IF (old_order - ARRAY['payer','state','preparedTx','invoice','paymentHint','receipt','applied','jobLease','reviewReason','paymentMode','transferReceipt','directScanCursor','directScanComplete'])
           IS DISTINCT FROM
           (new_order - ARRAY['payer','state','preparedTx','invoice','paymentHint','receipt','applied','jobLease','reviewReason','paymentMode','transferReceipt','directScanCursor','directScanComplete']) THEN
            RAISE EXCEPTION 'USDC frozen order terms are immutable' USING ERRCODE='23514';
        END IF;
        old_mode := COALESCE(old_order->>'paymentMode','invoice');
        new_mode := COALESCE(new_order->>'paymentMode','invoice');
        IF old_mode IS DISTINCT FROM new_mode THEN
            IF NOT (old_mode='invoice' AND new_mode='direct_transfer' AND old_order->>'state'='prepared'
                    AND (old_order->'payer' IS NULL OR old_order->'payer'='null'::jsonb)
                    AND (old_order->'preparedTx' IS NULL OR old_order->'preparedTx'='null'::jsonb)
                    AND (old_order->'invoice' IS NULL OR old_order->'invoice'='null'::jsonb)
                    AND (old_order->'paymentHint' IS NULL OR old_order->'paymentHint'='null'::jsonb)
                    AND (old_order->'receipt' IS NULL OR old_order->'receipt'='null'::jsonb)
                    AND (old_order->'transferReceipt' IS NULL OR old_order->'transferReceipt'='null'::jsonb)
                    AND (old_order->'applied' IS NULL OR old_order->'applied'='null'::jsonb)
                    AND (old_order->'jobLease' IS NULL OR old_order->'jobLease'='null'::jsonb)
                    AND (old_order->'reviewReason' IS NULL OR old_order->'reviewReason'='null'::jsonb)
                    AND new_order->>'state' IN ('payerBound','paidPending')
                    AND COALESCE(new_order->>'payer','') ~ '^0x[0-9a-f]{64}$') THEN
                RAISE EXCEPTION 'USDC payment mode is immutable after binding' USING ERRCODE='23514';
            END IF;
        END IF;
        FOREACH authority_field IN ARRAY ARRAY['payer','preparedTx','invoice','receipt','applied','transferReceipt'] LOOP
            IF old_order->authority_field IS NOT NULL AND old_order->authority_field <> 'null'::jsonb
               AND old_order->authority_field IS DISTINCT FROM new_order->authority_field THEN
                RAISE EXCEPTION 'USDC authority binding is immutable: %',authority_field USING ERRCODE='23514';
            END IF;
        END LOOP;
        IF (old_order->'directScanCursor' IS NOT NULL AND old_order->'directScanCursor'<>'null'::jsonb
            AND (new_order->'directScanCursor' IS NULL OR new_order->'directScanCursor'='null'::jsonb))
           OR (old_order->'directScanComplete'='true'::jsonb AND new_order->'directScanComplete' IS DISTINCT FROM 'true'::jsonb) THEN
            RAISE EXCEPTION 'USDC direct scan authority cannot be erased by older writer' USING ERRCODE='23514';
        END IF;
    END LOOP;
    -- Take tuple locks in a stable order before reading/inserting the registry.
    -- A SQL trigger must serialize independent accounts, not merely one process.
    FOR lock_key IN
        SELECT DISTINCT jsonb_build_array(value->'domain'->>'chainIdentifier',value->>'payer',
            value->'domain'->>'merchant',value->'domain'->>'coinType')::TEXT
        FROM jsonb_each(new_orders) WHERE value->>'payer' IS NOT NULL ORDER BY 1
    LOOP
        PERFORM pg_advisory_xact_lock(hashtextextended(lock_key,0));
    END LOOP;
    FOR entry_key,new_order IN SELECT key,value FROM jsonb_each(new_orders) LOOP
        invoice := new_order->'invoice';
        payment := new_order->'receipt';
        transfer := new_order->'transferReceipt';
        new_mode := COALESCE(new_order->>'paymentMode','invoice');
        IF (new_order ? 'paymentMode' AND (jsonb_typeof(new_order->'paymentMode') <> 'string'
                OR new_mode NOT IN ('invoice','direct_transfer')))
           OR (new_mode='invoice' AND transfer IS NOT NULL AND transfer <> 'null'::jsonb) THEN
            RAISE EXCEPTION 'USDC payment mode or receipt type is invalid' USING ERRCODE='23514';
        END IF;
        IF (new_order->'directScanComplete' IS NOT NULL AND jsonb_typeof(new_order->'directScanComplete') <> 'boolean')
           OR (new_order->'directScanComplete'='true'::jsonb AND (new_order->'directScanCursor' IS NULL OR new_order->'directScanCursor'='null'::jsonb))
           OR (new_mode='invoice' AND (new_order->'directScanCursor' IS NOT NULL AND new_order->'directScanCursor'<>'null'::jsonb
                                      OR new_order->'directScanComplete'='true'::jsonb)) THEN
            RAISE EXCEPTION 'USDC direct scan progress is invalid for this mode' USING ERRCODE='23514';
        END IF;
        IF new_order->'directScanCursor' IS NOT NULL AND new_order->'directScanCursor'<>'null'::jsonb THEN
            IF jsonb_typeof(new_order->'directScanCursor') <> 'string'
               OR octet_length(new_order->>'directScanCursor') NOT BETWEEN 2 AND 4096
               OR jsonb_typeof((new_order->>'directScanCursor')::jsonb) <> 'object' THEN
                RAISE EXCEPTION 'USDC direct scan cursor must be a bounded opaque JSON object' USING ERRCODE='23514';
            END IF;
        END IF;
        IF new_mode='direct_transfer' AND (old_orders->entry_key->>'payer') IS NULL THEN
            PERFORM 1 FROM identity_credentials
                WHERE account_id=NEW.account_id AND credential_kind='sui_wallet'
                    AND credential_subject=new_order->>'payer' AND revoked_at_ms IS NULL FOR SHARE;
            IF NOT FOUND THEN
                RAISE EXCEPTION 'USDC direct payer requires a live account wallet credential' USING ERRCODE='23514';
            END IF;
        END IF;
        IF new_order->>'payer' IS NOT NULL THEN
            is_active := (payment IS NULL OR payment='null'::jsonb) AND (transfer IS NULL OR transfer='null'::jsonb);
            SELECT EXISTS(SELECT 1 FROM sui_usdc_payer_windows WHERE account_id=NEW.account_id AND order_id=entry_key) INTO was_registered;
            INSERT INTO sui_usdc_payer_windows(account_id,order_id,chain_identifier,payer,merchant,coin_type,amount_micro,starts_at_ms,ends_at_ms,payment_mode,active)
            VALUES(NEW.account_id,entry_key,new_order->'domain'->>'chainIdentifier',new_order->>'payer',
                new_order->'domain'->>'merchant',new_order->'domain'->>'coinType',(new_order->>'amountMicro')::NUMERIC,
                (new_order->>'createdAtMs')::NUMERIC,(new_order->>'expiresAtMs')::NUMERIC,new_mode,is_active)
            ON CONFLICT DO NOTHING;
            IF NOT is_active THEN
                UPDATE sui_usdc_payer_windows SET active=FALSE WHERE account_id=NEW.account_id AND order_id=entry_key AND active;
            END IF;
            SELECT * INTO STRICT existing FROM sui_usdc_payer_windows WHERE account_id=NEW.account_id AND order_id=entry_key;
            IF existing.chain_identifier IS DISTINCT FROM new_order->'domain'->>'chainIdentifier'
               OR existing.payer IS DISTINCT FROM new_order->>'payer'
               OR existing.merchant IS DISTINCT FROM new_order->'domain'->>'merchant'
               OR existing.coin_type IS DISTINCT FROM new_order->'domain'->>'coinType'
               OR existing.amount_micro IS DISTINCT FROM (new_order->>'amountMicro')::NUMERIC
               OR existing.starts_at_ms IS DISTINCT FROM (new_order->>'createdAtMs')::NUMERIC
               OR existing.ends_at_ms IS DISTINCT FROM (new_order->>'expiresAtMs')::NUMERIC
               OR existing.payment_mode IS DISTINCT FROM new_mode OR existing.active IS DISTINCT FROM is_active THEN
                RAISE EXCEPTION 'USDC payer window binding is immutable' USING ERRCODE='23514';
            END IF;
            IF EXISTS (SELECT 1 FROM sui_usdc_payer_windows w
                WHERE w.chain_identifier=existing.chain_identifier AND w.payer=existing.payer AND w.merchant=existing.merchant
                  AND w.coin_type=existing.coin_type AND w.amount_micro=existing.amount_micro
                  AND (w.account_id,w.order_id) <> (NEW.account_id,entry_key)
                  AND (existing.active OR NOT was_registered) AND w.active AND (new_mode='direct_transfer' OR w.payment_mode='direct_transfer')
                  AND w.starts_at_ms < existing.ends_at_ms AND existing.starts_at_ms < w.ends_at_ms) THEN
                RAISE EXCEPTION 'USDC payer payment windows overlap across orders' USING ERRCODE='23505';
            END IF;
        END IF;
        IF new_mode='direct_transfer' THEN
            IF new_order->>'id' IS DISTINCT FROM entry_key
               OR (new_order->'preparedTx' IS NOT NULL AND new_order->'preparedTx' <> 'null'::jsonb)
               OR (invoice IS NOT NULL AND invoice <> 'null'::jsonb)
               OR (payment IS NOT NULL AND payment <> 'null'::jsonb)
               OR COALESCE(new_order->>'state','') NOT IN ('payerBound','paidPending','applied','review') THEN
                RAISE EXCEPTION 'USDC direct transfer cannot contain invoice authority' USING ERRCODE='23514';
            END IF;
            IF new_order->>'payer' IS NOT NULL AND
               (new_order->>'payer' !~ '^0x[0-9a-f]{64}$' OR new_order->>'payer'='0x'||repeat('0',64)
                OR new_order->>'payer'=new_order->'domain'->>'merchant') THEN
                RAISE EXCEPTION 'USDC direct payer is invalid' USING ERRCODE='23514';
            END IF;
            IF transfer IS NOT NULL AND transfer <> 'null'::jsonb THEN
                IF jsonb_typeof(transfer) <> 'object'
                   OR transfer - ARRAY['chainIdentifier','genesisDigest','orderId','accountRef','payer','merchant','coinType','amountMicro','frozenHash','digest','checkpoint','confirmedAtMs','observedAtMs'] <> '{}'::jsonb
                   OR NOT (transfer ?& ARRAY['chainIdentifier','genesisDigest','orderId','accountRef','payer','merchant','coinType','amountMicro','frozenHash','digest','checkpoint','confirmedAtMs','observedAtMs'])
                   OR EXISTS (SELECT 1 FROM jsonb_each(transfer) f WHERE f.key IN ('chainIdentifier','genesisDigest','orderId','accountRef','payer','merchant','coinType','frozenHash','digest') AND jsonb_typeof(f.value)<>'string')
                   OR EXISTS (SELECT 1 FROM jsonb_each(transfer) f WHERE f.key IN ('amountMicro','checkpoint','confirmedAtMs','observedAtMs') AND (jsonb_typeof(f.value)<>'number' OR f.value::TEXT !~ '^[0-9]+$'))
                   OR transfer->>'chainIdentifier' IS DISTINCT FROM new_order->'domain'->>'chainIdentifier'
                   OR transfer->>'chainIdentifier' IS DISTINCT FROM '4c78adac'
                   OR transfer->>'genesisDigest' IS DISTINCT FROM '69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD'
                   OR transfer->>'orderId' IS DISTINCT FROM entry_key
                   OR transfer->>'accountRef' IS DISTINCT FROM new_order->>'accountRef'
                   OR transfer->>'payer' IS DISTINCT FROM new_order->>'payer'
                   OR transfer->>'merchant' IS DISTINCT FROM new_order->'domain'->>'merchant'
                   OR transfer->>'coinType' IS DISTINCT FROM new_order->'domain'->>'coinType'
                   OR transfer->>'amountMicro' IS DISTINCT FROM new_order->>'amountMicro'
                   OR COALESCE(transfer->>'frozenHash','') !~ '^0x[0-9a-f]{64}$'
                   OR COALESCE(transfer->>'digest','') !~ '^[1-9A-HJ-NP-Za-km-z]{32,64}$'
                   OR COALESCE((transfer->>'checkpoint')::NUMERIC,0) <= 0
                   OR (transfer->>'checkpoint')::NUMERIC > 18446744073709551615
                   OR COALESCE((transfer->>'confirmedAtMs')::NUMERIC,0) < (new_order->>'createdAtMs')::NUMERIC
                   OR COALESCE((transfer->>'confirmedAtMs')::NUMERIC,0) >= (new_order->>'expiresAtMs')::NUMERIC
                   OR COALESCE((transfer->>'observedAtMs')::NUMERIC,0) < (transfer->>'confirmedAtMs')::NUMERIC
                   OR (transfer->>'observedAtMs')::NUMERIC > 18446744073709551615 THEN
                    RAISE EXCEPTION 'USDC direct receipt does not match order' USING ERRCODE='23514';
                END IF;
                PERFORM mir2_claim_sui_usdc_transaction(transfer->>'chainIdentifier',transfer->>'digest',NEW.account_id,entry_key,new_mode,transfer);
            END IF;
            IF (new_order->>'state' IN ('payerBound','review') AND (new_order->>'payer' IS NULL OR transfer IS NOT NULL AND transfer <> 'null'::jsonb))
               OR (new_order->>'state' IN ('paidPending','applied') AND (new_order->>'payer' IS NULL OR transfer IS NULL OR transfer='null'::jsonb))
               OR (new_order->>'state'='applied' AND (new_order->'applied' IS NULL OR new_order->'applied'='null'::jsonb))
               OR (new_order->>'state'<>'applied' AND new_order->'applied' IS NOT NULL AND new_order->'applied'<>'null'::jsonb) THEN
                RAISE EXCEPTION 'USDC direct order state is invalid' USING ERRCODE='23514';
            END IF;
            IF (new_order->>'state'='review') IS DISTINCT FROM (new_order->>'reviewReason' IS NOT NULL) THEN
                RAISE EXCEPTION 'USDC direct review state is invalid' USING ERRCODE='23514';
            END IF;
            CONTINUE;
        END IF;
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
            PERFORM mir2_claim_sui_usdc_transaction(payment->>'chainIdentifier',payment->>'digest',NEW.account_id,entry_key,new_mode,payment);
        END IF;
    END LOOP;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS accounts_sui_usdc_billing_authority_guard ON accounts;
CREATE TRIGGER accounts_sui_usdc_billing_authority_guard
BEFORE INSERT OR UPDATE OF raw_json OR DELETE ON accounts
FOR EACH ROW EXECUTE FUNCTION mir2_guard_sui_usdc_billing_authority();


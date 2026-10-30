-- Durable Sui payment attempts. Never deletes account, character or ledger data.
CREATE TABLE IF NOT EXISTS promotion_payout_intents (
    environment TEXT NOT NULL,
    payout_id TEXT NOT NULL,
    intent_sha256 TEXT NOT NULL CHECK(intent_sha256 ~ '^[0-9a-f]{64}$'),
    intent_json JSONB NOT NULL,
    settlement_sha256 TEXT NOT NULL CHECK(settlement_sha256 ~ '^[0-9a-f]{64}$'),
    created_by TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    reviewed_by TEXT,
    reviewed_at_ms BIGINT,
    PRIMARY KEY(environment,payout_id),
    UNIQUE(environment,payout_id,intent_sha256),
    FOREIGN KEY(environment,payout_id) REFERENCES promotion_payouts(environment,payout_id) ON DELETE RESTRICT,
    CHECK(reviewed_by IS NULL OR reviewed_by<>created_by),
    CHECK((reviewed_by IS NULL)=(reviewed_at_ms IS NULL))
);
CREATE TABLE IF NOT EXISTS promotion_payout_attempts (
    environment TEXT NOT NULL,
    payout_id TEXT NOT NULL,
    intent_sha256 TEXT NOT NULL,
    network TEXT NOT NULL CHECK(network IN ('sui:testnet','sui:mainnet')),
    tx_digest TEXT NOT NULL,
    transaction_bytes_base64 TEXT NOT NULL CHECK(length(transaction_bytes_base64) BETWEEN 1 AND 65536),
    signatures_json JSONB NOT NULL CHECK(jsonb_typeof(signatures_json)='array' AND jsonb_array_length(signatures_json)=1),
    status TEXT NOT NULL CHECK(status IN ('reserved','submitted','unknown','confirmed','failed')),
    created_by TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL,
    submitted_at_ms BIGINT,
    updated_at_ms BIGINT NOT NULL,
    PRIMARY KEY(environment,payout_id),
    UNIQUE(network,tx_digest),
    FOREIGN KEY(environment,payout_id,intent_sha256) REFERENCES promotion_payout_intents(environment,payout_id,intent_sha256) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS promotion_payout_chain_observations (
    environment TEXT NOT NULL,
    payout_id TEXT NOT NULL,
    observation_id BIGSERIAL PRIMARY KEY,
    tx_digest TEXT NOT NULL,
    result_json JSONB NOT NULL,
    observed_at_ms BIGINT NOT NULL,
    FOREIGN KEY(environment,payout_id) REFERENCES promotion_payout_attempts(environment,payout_id) ON DELETE RESTRICT
);

-- Remove only the former unconditional "no execution adapter" gate. Retain the
-- status enum, required digest/reviewer, unique settlement and every other check.
DO $$ DECLARE c record; BEGIN
  FOR c IN SELECT conname FROM pg_constraint
    WHERE conrelid='promotion_payouts'::regclass AND contype='c'
      AND pg_get_constraintdef(oid) LIKE '%submitted%'
      AND pg_get_constraintdef(oid) LIKE '%unknown%'
      AND pg_get_constraintdef(oid) LIKE '%confirmed%'
      AND pg_get_constraintdef(oid) NOT LIKE '%draft%'
      AND pg_get_constraintdef(oid) NOT LIKE '%tx_digest%'
  LOOP EXECUTE format('ALTER TABLE promotion_payouts DROP CONSTRAINT %I',c.conname); END LOOP;
END $$;

CREATE OR REPLACE FUNCTION promotion_guard_payout_intent() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
  IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'promotionPayoutIntentImmutable'; END IF;
  IF OLD.environment IS DISTINCT FROM NEW.environment OR OLD.payout_id IS DISTINCT FROM NEW.payout_id
     OR OLD.intent_sha256 IS DISTINCT FROM NEW.intent_sha256 OR OLD.intent_json IS DISTINCT FROM NEW.intent_json
     OR OLD.settlement_sha256 IS DISTINCT FROM NEW.settlement_sha256 OR OLD.created_by IS DISTINCT FROM NEW.created_by
     OR OLD.created_at_ms IS DISTINCT FROM NEW.created_at_ms OR OLD.reviewed_by IS NOT NULL
     OR NEW.reviewed_by IS NULL OR NEW.reviewed_at_ms IS NULL THEN RAISE EXCEPTION 'promotionPayoutIntentImmutable'; END IF;
  RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS promotion_payout_intent_immutable ON promotion_payout_intents;
CREATE TRIGGER promotion_payout_intent_immutable BEFORE UPDATE OR DELETE ON promotion_payout_intents
  FOR EACH ROW EXECUTE FUNCTION promotion_guard_payout_intent();
DROP TRIGGER IF EXISTS promotion_payout_intent_no_truncate ON promotion_payout_intents;
CREATE TRIGGER promotion_payout_intent_no_truncate BEFORE TRUNCATE ON promotion_payout_intents
  FOR EACH STATEMENT EXECUTE FUNCTION promotion_guard_payout_intent();

CREATE OR REPLACE FUNCTION promotion_guard_payout_attempt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
  IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'promotionPayoutAttemptImmutable'; END IF;
  IF OLD.environment IS DISTINCT FROM NEW.environment OR OLD.payout_id IS DISTINCT FROM NEW.payout_id
     OR OLD.intent_sha256 IS DISTINCT FROM NEW.intent_sha256 OR OLD.network IS DISTINCT FROM NEW.network
     OR OLD.tx_digest IS DISTINCT FROM NEW.tx_digest OR OLD.transaction_bytes_base64 IS DISTINCT FROM NEW.transaction_bytes_base64
     OR OLD.signatures_json IS DISTINCT FROM NEW.signatures_json OR OLD.created_by IS DISTINCT FROM NEW.created_by
     OR OLD.created_at_ms IS DISTINCT FROM NEW.created_at_ms
     OR (OLD.status IN ('confirmed','failed') AND OLD.status IS DISTINCT FROM NEW.status)
     OR (NEW.status='reserved' AND OLD.status<>'reserved') THEN RAISE EXCEPTION 'promotionPayoutAttemptImmutable'; END IF;
  RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS promotion_payout_attempt_immutable ON promotion_payout_attempts;
CREATE TRIGGER promotion_payout_attempt_immutable BEFORE UPDATE OR DELETE ON promotion_payout_attempts
  FOR EACH ROW EXECUTE FUNCTION promotion_guard_payout_attempt();
DROP TRIGGER IF EXISTS promotion_payout_attempt_no_truncate ON promotion_payout_attempts;
CREATE TRIGGER promotion_payout_attempt_no_truncate BEFORE TRUNCATE ON promotion_payout_attempts
  FOR EACH STATEMENT EXECUTE FUNCTION promotion_guard_payout_attempt();
DROP TRIGGER IF EXISTS promotion_payout_observation_immutable ON promotion_payout_chain_observations;
CREATE TRIGGER promotion_payout_observation_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON promotion_payout_chain_observations
  FOR EACH STATEMENT EXECUTE FUNCTION promotion_forbid_ledger_mutation();

CREATE OR REPLACE FUNCTION promotion_guard_payout_transition() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
  IF EXISTS(SELECT 1 FROM promotion_payout_intents i WHERE i.environment=OLD.environment AND i.payout_id=OLD.payout_id)
     AND (OLD.environment IS DISTINCT FROM NEW.environment OR OLD.payout_id IS DISTINCT FROM NEW.payout_id
       OR OLD.settlement_id IS DISTINCT FROM NEW.settlement_id OR OLD.network IS DISTINCT FROM NEW.network
       OR OLD.coin_type IS DISTINCT FROM NEW.coin_type OR OLD.destination IS DISTINCT FROM NEW.destination
       OR OLD.address_version IS DISTINCT FROM NEW.address_version OR OLD.amount_micro_usd IS DISTINCT FROM NEW.amount_micro_usd
       OR OLD.created_by IS DISTINCT FROM NEW.created_by) THEN RAISE EXCEPTION 'promotionPayoutIntentImmutable'; END IF;
  IF NEW.status IN ('submitted','unknown','confirmed','failed') AND NOT EXISTS(
      SELECT 1 FROM promotion_payout_attempts a JOIN promotion_payout_intents i USING(environment,payout_id)
      WHERE a.environment=NEW.environment AND a.payout_id=NEW.payout_id AND a.tx_digest=NEW.tx_digest
        AND i.reviewed_by IS NOT NULL AND i.reviewed_by=NEW.reviewed_by
        AND (NEW.status='unknown' OR a.status=NEW.status)) THEN RAISE EXCEPTION 'promotionPayoutAttemptRequired'; END IF;
  IF OLD.status IN ('confirmed','failed') AND OLD.status IS DISTINCT FROM NEW.status THEN RAISE EXCEPTION 'promotionPayoutFinalStateImmutable'; END IF;
  RETURN NEW;
END $$;
DROP TRIGGER IF EXISTS promotion_payout_transition_guard ON promotion_payouts;
CREATE TRIGGER promotion_payout_transition_guard BEFORE UPDATE ON promotion_payouts
  FOR EACH ROW EXECUTE FUNCTION promotion_guard_payout_transition();

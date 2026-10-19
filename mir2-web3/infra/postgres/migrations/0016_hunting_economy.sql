-- New assets only. No UPDATE/trigger/backfill touches legacy gold or character saves.
CREATE TABLE IF NOT EXISTS hunting_economy_policies (
    realm_id TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    policy_digest TEXT NOT NULL,
    policy_json JSONB NOT NULL,
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms >= 0),
    PRIMARY KEY (realm_id, policy_version)
);

CREATE TABLE IF NOT EXISTS hunting_economy_pools (
    realm_id TEXT PRIMARY KEY,
    policy_version TEXT NOT NULL,
    stock_atoms BIGINT NOT NULL DEFAULT 0 CHECK (stock_atoms >= 0),
    mint_remainder BIGINT NOT NULL DEFAULT 0 CHECK (mint_remainder >= 0),
    decay_remainder BIGINT NOT NULL DEFAULT 0 CHECK (decay_remainder >= 0),
    genesis_day BIGINT NOT NULL,
    settled_through_day BIGINT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    updated_at_ms BIGINT NOT NULL CHECK (updated_at_ms >= 0),
    CHECK (settled_through_day >= genesis_day - 1),
    FOREIGN KEY (realm_id, policy_version)
        REFERENCES hunting_economy_policies(realm_id, policy_version)
);

CREATE TABLE IF NOT EXISTS hunting_economy_wallets (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    character_incarnation TEXT NOT NULL,
    bound_gold BIGINT NOT NULL DEFAULT 0 CHECK (bound_gold >= 0),
    unbound_gold BIGINT NOT NULL DEFAULT 0 CHECK (unbound_gold >= 0),
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    updated_at_ms BIGINT NOT NULL CHECK (updated_at_ms >= 0),
    PRIMARY KEY (realm_id, account_id, character_index, character_incarnation)
);

CREATE TABLE IF NOT EXISTS hunting_economy_operations (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    operation_key TEXT NOT NULL,
    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('conversion','bound_credit','burn')),
    request_digest TEXT NOT NULL,
    receipt_json JSONB NOT NULL,
    receipt_digest TEXT NOT NULL,
    business_day BIGINT NOT NULL,
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms >= 0),
    PRIMARY KEY (realm_id, operation_key)
);

CREATE TABLE IF NOT EXISTS hunting_economy_burn_events (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    event_id TEXT NOT NULL,
    operation_key TEXT NOT NULL,
    layer TEXT NOT NULL CHECK (layer IN ('daily','light_pvp','siege')),
    source_kind TEXT NOT NULL,
    source_key TEXT NOT NULL,
    gold_equivalent BIGINT NOT NULL CHECK (gold_equivalent > 0),
    valuation_version TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    business_day BIGINT NOT NULL,
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms >= 0),
    PRIMARY KEY (realm_id, event_id),
    UNIQUE (realm_id, source_kind, source_key),
    FOREIGN KEY (realm_id, operation_key)
        REFERENCES hunting_economy_operations(realm_id, operation_key)
        DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX IF NOT EXISTS hunting_economy_burn_day_idx
    ON hunting_economy_burn_events(realm_id, business_day, layer);

CREATE TABLE IF NOT EXISTS hunting_economy_conversions (
    realm_id TEXT NOT NULL,
    operation_key TEXT NOT NULL,
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    character_incarnation TEXT NOT NULL,
    gross_gold BIGINT NOT NULL CHECK (gross_gold > 0),
    fee_gold BIGINT NOT NULL CHECK (fee_gold > 0),
    net_gold BIGINT NOT NULL CHECK (net_gold >= 0),
    consumed_atoms BIGINT NOT NULL CHECK (consumed_atoms > 0),
    business_day BIGINT NOT NULL,
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms >= 0),
    PRIMARY KEY (realm_id, operation_key),
    CHECK (gross_gold = fee_gold + net_gold),
    CHECK (gross_gold = consumed_atoms),
    FOREIGN KEY (realm_id, operation_key)
        REFERENCES hunting_economy_operations(realm_id, operation_key)
);
CREATE INDEX IF NOT EXISTS hunting_economy_conversion_day_idx
    ON hunting_economy_conversions(realm_id, business_day);

CREATE TABLE IF NOT EXISTS hunting_economy_settlements (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    business_day BIGINT NOT NULL,
    policy_version TEXT NOT NULL,
    receipt_json JSONB NOT NULL,
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms >= 0),
    PRIMARY KEY (realm_id, business_day)
);

CREATE TABLE IF NOT EXISTS hunting_economy_outbox (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    event_id TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    payload_json JSONB NOT NULL,
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms >= 0),
    delivered_at_ms BIGINT,
    PRIMARY KEY (realm_id, event_id)
);
CREATE INDEX IF NOT EXISTS hunting_economy_outbox_pending_idx
    ON hunting_economy_outbox(realm_id, created_at_ms)
    WHERE delivered_at_ms IS NULL;

CREATE OR REPLACE FUNCTION hunting_economy_reject_history_mutation()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'hunting economy history is immutable';
END;
$$;

-- An operator creates a new policy version; past calculations retain their inputs.
DROP TRIGGER IF EXISTS hunting_economy_policy_immutable ON hunting_economy_policies;
CREATE TRIGGER hunting_economy_policy_immutable
    BEFORE UPDATE OR DELETE ON hunting_economy_policies
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_economy_operations_immutable ON hunting_economy_operations;
CREATE TRIGGER hunting_economy_operations_immutable
    BEFORE UPDATE OR DELETE ON hunting_economy_operations
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_economy_burn_immutable ON hunting_economy_burn_events;
CREATE TRIGGER hunting_economy_burn_immutable
    BEFORE UPDATE OR DELETE ON hunting_economy_burn_events
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_economy_conversion_immutable ON hunting_economy_conversions;
CREATE TRIGGER hunting_economy_conversion_immutable
    BEFORE UPDATE OR DELETE ON hunting_economy_conversions
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_economy_settlement_immutable ON hunting_economy_settlements;
CREATE TRIGGER hunting_economy_settlement_immutable
    BEFORE UPDATE OR DELETE ON hunting_economy_settlements
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();

CREATE OR REPLACE FUNCTION hunting_economy_guard_outbox_history()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'hunting economy outbox history is immutable';
    END IF;
    IF ROW(NEW.realm_id, NEW.event_id, NEW.event_kind, NEW.payload_json, NEW.created_at_ms)
        IS DISTINCT FROM
       ROW(OLD.realm_id, OLD.event_id, OLD.event_kind, OLD.payload_json, OLD.created_at_ms)
       OR (OLD.delivered_at_ms IS NOT NULL AND NEW.delivered_at_ms IS DISTINCT FROM OLD.delivered_at_ms)
       OR (NEW.delivered_at_ms IS NOT NULL AND NEW.delivered_at_ms < NEW.created_at_ms) THEN
        RAISE EXCEPTION 'only first hunting economy outbox acknowledgement may change';
    END IF;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS hunting_economy_outbox_history ON hunting_economy_outbox;
CREATE TRIGGER hunting_economy_outbox_history
    BEFORE UPDATE OR DELETE ON hunting_economy_outbox
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_guard_outbox_history();

-- A material conversion consumes a permit allowance and EXISTING unbound coins.
-- It does not issue coins, change legacy gold, or alter a character by itself.
-- The trusted main-server source callback stages its complete checkpoint in the
-- same transaction as these immutable records.
ALTER TABLE hunting_economy_operations
    DROP CONSTRAINT IF EXISTS hunting_economy_operations_operation_kind_check;
ALTER TABLE hunting_economy_operations
    ADD CONSTRAINT hunting_economy_operations_operation_kind_check
    CHECK (operation_kind IN ('conversion','bound_credit','burn','bound_debit_burn','zero_fee_source','material_conversion'));

CREATE TABLE IF NOT EXISTS hunting_material_valuations (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    valuation_version TEXT NOT NULL CHECK (length(btrim(valuation_version)) BETWEEN 1 AND 128),
    valuation_digest TEXT NOT NULL,
    config_json JSONB NOT NULL CHECK (jsonb_typeof(config_json) = 'object'),
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms >= 0),
    PRIMARY KEY (realm_id, valuation_version),
    UNIQUE (realm_id, valuation_version, valuation_digest)
);
CREATE TABLE IF NOT EXISTS hunting_material_active_valuations (
    realm_id TEXT PRIMARY KEY REFERENCES hunting_economy_pools(realm_id),
    valuation_version TEXT NOT NULL,
    valuation_digest TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    activated_at_ms BIGINT NOT NULL CHECK (activated_at_ms >= 0),
    FOREIGN KEY (realm_id, valuation_version, valuation_digest)
        REFERENCES hunting_material_valuations(realm_id, valuation_version, valuation_digest)
);

CREATE TABLE IF NOT EXISTS hunting_material_conversion_quotes (
    realm_id TEXT NOT NULL REFERENCES hunting_economy_pools(realm_id),
    quote_id TEXT NOT NULL CHECK (quote_id ~ '^[0-9a-f]{64}$' AND quote_id <> repeat('0',64)),
    request_digest TEXT NOT NULL,
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    character_incarnation TEXT NOT NULL,
    item_index INTEGER NOT NULL CHECK (item_index > 0),
    input_unique_id NUMERIC(20,0) NOT NULL CHECK (input_unique_id > 0 AND input_unique_id <= 18446744073709551615),
    input_count BIGINT NOT NULL CHECK (input_count > 0),
    quantity BIGINT NOT NULL CHECK (quantity > 0 AND quantity <= input_count),
    input_save_revision BIGINT NOT NULL CHECK (input_save_revision >= 0),
    input_item_digest TEXT NOT NULL,
    source_binding_digest TEXT NOT NULL,
    valuation_version TEXT NOT NULL,
    valuation_digest TEXT NOT NULL,
    valuation_revision BIGINT NOT NULL CHECK (valuation_revision > 0),
    policy_version TEXT NOT NULL,
    unit_gold BIGINT NOT NULL CHECK (unit_gold > 0),
    gold_equivalent BIGINT NOT NULL CHECK (gold_equivalent > 0),
    fee_gold BIGINT NOT NULL CHECK (fee_gold > 0 AND fee_gold <= gold_equivalent),
    consumed_atoms BIGINT NOT NULL CHECK (consumed_atoms = gold_equivalent),
    created_at_ms BIGINT NOT NULL CHECK (created_at_ms >= 0),
    expires_at_ms BIGINT NOT NULL CHECK (expires_at_ms > created_at_ms AND expires_at_ms - created_at_ms <= 30000),
    quote_json JSONB NOT NULL CHECK (jsonb_typeof(quote_json) = 'object'),
    quote_digest TEXT NOT NULL,
    PRIMARY KEY (realm_id, quote_id),
    CHECK (gold_equivalent::numeric = unit_gold::numeric * quantity::numeric),
    FOREIGN KEY (realm_id, policy_version) REFERENCES hunting_economy_policies(realm_id, policy_version),
    FOREIGN KEY (realm_id, valuation_version, valuation_digest)
        REFERENCES hunting_material_valuations(realm_id, valuation_version, valuation_digest),
    UNIQUE (realm_id, quote_id, policy_version, valuation_version, valuation_digest, item_index, quantity, gold_equivalent, fee_gold, consumed_atoms),
    UNIQUE (realm_id, quote_id, account_id, character_index, character_incarnation)
);

CREATE TABLE IF NOT EXISTS hunting_economy_material_conversions (
    realm_id TEXT NOT NULL,
    operation_key TEXT NOT NULL,
    quote_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL CHECK (character_index >= 0),
    character_incarnation TEXT NOT NULL,
    item_index INTEGER NOT NULL CHECK (item_index > 0),
    quantity BIGINT NOT NULL CHECK (quantity > 0),
    valuation_version TEXT NOT NULL,
    valuation_digest TEXT NOT NULL,
    gold_equivalent BIGINT NOT NULL CHECK (gold_equivalent > 0),
    fee_gold BIGINT NOT NULL CHECK (fee_gold > 0 AND fee_gold <= gold_equivalent),
    consumed_atoms BIGINT NOT NULL CHECK (consumed_atoms = gold_equivalent),
    policy_version TEXT NOT NULL,
    business_day BIGINT NOT NULL,
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms >= 0),
    PRIMARY KEY (realm_id, operation_key),
    UNIQUE (realm_id, quote_id),
    FOREIGN KEY (realm_id, operation_key) REFERENCES hunting_economy_operations(realm_id, operation_key)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (realm_id, quote_id, policy_version, valuation_version, valuation_digest, item_index, quantity, gold_equivalent, fee_gold, consumed_atoms)
        REFERENCES hunting_material_conversion_quotes(realm_id, quote_id, policy_version, valuation_version, valuation_digest, item_index, quantity, gold_equivalent, fee_gold, consumed_atoms),
    FOREIGN KEY (realm_id, quote_id, account_id, character_index, character_incarnation)
        REFERENCES hunting_material_conversion_quotes(realm_id, quote_id, account_id, character_index, character_incarnation)
);
CREATE INDEX IF NOT EXISTS hunting_material_conversion_day_idx
    ON hunting_economy_material_conversions(realm_id, business_day);

CREATE TABLE IF NOT EXISTS hunting_material_conversion_results (
    realm_id TEXT NOT NULL,
    quote_id TEXT NOT NULL,
    operation_key TEXT NOT NULL,
    receipt_json JSONB NOT NULL CHECK (jsonb_typeof(receipt_json) = 'object'),
    receipt_digest TEXT NOT NULL,
    before_revision BIGINT NOT NULL CHECK (before_revision >= 0),
    after_revision BIGINT NOT NULL CHECK (after_revision = before_revision + 1),
    committed_at_ms BIGINT NOT NULL CHECK (committed_at_ms >= 0),
    PRIMARY KEY (realm_id, quote_id),
    UNIQUE (realm_id, operation_key),
    FOREIGN KEY (realm_id, quote_id) REFERENCES hunting_material_conversion_quotes(realm_id, quote_id),
    FOREIGN KEY (realm_id, operation_key) REFERENCES hunting_economy_material_conversions(realm_id, operation_key)
        DEFERRABLE INITIALLY DEFERRED
);

DROP TRIGGER IF EXISTS hunting_material_valuation_immutable ON hunting_material_valuations;
CREATE TRIGGER hunting_material_valuation_immutable BEFORE UPDATE OR DELETE ON hunting_material_valuations
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_material_quote_immutable ON hunting_material_conversion_quotes;
CREATE TRIGGER hunting_material_quote_immutable BEFORE UPDATE OR DELETE ON hunting_material_conversion_quotes
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_material_conversion_immutable ON hunting_economy_material_conversions;
CREATE TRIGGER hunting_material_conversion_immutable BEFORE UPDATE OR DELETE ON hunting_economy_material_conversions
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();
DROP TRIGGER IF EXISTS hunting_material_result_immutable ON hunting_material_conversion_results;
CREATE TRIGGER hunting_material_result_immutable BEFORE UPDATE OR DELETE ON hunting_material_conversion_results
    FOR EACH ROW EXECUTE FUNCTION hunting_economy_reject_history_mutation();

CREATE OR REPLACE FUNCTION hunting_material_guard_activation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN RAISE EXCEPTION 'material activation history cannot be deleted'; END IF;
    IF NEW.realm_id IS DISTINCT FROM OLD.realm_id OR NEW.revision <> OLD.revision + 1 THEN
        RAISE EXCEPTION 'material activation must advance its realm revision';
    END IF;
    RETURN NEW;
END;
$$;
DROP TRIGGER IF EXISTS hunting_material_activation_guard ON hunting_material_active_valuations;
CREATE TRIGGER hunting_material_activation_guard BEFORE UPDATE OR DELETE ON hunting_material_active_valuations
    FOR EACH ROW EXECUTE FUNCTION hunting_material_guard_activation();

REVOKE ALL ON hunting_material_valuations,hunting_material_active_valuations,
    hunting_material_conversion_quotes,hunting_economy_material_conversions,
    hunting_material_conversion_results FROM PUBLIC;

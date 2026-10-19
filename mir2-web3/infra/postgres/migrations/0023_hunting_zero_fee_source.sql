-- A zero-fee real source still has an immutable operation and audit event.
-- Extend 0022 without modifying historical receipts, wallets or burn records.
ALTER TABLE hunting_economy_operations
    DROP CONSTRAINT IF EXISTS hunting_economy_operations_operation_kind_check;
ALTER TABLE hunting_economy_operations
    ADD CONSTRAINT hunting_economy_operations_operation_kind_check
    CHECK (operation_kind IN ('conversion','bound_credit','burn','bound_debit_burn','zero_fee_source'));

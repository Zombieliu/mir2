-- Extend the immutable operation ledger without rewriting historical migrations
-- or changing any legacy wallet. Reapplication retains every prior receipt.
ALTER TABLE hunting_economy_operations
    DROP CONSTRAINT IF EXISTS hunting_economy_operations_operation_kind_check;
ALTER TABLE hunting_economy_operations
    ADD CONSTRAINT hunting_economy_operations_operation_kind_check
    CHECK (operation_kind IN ('conversion','bound_credit','burn','bound_debit_burn'));

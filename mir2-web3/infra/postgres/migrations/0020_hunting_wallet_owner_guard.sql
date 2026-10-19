-- Character deletion checks all economic realms for this durable incarnation.
-- The realm-first wallet primary key cannot serve that ownership lookup.
CREATE INDEX IF NOT EXISTS hunting_economy_wallets_nonzero_owner_idx
    ON hunting_economy_wallets(account_id, character_index, character_incarnation)
    WHERE bound_gold <> 0 OR unbound_gold <> 0;

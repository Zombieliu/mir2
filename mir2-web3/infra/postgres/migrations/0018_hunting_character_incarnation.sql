-- Metadata identity only. No balance, legacy gold or save JSON is reclassified.
-- A save/upsert preserves this UUID; deletion and reinsertion generate a new one,
-- including two operations in the same PostgreSQL transaction.
ALTER TABLE characters
    ADD COLUMN IF NOT EXISTS hunting_incarnation UUID NOT NULL DEFAULT gen_random_uuid();
CREATE UNIQUE INDEX IF NOT EXISTS characters_hunting_incarnation_idx
    ON characters(hunting_incarnation);

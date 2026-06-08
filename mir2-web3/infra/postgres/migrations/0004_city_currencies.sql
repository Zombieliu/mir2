-- 0004_city_currencies.sql
--
-- Net-new (beyond Crystal) per-city reputation currency wallet.
--
-- Each town mints its own token (飞天城币 / 比奇城币 …), earned via accept-style
-- bounty quests and spendable in player trade + the auction house. The full
-- wallet already round-trips inside `character_saves.snapshot_json`; this adds a
-- dedicated, queryable JSONB column (mirroring how `credit` is denormalised
-- alongside the snapshot) so the balances can be inspected/aggregated directly.
--
-- The map is keyed by city key (e.g. {"feitian": 120, "bichon": 0}); adding a
-- new city is a code-only change (no schema change needed) thanks to the JSONB
-- shape.

ALTER TABLE character_saves
    ADD COLUMN IF NOT EXISTS city_currencies JSONB NOT NULL DEFAULT '{}'::jsonb;

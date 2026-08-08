-- 0002_normalized_projections.sql
--
-- Normalized, queryable read-side projections derived transactionally from the
-- authoritative character_saves snapshot. The JSON blobs (accounts.raw_json and
-- character_saves.snapshot_json) remain the source of truth; these tables are
-- query models maintained inside the same save transaction so they never drift
-- from the authoritative state.
--
-- They exist so operations/admin read paths and economy/anti-fraud analytics can
-- run real SQL ("who holds item X", "total gold supply", "unclaimed mail",
-- "active auctions") instead of deserializing every account blob in Rust.
--
-- Every statement is idempotent (IF NOT EXISTS) so the file can be re-applied.

-- Per-character flattened state for fast player/economy/server queries.
CREATE TABLE IF NOT EXISTS character_state (
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    character_name TEXT NOT NULL DEFAULT '',
    class TEXT NOT NULL DEFAULT '',
    gender TEXT NOT NULL DEFAULT '',
    level INTEGER NOT NULL DEFAULT 0,
    experience BIGINT NOT NULL DEFAULT 0,
    max_experience BIGINT NOT NULL DEFAULT 0,
    gold BIGINT NOT NULL DEFAULT 0,
    credit BIGINT NOT NULL DEFAULT 0,
    pk_points INTEGER NOT NULL DEFAULT 0,
    hp INTEGER NOT NULL DEFAULT 0,
    max_hp INTEGER NOT NULL DEFAULT 0,
    mp INTEGER NOT NULL DEFAULT 0,
    map_file_name TEXT NOT NULL DEFAULT '',
    position_x INTEGER NOT NULL DEFAULT 0,
    position_y INTEGER NOT NULL DEFAULT 0,
    chat_banned BOOLEAN NOT NULL DEFAULT FALSE,
    guild_name TEXT NOT NULL DEFAULT '',
    inventory_count INTEGER NOT NULL DEFAULT 0,
    belt_count INTEGER NOT NULL DEFAULT 0,
    storage_count INTEGER NOT NULL DEFAULT 0,
    equipment_count INTEGER NOT NULL DEFAULT 0,
    hero_count INTEGER NOT NULL DEFAULT 0,
    mail_count INTEGER NOT NULL DEFAULT 0,
    unread_mail_count INTEGER NOT NULL DEFAULT 0,
    unclaimed_mail_count INTEGER NOT NULL DEFAULT 0,
    active_auction_count INTEGER NOT NULL DEFAULT 0,
    has_hero BOOLEAN NOT NULL DEFAULT FALSE,
    save_version BIGINT NOT NULL DEFAULT 0,
    updated_at_ms BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, character_index)
);

CREATE INDEX IF NOT EXISTS idx_character_state_gold ON character_state(gold DESC);
CREATE INDEX IF NOT EXISTS idx_character_state_level ON character_state(level DESC);
CREATE INDEX IF NOT EXISTS idx_character_state_map ON character_state(map_file_name);
CREATE INDEX IF NOT EXISTS idx_character_state_guild ON character_state(guild_name) WHERE guild_name <> '';
CREATE INDEX IF NOT EXISTS idx_character_state_name ON character_state(character_name);

-- One row per item in any container (inventory/belt/storage/equipment/hero).
CREATE TABLE IF NOT EXISTS character_items (
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    container TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    slot INTEGER NOT NULL DEFAULT 0,
    item_key TEXT NOT NULL DEFAULT '',
    item_name TEXT NOT NULL DEFAULT '',
    unique_id BIGINT NOT NULL DEFAULT 0,
    quantity BIGINT NOT NULL DEFAULT 0,
    grade TEXT NOT NULL DEFAULT '',
    durability_current INTEGER,
    durability_max INTEGER,
    cursed BOOLEAN NOT NULL DEFAULT FALSE,
    sealed BOOLEAN NOT NULL DEFAULT FALSE,
    rental BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at_ms BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, character_index, container, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_character_items_key ON character_items(item_key) WHERE item_key <> '';
CREATE INDEX IF NOT EXISTS idx_character_items_unique ON character_items(unique_id) WHERE unique_id <> 0;
CREATE INDEX IF NOT EXISTS idx_character_items_owner ON character_items(account_id, character_index);

-- Queryable mail (recipient lookup, unclaimed/locked auditing) projected per owner.
CREATE TABLE IF NOT EXISTS character_mail (
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    mail_id BIGINT NOT NULL,
    owner_name TEXT NOT NULL DEFAULT '',
    sender TEXT NOT NULL DEFAULT '',
    recipient TEXT NOT NULL DEFAULT '',
    subject TEXT NOT NULL DEFAULT '',
    gold BIGINT NOT NULL DEFAULT 0,
    item_count INTEGER NOT NULL DEFAULT 0,
    opened BOOLEAN NOT NULL DEFAULT FALSE,
    locked BOOLEAN NOT NULL DEFAULT FALSE,
    claimed BOOLEAN NOT NULL DEFAULT FALSE,
    deleted BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at_ms BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, character_index, mail_id)
);

CREATE INDEX IF NOT EXISTS idx_character_mail_recipient ON character_mail(recipient) WHERE recipient <> '';
CREATE INDEX IF NOT EXISTS idx_character_mail_sender ON character_mail(sender) WHERE sender <> '';
CREATE INDEX IF NOT EXISTS idx_character_mail_pending ON character_mail(claimed, deleted);

-- Global auction-house view (active listings, item-key market depth) projected per seller.
CREATE TABLE IF NOT EXISTS auction_listings (
    seller_account_id TEXT NOT NULL,
    seller_character_index INTEGER NOT NULL,
    listing_id BIGINT NOT NULL,
    seller_name TEXT NOT NULL DEFAULT '',
    item_key TEXT NOT NULL DEFAULT '',
    price BIGINT NOT NULL DEFAULT 0,
    sold BOOLEAN NOT NULL DEFAULT FALSE,
    cancelled BOOLEAN NOT NULL DEFAULT FALSE,
    expired BOOLEAN NOT NULL DEFAULT FALSE,
    active BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at_ms BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (seller_account_id, seller_character_index, listing_id)
);

CREATE INDEX IF NOT EXISTS idx_auction_active ON auction_listings(active) WHERE active;
CREATE INDEX IF NOT EXISTS idx_auction_item ON auction_listings(item_key) WHERE item_key <> '';
CREATE INDEX IF NOT EXISTS idx_auction_price ON auction_listings(price);

-- NPC script progression (flags + saved values) projected for quest/progression auditing.
CREATE TABLE IF NOT EXISTS character_npc_state (
    account_id TEXT NOT NULL,
    character_index INTEGER NOT NULL,
    kind TEXT NOT NULL,
    state_key TEXT NOT NULL,
    state_value TEXT NOT NULL DEFAULT '',
    updated_at_ms BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, character_index, kind, state_key)
);

CREATE INDEX IF NOT EXISTS idx_character_npc_state_key ON character_npc_state(kind, state_key);

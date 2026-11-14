-- Hero identity survives actor despawn, sealing, trade and character sessions.
CREATE TABLE IF NOT EXISTS shared_hero_allocator (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    high_watermark INTEGER NOT NULL DEFAULT 0 CHECK (high_watermark >= 0),
    store_version BIGINT NOT NULL DEFAULT 1 CHECK (store_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO shared_hero_allocator(singleton) VALUES(TRUE) ON CONFLICT(singleton) DO NOTHING;

CREATE TABLE IF NOT EXISTS shared_heroes (
    hero_id INTEGER PRIMARY KEY CHECK (hero_id > 0),
    raw_json JSONB NOT NULL,
    store_version BIGINT NOT NULL CHECK (store_version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    attached_account_id TEXT GENERATED ALWAYS AS
        (CASE WHEN raw_json #>> '{custody,kind}' = 'attached' THEN raw_json #>> '{custody,accountId}' END) STORED,
    attached_character_index INTEGER GENERATED ALWAYS AS
        (CASE WHEN raw_json #>> '{custody,kind}' = 'attached' THEN (raw_json #>> '{custody,characterIndex}')::INTEGER END) STORED,
    attached_slot SMALLINT GENERATED ALWAYS AS
        (CASE WHEN raw_json #>> '{custody,kind}' = 'attached' THEN (raw_json #>> '{custody,slot}')::SMALLINT END) STORED,
    sealed_carrier_uid NUMERIC(20,0) GENERATED ALWAYS AS
        (CASE WHEN raw_json #>> '{custody,kind}' = 'sealed' THEN (raw_json #>> '{custody,carrierUid}')::NUMERIC(20,0) END) STORED,
    CHECK (((raw_json ->> 'id')::INTEGER = hero_id) IS TRUE),
    CHECK (((raw_json ->> 'revision')::NUMERIC BETWEEN 1 AND 18446744073709551615) IS TRUE),
    CHECK ((
        (raw_json #>> '{custody,kind}' = 'attached'
            AND attached_account_id <> '' AND attached_character_index >= 0
            AND attached_slot BETWEEN 0 AND 255
            AND (raw_json #>> '{custody,attachmentRevision}')::NUMERIC BETWEEN 1 AND (raw_json ->> 'revision')::NUMERIC)
        OR (raw_json #>> '{custody,kind}' = 'sealed'
            AND sealed_carrier_uid BETWEEN 1 AND 18446744073709551615)
        OR raw_json #>> '{custody,kind}' = 'released'
    ) IS TRUE),
    CONSTRAINT shared_hero_attachment_unique UNIQUE
        (attached_account_id, attached_character_index, attached_slot) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT shared_hero_seal_unique UNIQUE (sealed_carrier_uid) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT shared_hero_character FOREIGN KEY (attached_account_id, attached_character_index)
        REFERENCES characters(account_id, character_index) DEFERRABLE INITIALLY DEFERRED
);
-- No cascade or delete API: release retains the Hero row as an identity tombstone.

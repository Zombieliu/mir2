-- Candidate only: not registered until the authority/cutover review is complete.
CREATE TABLE shared_item_identity_allocator (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    high_watermark NUMERIC(20,0) NOT NULL CHECK (
        high_watermark >= 0 AND high_watermark <= 18446744073709551615
    ),
    store_version BIGINT NOT NULL CHECK (store_version > 0),
    bootstrapped BOOLEAN NOT NULL DEFAULT FALSE,
    census_sha256 TEXT,
    CHECK (
        (NOT bootstrapped AND high_watermark = 0 AND census_sha256 IS NULL)
        OR (bootstrapped AND census_sha256 IS NOT NULL
            AND census_sha256 ~ '^[0-9a-f]{64}$')
    )
);
INSERT INTO shared_item_identity_allocator
    (singleton, high_watermark, store_version, bootstrapped)
VALUES (TRUE, 0, 1, FALSE);

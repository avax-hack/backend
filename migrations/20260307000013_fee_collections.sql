CREATE TABLE fee_collections (
    id              SERIAL PRIMARY KEY,
    token_id        VARCHAR(42) NOT NULL,
    amount0         NUMERIC NOT NULL,
    amount1         NUMERIC NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL UNIQUE,
    block_number    BIGINT NOT NULL,
    created_at      BIGINT NOT NULL
);
CREATE INDEX idx_fee_collections_token ON fee_collections(token_id);

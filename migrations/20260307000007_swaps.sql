CREATE TABLE swaps (
    id              SERIAL PRIMARY KEY,
    token_id        VARCHAR(42) NOT NULL,
    account_id      VARCHAR(42) NOT NULL,
    event_type      VARCHAR(4) NOT NULL,
    native_amount   NUMERIC NOT NULL,
    token_amount    NUMERIC NOT NULL,
    price           NUMERIC NOT NULL,
    value           NUMERIC NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL UNIQUE,
    block_number    BIGINT NOT NULL,
    created_at      BIGINT NOT NULL
);
CREATE INDEX idx_swaps_token ON swaps(token_id, created_at DESC);
CREATE INDEX idx_swaps_account ON swaps(account_id, created_at DESC);
CREATE INDEX idx_swaps_created ON swaps(created_at DESC);

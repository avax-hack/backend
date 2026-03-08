CREATE TABLE liquidity_positions (
    token_id        VARCHAR(42) PRIMARY KEY,
    pool_id         VARCHAR(66),
    tick_lower      INT,
    tick_upper      INT,
    liquidity       NUMERIC,
    created_at      BIGINT NOT NULL
);

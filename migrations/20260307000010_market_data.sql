CREATE TABLE market_data (
    token_id            VARCHAR(42) PRIMARY KEY,
    market_type         VARCHAR(5) NOT NULL DEFAULT 'IDO',
    token_price         NUMERIC NOT NULL DEFAULT 0,
    native_price        NUMERIC NOT NULL DEFAULT 0,
    ath_price           NUMERIC NOT NULL DEFAULT 0,
    total_supply        NUMERIC NOT NULL,
    volume_24h          NUMERIC NOT NULL DEFAULT 0,
    holder_count        INT NOT NULL DEFAULT 0,
    bonding_percent     NUMERIC NOT NULL DEFAULT 0,
    milestone_completed INT NOT NULL DEFAULT 0,
    milestone_total     INT NOT NULL DEFAULT 0,
    is_graduated        BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at          BIGINT NOT NULL
);

CREATE TABLE holders (
    token_id        VARCHAR(42) NOT NULL,
    account_id      VARCHAR(42) NOT NULL,
    balance         NUMERIC NOT NULL,
    percent         NUMERIC NOT NULL,
    rank            INT NOT NULL,
    updated_at      BIGINT NOT NULL,
    PRIMARY KEY (token_id, account_id)
);
CREATE INDEX idx_holders_token_rank ON holders(token_id, rank);

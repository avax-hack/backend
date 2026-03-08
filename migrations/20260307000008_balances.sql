CREATE TABLE balances (
    account_id      VARCHAR(42) NOT NULL,
    token_id        VARCHAR(42) NOT NULL,
    balance         NUMERIC NOT NULL DEFAULT 0,
    updated_at      BIGINT NOT NULL,
    PRIMARY KEY (account_id, token_id)
);
CREATE INDEX idx_balances_token ON balances(token_id);

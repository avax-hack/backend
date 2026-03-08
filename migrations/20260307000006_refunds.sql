CREATE TABLE refunds (
    id              SERIAL PRIMARY KEY,
    project_id      VARCHAR(42) NOT NULL REFERENCES projects(project_id),
    account_id      VARCHAR(42) NOT NULL REFERENCES accounts(account_id),
    tokens_burned   NUMERIC NOT NULL,
    usdc_returned   NUMERIC NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL UNIQUE,
    block_number    BIGINT NOT NULL,
    created_at      BIGINT NOT NULL
);
CREATE INDEX idx_refunds_project ON refunds(project_id);
CREATE INDEX idx_refunds_account ON refunds(account_id);

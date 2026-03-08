CREATE TABLE investments (
    id              SERIAL PRIMARY KEY,
    project_id      VARCHAR(42) NOT NULL REFERENCES projects(project_id),
    account_id      VARCHAR(42) NOT NULL REFERENCES accounts(account_id),
    usdc_amount     NUMERIC NOT NULL,
    token_amount    NUMERIC NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL UNIQUE,
    block_number    BIGINT NOT NULL,
    created_at      BIGINT NOT NULL
);
CREATE INDEX idx_investments_project ON investments(project_id);
CREATE INDEX idx_investments_account ON investments(account_id);
CREATE INDEX idx_investments_created ON investments(created_at DESC);

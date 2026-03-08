CREATE TABLE projects (
    project_id      VARCHAR(42) PRIMARY KEY,
    name            VARCHAR(50) NOT NULL,
    symbol          VARCHAR(10) NOT NULL UNIQUE,
    image_uri       TEXT NOT NULL,
    description     TEXT,
    tagline         VARCHAR(120) NOT NULL,
    category        VARCHAR(20) NOT NULL,
    creator         VARCHAR(42) NOT NULL REFERENCES accounts(account_id),
    status          VARCHAR(20) NOT NULL DEFAULT 'funding',
    target_raise    NUMERIC NOT NULL,
    token_price     NUMERIC NOT NULL,
    ido_supply      NUMERIC NOT NULL,
    ido_sold        NUMERIC NOT NULL DEFAULT 0,
    total_supply    NUMERIC NOT NULL,
    usdc_raised     NUMERIC NOT NULL DEFAULT 0,
    usdc_released   NUMERIC NOT NULL DEFAULT 0,
    tokens_refunded NUMERIC NOT NULL DEFAULT 0,
    deadline        BIGINT NOT NULL,
    website         TEXT,
    twitter         TEXT,
    github          TEXT,
    telegram        TEXT,
    created_at      BIGINT NOT NULL,
    tx_hash         VARCHAR(66) NOT NULL
);
CREATE INDEX idx_projects_creator ON projects(creator);
CREATE INDEX idx_projects_status ON projects(status);
CREATE INDEX idx_projects_created ON projects(created_at DESC);
CREATE INDEX idx_projects_symbol ON projects(symbol);

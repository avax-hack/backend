CREATE TABLE IF NOT EXISTS pool_mappings (
    pool_id     TEXT PRIMARY KEY,
    token_id    TEXT NOT NULL,
    is_token0   BOOLEAN NOT NULL,
    created_at  BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pool_mappings_token ON pool_mappings (token_id);

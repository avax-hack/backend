CREATE TABLE block_progress (
    event_type      VARCHAR(20) PRIMARY KEY,
    last_block      BIGINT NOT NULL,
    updated_at      BIGINT NOT NULL
);

CREATE TABLE sessions (
    session_id      VARCHAR(64) PRIMARY KEY,
    account_id      VARCHAR(42) NOT NULL REFERENCES accounts(account_id),
    created_at      BIGINT NOT NULL,
    expires_at      BIGINT NOT NULL
);
CREATE INDEX idx_sessions_account ON sessions(account_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
